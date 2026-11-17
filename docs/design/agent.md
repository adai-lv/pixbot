# Agent 内核设计

> 本文件只权威定义 Agent 的回合控制流、状态、双队列、恢复、取消、事件与协作端口。下述 M0–M3 均为目标设计，当前行为以 Rust 代码与测试为准。全局约束见[架构总览](../architecture.md)；pi 的实际行为以 `resources/pi/` 源码为唯一事实来源。

## 分期与模块边界

| 阶段 | Agent 目标范围 |
|---|---|
| M0 | `run` / `steer` / `follow_up` / `set_queue_mode` / `interrupt` / `drain_background`；内轮、双队列、截断安全、取消和调用级恢复。`recover` 不发起压缩续跑 |
| M1 | 保持上述入站方法不变；启用压缩失败恢复与一次恢复 Continue，并用可控模型和工具补齐全队列契约测试。resume/fork 由装配层加载会话，不新增 Agent 路由方法 |
| M2 | 仅在 dispatch 接入通用 hook，不增加权限或审批控制流 |

模型在 `run()` 前由前端／装配层选定，`ProviderRuntime` 将其解析为 `ChatModel`；Agent 在一次 run 内只使用该实例及其不可变元数据，不做轮间换模型或路由。

| 模块 | 唯一职责 |
|---|---|
| `driver` | 双层循环、轮次边界和队列排水 |
| `state` | 运行时转录、双队列、取消令牌和未完成调用集合 |
| `events` | 事件构造、配对与串行发送 |
| `dispatch` | 工具预备、执行和结果回填；不含安全策略 |
| `recover` | 调用级重试、M1 溢出压缩协调和恢复 Continue；压缩算法属于[上下文设计](context.md) |

转录只追加，不原地改写。运行中再次 `run()` 立即返回 `RunOutcome { stop: Failed, error: Some(Busy) }`；取消只作用当前 run，空闲 `interrupt()` 幂等。以上均为 Pixbot 自身目标设计。

## M0 主循环

下面只描述 M0 请求路径，不混入 M1 工具差量、树投影或压缩。`prepare_request` 在此处仅执行 M0 线性投影、system 装配、`to_wire` v1 与 `CompletionRequest` 构造。

```rust
// 概念伪代码，不是可直接编译的 Rust。
async fn drive(ctx: &mut RunContext) -> RunOutcome {
    emit(AgentStart);
    let mut pending = ctx.drain_steering();
    let mut needs_turn = true; // run 输入已追加，至少请求一次
    let mut follow_up_budget = 1;
    let mut recover_continue_budget = 1;

    'outer: loop {
        loop {
            if !needs_turn && pending.is_empty() {
                break;
            }
            emit(TurnStart);
            for message in pending.drain(..) {
                ctx.append_and_emit(message);
            }

            let request = ctx.prepare_request().await;
            // 唯一的 assistant 调用入口；瞬时错误在内部重试。
            let assistant = recover::stream_assistant(ctx, request).await;
            let model_stop = assistant.stop_reason;
            let has_tools = has_tool_use(&assistant);

            match model_stop {
                ModelStopReason::Error | ModelStopReason::Aborted => {
                    // 即使响应已含 tool_use 也不执行；统一生成唯一错误结果。
                    let results = fail_unexecuted_tools(&assistant, model_stop);
                    ctx.finish_turn(&assistant, &results);
                    emit(TurnEnd { stop_reason: model_stop });
                    break 'outer; // 仅最终、不可再恢复的错误或中止退出
                }
                ModelStopReason::Length if has_tools => {
                    let results = fail_truncated_tools(&assistant);
                    ctx.append_tool_results(results); // 整批不执行
                }
                _ if has_tools => {
                    let results = dispatch::execute(&assistant).await;
                    ctx.append_tool_results(results);
                }
                _ => {}
            }

            needs_turn = ctx.last_batch_has_more();
            emit(TurnEnd { stop_reason: model_stop });
            // 每个工具批次后、下次请求前都排水 steering。
            pending = ctx.drain_steering();
            if !pending.is_empty() {
                needs_turn = true;
            }
        }

        if follow_up_budget > 0 {
            let follow_ups = ctx.drain_follow_up();
            if !follow_ups.is_empty() {
                follow_up_budget = 0;
                pending = follow_ups;
                needs_turn = true;
                continue 'outer;
            }
        }
        if recover_continue_budget > 0 && recover::take_continue(ctx) {
            recover_continue_budget = 0;
            needs_turn = true;
            continue 'outer;
        }
        break;
    }

    emit(TurnComplete { usage: ctx.usage });
    RunOutcome {
        usage: ctx.usage,
        stop: ctx.run_stop,
        error: ctx.last_error,
    }
}
```

每次 run 的 follow-up 和恢复 Continue **各有一次独立预算**，不是两类续跑共享一次预算。预算耗尽后新消息仍留在队列，不静默丢弃。M0 的 `recover::take_continue` 不产生续跑；M1 仅压缩成功且原先因溢出中断时置位。

