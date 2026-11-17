use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::ContractError;
use crate::event::ModelStopReason;
use crate::request::CompletionRequest;
use crate::usage::Usage;

/// 对已选 Provider 与模型的稳定引用。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelRef {
    /// Provider 标识。
    pub provider: String,
    /// Provider 内的模型标识。
    pub model: String,
}

/// 请求装配所需的最小不可变模型元数据。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelMetadata {
    /// 模型标识。
    pub id: String,
    /// 协议适配器标识。
    pub api: String,
    /// 上下文窗口 token 上限。
    pub context_window: u64,
    /// 最大输出 token 数。
    pub max_output_tokens: u64,
}

/// 中立 wire 消息角色。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WireRole {
    /// 系统指令。
    System,
    /// 用户输入。
    User,
    /// 助手响应。
    Assistant,
    /// 工具结果角色。
    Tool,
}

/// 中立 wire 内容块，与可扩展领域 `Content` 相互独立。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireContent {
    /// 文本内容。
    Text { text: String },
    /// 思考内容及可选签名。
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
    /// 模型工具调用。
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// 回填的工具结果内容块。
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

/// 已完成领域到 wire 转换的消息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireMessage {
    /// wire 角色。
    pub role: WireRole,
    /// 有序 wire 内容块。
    pub content: Vec<WireContent>,
}

/// 模型流事件；调用失败只经 `Error` 表达。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StreamEvent {
    /// 响应开始。
    Start,
    /// 文本增量。
    TextDelta(String),
    /// 思考文本增量。
    ThinkingDelta(String),
    /// 思考签名增量。
    ThinkingSignature(String),
    /// 工具调用开始。
    ToolUseStart { id: String, name: String },
    /// 工具调用部分 JSON 参数。
    ToolUseDelta { id: String, partial_json: String },
    /// 工具调用结束。
    ToolUseEnd { id: String },
    /// 响应完成。
    Done {
        stop_reason: ModelStopReason,
        usage: Usage,
        model: String,
    },
    /// 认证、连接、协议或流内失败。
    Error(ContractError),
}

/// 可跨 tokio 任务传递的模型事件流。
pub type ChatStream = Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>;

/// 已选模型的流式调用端口，不提供同步错误返回路径。
#[async_trait]
pub trait ChatModel: Send + Sync {
    /// 返回本次调用的不可变模型元数据。
    fn metadata(&self) -> &ModelMetadata;
    /// 使用完整中立请求创建事件流。
    async fn stream(&self, request: CompletionRequest) -> ChatStream;
    /// 使用简化兼容路径创建事件流。
    async fn stream_simple(&self, request: CompletionRequest) -> ChatStream;
}

/// 只按已选 `ModelRef` 解析模型，不执行自动选模或路由。
#[async_trait]
pub trait ProviderRuntime: Send + Sync {
    /// 解析已选模型对应的聊天端口。
    async fn resolve(&self, model: &ModelRef) -> Result<Arc<dyn ChatModel>, ContractError>;
}
