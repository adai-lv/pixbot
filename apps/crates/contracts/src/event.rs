use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::ContractError;
use crate::message::Message;
use crate::tool::ToolResult;
use crate::usage::Usage;

/// assistant 消息的流式增量。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AssistantDelta {
    /// 文本增量。
    Text(String),
    /// 思考文本增量。
    Thinking(String),
    /// 思考签名增量。
    ThinkingSignature(String),
    /// 工具调用开始。
    ToolUseStart { id: String, name: String },
    /// 工具调用的部分 JSON 参数。
    ToolUseDelta { id: String, partial_json: String },
    /// 工具调用结束。
    ToolUseEnd { id: String },
}

/// 带外通知级别。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NoticeLevel {
    /// 一般信息。
    Info,
    /// 警告。
    Warn,
    /// 错误通知。
    Error,
}

/// 运行中输入队列的排水方式。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueueMode {
    /// 每次排水一条。
    OneAtATime,
    /// 一次排空当前队列。
    All,
}

/// 单次模型响应的停止原因。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelStopReason {
    /// 正常停止。
    Stop,
    /// 需要执行工具。
    ToolUse,
    /// 输出长度受限。
    Length,
    /// 模型响应失败。
    Error,
    /// 模型响应被中止。
    Aborted,
}

/// 整个 `run()` 的停止原因，与单次模型响应分型。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStopReason {
    /// 正常完成。
    Completed,
    /// 运行被中止。
    Aborted,
    /// 运行失败。
    Failed,
}

/// Agent 运行事件；运行时实现负责顺序，本契约不伪造顺序保证。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AgentEvent {
    /// 一次运行开始。
    AgentStart,
    /// 一个内轮开始。
    TurnStart,
    /// 完整消息开始。
    MessageStart { message: Message },
    /// 仅 assistant 消息的增量更新。
    MessageUpdate { delta: AssistantDelta },
    /// 完整消息结束。
    MessageEnd { message: Message },
    /// 工具执行开始。
    ToolExecutionStart {
        call_id: String,
        name: String,
        args: serde_json::Value,
    },
    /// 工具执行的部分结果。
    ToolExecutionUpdate {
        call_id: String,
        partial: ToolResult,
    },
    /// 工具执行结束；失败只从 `result.is_error` 读取。
    ToolExecutionEnd {
        call_id: String,
        result: ToolResult,
        duration_ms: Option<u64>,
    },
    /// 一个内轮结束。
    TurnEnd { stop_reason: ModelStopReason },
    /// 整个运行收尾事件，运行时应在最后发送。
    TurnComplete { usage: Usage },
    /// 带外错误。
    Error { error: ContractError },
    /// 带外通知。
    Notice { level: NoticeLevel, message: String },
}

impl AgentEvent {
    /// 从工具结果与可选持续时间构造执行结束事件，不执行 IO 或读取时钟。
    pub fn tool_execution_end(
        call_id: impl Into<String>,
        result: ToolResult,
        duration_ms: Option<u64>,
    ) -> Self {
        Self::ToolExecutionEnd {
            call_id: call_id.into(),
            result,
            duration_ms,
        }
    }

    /// 从累计用量构造运行收尾事件，不读取全局状态。
    pub fn turn_complete(usage: Usage) -> Self {
        Self::TurnComplete { usage }
    }
}

/// 一次 `run()` 的返回值，不是第二个结束事件。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunOutcome {
    /// 本次运行的累计用量。
    pub usage: Usage,
    /// 整个运行的停止原因。
    pub stop: RunStopReason,
    /// 最终中立错误；成功或无需错误时为 None。
    pub error: Option<ContractError>,
}

/// 一次 `run()` 的唯一输入。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunInput {
    /// 要追加并处理的领域消息。
    pub message: Message,
}

/// 同步、非阻塞事件发送端口；false 只表示接收端已经关闭。
pub trait EventSink: Send + Sync {
    /// 尝试发送事件，不执行异步等待。
    fn send(&self, event: AgentEvent) -> bool;
}

/// 可共享的事件发送端口对象。
pub type EventSender = Arc<dyn EventSink>;

/// Agent 回合控制端口。
#[async_trait]
pub trait AgentSession: Send + Sync {
    /// 处理一条输入并返回运行结果。
    async fn run(&self, input: RunInput, tx: EventSender) -> RunOutcome;
    /// 在运行中加入 steering 消息。
    fn steer(&self, message: String);
    /// 在运行中加入 follow-up 消息。
    fn follow_up(&self, message: String);
    /// 设置队列排水方式。
    fn set_queue_mode(&self, mode: QueueMode);
    /// 请求中止当前运行。
    fn interrupt(&self);
    /// 在指定预算内等待后台工作收尾。
    async fn drain_background(&self, timeout: Duration) -> usize;
}