`recover::stream_assistant` 是正常响应与 M1 压缩摘要的**调用级重试唯一入口**。driver 不得直接调用 `ChatModel::stream`，其他模块不得另写整轮重试循环；中间可重试错误不提前结束 turn，只有最终 `Error` / `Aborted` 才走硬退出，abort 永不重试。退避为 `baseDelayMs × 2^(attempt-1)`（`baseDelayMs` 为 pi 标识符），默认封顶 60s、无抖动；可重试断流和约定错误模式，配额／限额与 abort 不重试。该公式、60s 封顶与无抖动是 pi 快照实际行为，位于 `resources/pi/packages/ai/src/utils/retry.ts`（`DEFAULT_MAX_AGENT_RETRY_DELAY_MS=60_000`、`retryDelayMs()`），不在 `resources/pi/packages/agent/src/stream-fn.ts`；「唯一入口」与「收归 Agent recover」是 Pixbot 自身设计。provider 响应头之前的协议重试是另一层，见[Provider 设计](providers.md)。

## 队列与收敛

| 队列 | 排水点 | 语义 |
|---|---|---|
| steering | 工具批次结束、下次模型请求前 | 不打断已发出的调用；默认 `one-at-a-time` |
| follow-up | Agent 本应停止时 | 至多触发一次重入；默认 `one-at-a-time` |

取消默认保留两个队列，调用方可显式清空。每个 `tool_use` 在取消、最终模型错误、未知工具、参数错误、hook 阻断或截断时仍须回填唯一错误结果。`stop_reason == Length` 时该 assistant 消息内所有工具均不执行；最终 Error/Aborted 也走同一未执行回填函数，不得传空结果收尾。pi 快照实际行为：双队列默认 `one-at-a-time` 见 `resources/pi/packages/agent/src/agent.ts`（`steeringMode`/`followUpMode`）、`resources/pi/packages/agent/src/types.ts`（`QueueMode`）。

`run()` future resolve 即 settled：恢复、最终事件发送与后台收尾决策均已完成。前端同时消费 `AgentEvent` 流和 `run()` 的 `RunOutcome` 返回值；前者用于渲染与审计，后者用于 drain/flush 后的退出码映射，不是第二个结束事件。pi 快照实际行为：`waitForIdle()`（pi 标识符）收敛为 run resolve 即 settled，见 `resources/pi/packages/agent/src/agent.ts`。

## 事件与结果类型

下面这段 `AgentEvent` / `RunOutcome` 是 **Pixbot 自身的目标契约定义**（contracts 中立 DTO），不是 pi 的变体清单。其中 `ToolResult` 专指工具执行结果 DTO；`Content::ToolResult` 是消息内容块变体，「`tool_result` 消息」指回填消息，三者不得混用；统一用词见[接口与数据契约](../api/README.md)。

```rust
pub enum AgentEvent {
    AgentStart,
    TurnStart,
    MessageStart { message: Message },
    MessageUpdate { delta: AssistantDelta }, // 仅 assistant
    MessageEnd { message: Message },
    ToolExecutionStart { call_id: String, name: String, args: JsonValue },
    ToolExecutionUpdate { call_id: String, partial: ToolResult },
    ToolExecutionEnd { call_id: String, result: ToolResult, duration_ms: Option<u64> },
    TurnEnd { stop_reason: ModelStopReason },
    TurnComplete { usage: Usage }, // 恒为最后事件
    Error { error: AgentError },   // 带外
    Notice { level: NoticeLevel, message: String }, // 带外
}

pub struct RunOutcome {
    pub usage: Usage,
    pub stop: RunStopReason, // Completed / Aborted / Failed
    pub error: Option<AgentError>,
}
```

pi 快照实际行为：`AgentEvent`（pi 标识符）为 10 个变体（`agent_start` / `agent_end` / `turn_start` / `turn_end` / `message_start` / `message_update` / `message_end` / `tool_execution_start` / `tool_execution_update` / `tool_execution_end`），**没有 `turn_complete` / `error` / `notice`**，`tool_execution_end` 带外层 `isError` 与可选 `durationMs`（pi 标识符）；`AssistantMessageEvent` 命名形态为 `toolcall_*`、无独立 `ThinkingSignature` 变体（`resources/pi/packages/agent/src/types.ts`、`resources/pi/packages/ai/src/types.ts`）。因此上面的 `TurnComplete` / `Error` / `Notice`、`RunStopReason` 与 `ModelStopReason` 分离都是 Pixbot 自身设计，与 pi 事件模型并不相同。

`ModelStopReason` 描述单次模型响应，`RunStopReason` 描述整个 run，二者不得共用或互相代替。`ToolExecutionEnd` 不设置重复的外层 `is_error`；失败只从 `result.is_error` 读取——这是 **Pixbot 相对 pi 的显式收紧**（pi 的 `tool_execution_end` 带外层 `isError`，`AfterToolCallResult` 可逐字段覆盖 `isError` / `usage` / `terminate`，见 `resources/pi/packages/agent/src/types.ts`）。每次 run 使用独立 `mpsc::UnboundedSender<AgentEvent>`，前端顺序消费。

