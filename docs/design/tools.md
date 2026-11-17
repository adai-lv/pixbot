# 工具系统设计

> 本文件只权威定义工具 DTO、注册与执行流水线、M0 六工具最低行为及固定 pi 工具基线。所有里程碑内容均为目标。回合控制见[Agent 设计](agent.md)，hook 与 sandbox 边界见[安全设计](security.md)。

## 结果与 Tool 契约

协议工具 ID 统一使用小写 `read` / `write` / `edit` / `bash` / `glob` / `grep`（Pixbot 自命名的 M0 六工具）；pi 内建工具 ID 也是小写，但集合为 `read` / `bash` / `powershell` / `edit` / `write` / `grep` / `find` / `ls`，其中没有 `glob`（见 `resources/pi/packages/coding-agent/src/core/tools/index.ts` 的 `ToolName` / `allToolNames`：`ToolName` 是这 8 个字面量的闭合联合类型，`allToolNames` 是运行时 `Set`，见 `tools/index.ts:95-96`）。`glob` 是 Pixbot 额外工具，对应 pi 快照的 `find`。

下表是工具执行结果 DTO `ToolResult` 的字段，按实测为 6 个：`content` / `details` / `structured_content` / `usage` / `is_error` / `terminate`。pi 侧对应 `AgentToolResult`（`resources/pi/packages/agent/src/types.ts`），字段为 `content` / `details` / `structuredContent` / `usage` / `isError` / `terminate`。表中是 Pixbot Rust 契约的 snake_case 写法；下文出现的 camelCase 名称（如 `structuredContent`、`autoResizeImages`、`BeforeToolCallResult`）均为 **pi 标识符**，保留原始写法。

| 字段 | 消费者 | 语义 |
|---|---|---|
| `content` | 模型 | text/image 结果 |
| `details` | 前端/日志 | 渲染与调试，不发给模型 |
| `structured_content` | 程序化调用方 | 形状由 `output_schema` 声明，不发给模型 |
| `usage` | 台账/前端 | 工具自身用量，不并入主上下文估算 |
| `is_error` | 模型/前端 | 唯一失败标记 |
| `terminate` | Agent | 仅当批次全部 finalized result 都为 true 时提前停止（`resources/pi/packages/agent/src/agent-loop.ts` 的 `shouldTerminateToolBatch`） |

业务失败返回 `Err` 或工具执行结果 DTO `ToolResult { is_error: true }`，dispatch 将 `Err` 归一化为错误结果；不得只把错误写进 `content`，不得 panic。`ToolContext` 携带 `call_id`、cwd、取消令牌和 `on_update`；`on_update` 发送完整 partial result。「settle 后更新忽略」在本次核对范围内未逐行确证，不写成 pi 已确证行为；`Err` 归一化是 Pixbot dispatch 自身设计。

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> &JsonValue;
    fn output_schema(&self) -> Option<&JsonValue> { None }
    fn constrained_sampling(&self) -> Option<ConstrainedSampling> { None }
    fn prepare_arguments(&self, input: JsonValue) -> Result<JsonValue, ToolError> {
        Ok(input)
    }
    async fn execute(&self, ctx: ToolContext, input: JsonValue)
        -> Result<ToolResult, ToolError>;
    fn execution_mode(&self) -> ToolExecutionMode { ToolExecutionMode::Parallel }
    fn replay(&self) -> ToolReplay { ToolReplay::Never }
}
```

上述 `Tool` trait 端口的默认值（`Parallel` / `Never` 等）属 **Pixbot 自身设计**。pi 侧 `AgentTool` 的 `outputSchema` / `prepareArguments` / `executionMode` / `replay` 均为 optional（pi 标识符），`constrainedSampling` 位于 `resources/pi/packages/ai/src/types.ts`；pi 源码未定义显式默认常量，并行默认来自 `resources/pi/packages/agent/src/agent.ts` 的 `toolExecution ?? "parallel"`、`resources/pi/packages/agent/src/agent-loop.ts` 的非 sequential 即并行。

端口错误使用 contracts 中立类型。`ToolSet` 负责注册、选择活动集和按注册序声明；内建工具形成连续前缀，新工具追加在末尾。未知工具不执行，只回填错误结果。

## 执行流水线

```text
prepare_arguments
  → beforeToolCall（M2，可选，可修改 input 或阻断）
  → 最终 input schema 校验
  → execute
  → afterToolCall（M2，可选）
  → ToolExecutionEnd