顺序约束：`TurnComplete` 之后不再发送本 run 事件；`Error` / `Notice` 不得插在 `TurnEnd` 与 `TurnComplete` 之间；user/system／`tool_result` 消息没有 `MessageUpdate`。请求取消通过 drop 流传播，工具经 `ToolContext` 的取消令牌协作停止；已落盘转录不回滚。以上顺序约束为 Pixbot 自身设计。

## 入站与协作端口

```rust
#[async_trait]
pub trait AgentSession: Send + Sync {
    async fn run(&self, input: RunInput, tx: EventSender) -> RunOutcome;
    fn steer(&self, message: String);
    fn follow_up(&self, message: String);
    fn set_queue_mode(&self, mode: QueueMode);
    fn interrupt(&self);
    async fn drain_background(&self, timeout: Duration) -> usize;
}
```

| 端口 | M0 契约摘要 | 异常语义 |
|---|---|---|
| `ContextEngine` | `project` 线性消息；`prepare_request` 返回 `CompletionRequest`；`to_wire` v1 | 不可降级消息返回中立错误；M1 才扩展树投影和压缩计划 |
| `SessionRepository` | `append` / `load` | 写失败只告警；坏行跳过，孤儿不进入当前投影 |
| `UsageRecorder` | 独立记录 `Usage` | 失败只告警，不影响 run |
| `ToolSet` | `definitions` / `get` | 未知工具由 dispatch 生成 `is_error` 结果 |
| `ChatModel` | 已选模型的流式调用 | 失败编码为流内事件，同步不抛错 |
| `EventSender` | 顺序发送事件 | 前端断开后丢弃后续发送并请求取消 |

所有端口与 DTO 位于 `contracts`。`AgentError`、`SessionError`、`UsageError`、`ToolError`、`ProviderError` 等端口错误均为 contracts 中立类型；实现 crate 的内部错误必须映射后才能越界。上述端口名称不要求 contracts 定义多个 thiserror 枚举；contracts 的单一错误实现及映射形状由 `0.1.0` 规格按 AGENTS 规则固定。M2 `ToolHooks` 也遵守该边界。

## pi 对齐索引

事实出处均在 `resources/pi/` 源码。本节的治理定位是**不维护第二份逐行映射**，只给出实测文件清单与职责边界，供核对方向参考（以行内代码标注源码相对路径，不新增逐文件链接）。

pi 快照实际行为：`resources/pi/packages/agent/src/` 只有 6 个 `.ts` 文件——`types.ts`（`AgentToolResult` / `AgentTool` / `AgentEvent` / `ToolExecutionMode` / `QueueMode` / `AgentLoopConfig` / `BeforeToolCallResult` / `AfterToolCallResult` / `PrepareRequest` / `AssistantMessageEvent` 等契约，均为 pi 标识符）、`agent-loop.ts`（流水线、双队列排水、事件发射序、`prepareToolCall` 与 sequential 降级、三个回调的调用点）、`agent.ts`（`waitForIdle`、`steeringMode`/`followUpMode` 默认 `one-at-a-time`、`toolExecution` 默认 `parallel`、事件监听）、`stream-fn.ts`（仅 `setDefaultStreamFn`/`getDefaultStreamFn`，不含调用级重试）、`index.ts`（导出面）、`proxy.ts`（导出符号）。该目录下**没有 `agent-session.ts`**。

pi 快照实际行为：`waitForIdle`（pi 标识符）位于 `resources/pi/packages/agent/src/agent.ts`；调用级整轮重试 `retryAssistantCall`（pi 标识符）位于 `resources/pi/packages/ai/src/utils/retry.ts`，不在 `packages/agent/`；`resources/pi/packages/coding-agent/src/core/agent-session.ts` 的职责是 **agent 生命周期与会话管理**（状态、事件订阅、模型/thinking、压缩、bash、会话切换与分支），**不是 recover**，且位于 `coding-agent` 而非 `agent` 包。

pi 快照实际行为：工具预备顺序为 `prepareToolCallArguments → validateToolArguments → beforeToolCall → execute → afterToolCall`，`beforeToolCall` 仅返回 block/reason/terminate、不修改参数（`resources/pi/packages/agent/src/agent-loop.ts`、`resources/pi/packages/agent/src/types.ts`）；`AfterToolCallResult`（pi 标识符）可逐字段覆盖 `isError` / `usage` / `terminate`（`resources/pi/packages/agent/src/types.ts`）。Pixbot 的 M0/M2 流水线（prepare → before hook → 最终 schema 校验 → execute、改参后再校验）与 after 三通道白名单是**相对 pi 的收紧**，属 Pixbot 自身设计；详见[工具设计](tools.md)。