```

M0 没有 hook，顺序就是 prepare → 最终 schema 校验 → execute。M2 before hook 改参后**必须再次校验**，不能沿用 prepare 前的校验结果；未知工具、准备失败、hook 阻断和校验失败均不进入 execute。该流水线（含 before hook 可改 input、改参后再校验）是 **Pixbot M0/M2 通用 hook 设计**，不是 pi 行为。pi 快照实际行为：预备顺序为 `prepareToolCallArguments → validateToolArguments → beforeToolCall → execute → afterToolCall`（`resources/pi/packages/agent/src/agent-loop.ts`）；`beforeToolCall` 仅返回 block/reason/terminate、**不修改参数**，参数校验在 hook 之前（`resources/pi/packages/agent/src/types.ts` 的 `BeforeToolCallResult`）。

- 预备严格串行；执行默认并行，全局 sequential 或批内任一工具声明 sequential 时整批顺序执行（`resources/pi/packages/agent/src/agent-loop.ts`、`resources/pi/packages/agent/src/agent.ts`）。
- `ToolExecutionEnd` 按完成序发送，「`tool_result` 消息」按 assistant 源序回填；未运行 execute 时 `duration_ms=None`。
- `ToolExecutionEnd` 只携带 `result`，错误从 `result.is_error` 读取，不设置重复外层字段——这是 **Pixbot 相对 pi 的显式收紧**。pi 快照实际行为：`tool_execution_end` 带外层 `isError` 与可选 `durationMs`（pi 标识符，`resources/pi/packages/agent/src/types.ts`），`durationMs` 在工具未运行时缺省。
- 每个 `tool_use` 有且仅有一个「`tool_result` 消息」。取消、最终模型错误、未知工具、参数错误、hook 阻断、截断都回填 `is_error=true`；abort 后不再预备后续调用。
- 同文件 edit/write 经 `file_mutation_queue` 按真实路径串行，不同文件仍可并行（`resources/pi/packages/coding-agent/src/core/tools/file-mutation-queue.ts` 的 `withFileMutationQueue`）。
- after hook 只能替换三通道（即 `content` / `details` / `structured_content` 三字段），不能覆盖内核控制的 `is_error` / `terminate` / `usage`；逐字段替换而非深合并；仅替换 `content` 而未提供 `structured_content` 时丢弃旧 `structured_content`。这是 **Pixbot 相对 pi 的显式收紧**：pi 的 `AfterToolCallResult`（pi 标识符）可逐字段覆盖 `isError` / `usage` / `terminate`（`resources/pi/packages/agent/src/types.ts`；合并见 `resources/pi/packages/agent/src/agent-loop.ts`），仅给 `content` 时同样丢弃旧 `structuredContent`。

## M0 六工具最低可观察行为

下表是 **M0 验收下限**，不是完整 pi 兼容声明，`glob` 名称与六行结构均为 Pixbot 自身决策。

| ID | M0 最低行为 | 明确留到后续 |
|---|---|---|
| `read` | 读取文本；offset 从 1 开始并支持 limit；最多返回 2000 行/50KiB，截断有显式标记；不存在、不可读和越界参数返回 `is_error` | 图片、缩放和非视觉模型提示 |
| `write` | 全文件覆盖、递归创建父目录；与 edit 同文件串行；失败保留诊断（Pixbot 目标设计，pi 源码未体现专门诊断）；取消不回滚已在途写入 | 完整 schema 与平台差异 |
| `edit` | 单次非空 `oldText` 精确替换（Pixbot M0 目标）；匹配必须唯一且新内容有变化；与 write 同文件串行；无匹配/多匹配均为错误 | 多 edit、旧参数转换、BOM/行尾与完整 diff 对齐 |
| `bash` | 经配置 shell 执行，无默认 timeout；保留 stdout/stderr 与退出码；非零退出为 `is_error` 且保留输出；100ms partial；取消/超时杀进程树；尾部最多 2000 行/50KiB 并标记截断，超限返回摘要与落盘路径（布局见「大输出与后台」） | 结构化 output 1MiB 截断与平台差异 |
| `glob` | 在指定 root 内按 glob 匹配，默认 limit=1000；结果确定性排序，空结果和截断可区分；非法模式/不可访问 root 明确报错 | 作为 Pixbot 额外工具扩展完整选项；对应 pi 快照的 `find`，pi 内建工具中没有 `glob`，不计入 pi 八工具基线 |
| `grep` | 支持 regex/literal、ignoreCase、glob，默认 limit=100；每行最多 500 字符、整体最多 50KiB 并标记截断；rg 缺失或执行失败明确报错，M0 不静默假绿 | context、完整参数及 rg 获取/离线规则 |

pi 快照实际行为：上述截断与默认数值均可在源码确证——`DEFAULT_MAX_LINES=2000`、`DEFAULT_MAX_BYTES=50*1024`、`GREP_MAX_LINE_LENGTH=500`（`resources/pi/packages/coding-agent/src/core/tools/truncate.ts`），bash partial 节流 `BASH_UPDATE_THROTTLE_MS=100`（`resources/pi/packages/coding-agent/src/core/tools/renderers/bash.ts`），结构化 full output `STRUCTURED_OUTPUT_MAX_BYTES=1024*1024`（`resources/pi/packages/coding-agent/src/core/tools/bash.ts`），grep `DEFAULT_LIMIT=100`（`resources/pi/packages/coding-agent/src/core/tools/grep.ts`），find `DEFAULT_LIMIT=1000`（`resources/pi/packages/coding-agent/src/core/tools/find.ts`），ls `DEFAULT_LIMIT=500`（`resources/pi/packages/coding-agent/src/core/tools/ls.ts`），read 的 `offset` 为 1-indexed（`resources/pi/packages/coding-agent/src/core/tools/read.ts`）。

六工具均须覆盖正常、边界、异常、取消及结果回填。零额外语言运行时不等于零外部工具：pi 快照实际行为中 `grep` 依赖 `ensureTool("rg")`、`find` 依赖 `ensureTool("fd")`（含 `fdfind` 别名），`PI_OFFLINE` 开启时跳过自动下载（`resources/pi/packages/coding-agent/src/utils/tools-manager.ts`）；`ls` 无外部二进制依赖（`resources/pi/packages/coding-agent/src/core/tools/ls.ts`）。shell/rg 的缺失必须可诊断，不能把未执行当成功。

## M1 完整 pi 工具基线

M1 对齐固定快照的八个内建工具：默认活动集为 `read/bash/edit/write`，只读预设为 `read/grep/find/ls`；“默认启用”不等于全量基线。Pixbot 的 `glob` 是额外工具，powershell/find/ls 是 M1 补齐项。pi 快照实际行为：8 个内建工具、默认 coding 活动集与只读预设见 `resources/pi/packages/coding-agent/src/core/tools/index.ts`（`createCodingToolDefinitions` `:164-171` / `createReadOnlyToolDefinitions` `:173-180` / `createAllToolDefinitions` `:182-193`（8 项全量定义），均为 pi 标识符）。

| ID | M1 必须补齐的行为摘要 |
|---|---|
| `read` | jpg/png/gif/webp/bmp、图片缩放、非视觉模型提示、结构化文本或图片结果 |
| `bash` | 在 M0 基础上合并 stdout/stderr，补齐完整日志路径、结构化 output 1MiB 头尾截断与平台差异 |
| `powershell` | 与 bash 同 schema/结果/截断；仅原生 Windows；pwsh/PowerShell 解析、UTF-8、NoProfile、NonInteractive |
| `edit` | 多 edit 对同一原文执行；`oldText` 唯一、非空、不重叠且有变化；旧参数兼容、BOM/行尾保留、diff/patch |
| `write` | 完整覆盖、父目录、在途取消与同文件串行 |
| `grep` | 在 M0 基础上补齐 context 与完整参数；按 pi 获取 rg，离线禁止下载且不可用时明确失败 |
| `find` | fd glob、hidden、尊重 gitignore、默认 limit=1000、basename/full-path、目录 `/`、50KiB；fd/fdfind 获取与离线规则同 grep |
| `ls` | 默认 path=`.`、limit=500、dotfiles、忽略大小写排序、目录 `/`、空目录提示、50KiB，无外部二进制依赖 |

完整验收还包括各工具 input/output schema、`constrainedSampling`（pi 标识符，`resources/pi/packages/coding-agent/src/core/tools/tool-definition-wrapper.ts`）、三通道、`usage`、错误、abort、`onUpdate`（pi 标识符）和平台差异。MCP、codemode 及其他 pi 源码未体现的额外工具命名不占八项基线名额。

pi 快照实际行为（M1 相关补入）：`read` 已支持图片 jpg/png/gif/webp/bmp、`autoResizeImages` 默认 `true` 与非视觉模型提示（`resources/pi/packages/coding-agent/src/core/tools/read.ts`）；`powershell` 与 bash 同 schema/结果/截断，`POWERSHELL_ARGS` 为 `-NoProfile/-NonInteractive/-ExecutionPolicy Bypass/-Command` 且非 win32 走专门分支（`resources/pi/packages/coding-agent/src/core/tools/powershell.ts`、`resources/pi/packages/coding-agent/src/utils/shell.ts`）；`write` 全量覆盖、递归建父目录、经 `withFileMutationQueue` 同文件串行，取消检查在每个 `await` 之后、**源码无回滚逻辑**，成功返回 `details: undefined` 且无专门结构化诊断（`resources/pi/packages/coding-agent/src/core/tools/write.ts`）；`edit` 的 `oldText` 须非空、唯一、有变化，多个 `edits[]` 互不重叠，默认**精确匹配优先、失败回退模糊匹配**（`fuzzyFindText`），兼容旧顶层 `oldText`/`newText` 与 JSON 字符串 `edits`，保留 BOM 与原行尾，结果携带 `diff` 与 `patch`（`resources/pi/packages/coding-agent/src/core/tools/edit.ts`、`resources/pi/packages/coding-agent/src/core/tools/edit-diff.ts`）。因此 M0 的「精确替换」是 Pixbot 目标措辞，不写成 pi 行为；pi 源码未体现「write 异常矩阵」这一具体条目，故不列入补齐表。

## 大输出与后台

大输出按工具特定规则截断，不使用统一 30k 字符阈值冒充 pi 兼容。pi 快照实际行为：shell 超限内容**不写入项目内 `tool-results/` 目录**，而是落在 OS `tmpdir()` 下 `pi-output-*.log` / `pi-bash-*` / `pi-powershell-*` 临时文件，权限 `0o600`（`createOutputFileStream` 定义在 `resources/pi/packages/coding-agent/src/utils/output-files.ts:31`，`OUTPUT_FILE_MODE=0o600` 在 `:15`、`flag:"wx"` 在 `:26,:33`；`resources/pi/packages/coding-agent/src/core/tools/output-accumulator.ts:249` 与 `resources/pi/packages/coding-agent/src/core/bash-executor.ts:66` 只是调用方）；返回摘要、截断标记与完整日志路径；结构化 output 另受 1MiB 限制（`resources/pi/packages/coding-agent/src/core/tools/bash.ts`）。

`~/.pixbot/sessions/<sid>/tool-results/` 是 **Pixbot 目标布局，非 pi 行为，当前未创建**（相关目标路径见[会话设计](session.md)）。M2 后台任务（Pixbot 目标设计）输出落盘，终态以 `is_meta` 通知回流，不直接回灌主上下文；pi 源码未体现 `TaskRegistry` 一类具体后台组件命名，不将其作为 pi 已有组件列出。

## pi 对齐索引

固定基线的工具注册、结果字段和平台细节，以 `resources/pi/` 源码为唯一事实来源；出处索引见 [pi-architecture](../../resources/pi-architecture.md)。本文只保留 Pixbot 的分期和契约差异，不复制第二份逐行源码映射。数值与清单的核对路径（行内代码，按包归并）：工具行为见 `resources/pi/packages/coding-agent/src/core/tools/{index,read,write,edit,edit-diff,bash,grep,find,ls,powershell,truncate,output-accumulator,file-mutation-queue,tool-definition-wrapper}.ts` 与 `resources/pi/packages/coding-agent/src/utils/{output-files,tools-manager,shell}.ts`；流水线与结果类型见 `resources/pi/packages/agent/src/{types,agent,agent-loop}.ts`；端口字段见 `resources/pi/packages/ai/src/types.ts`。
