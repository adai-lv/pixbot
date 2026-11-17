# 消息与上下文工程设计

> 本文件只权威定义消息双轨、上下文投影、system 装配、`to_wire`、请求构造、缓存元数据与 M1 压缩策略。以下分期均为目标而非已实现能力。会话物理格式见[会话设计](session.md)，Provider 适配见[Provider 设计](providers.md)。

## M0 与 M1 边界

| 能力 | M0 | M1 |
|---|---|---|
| 投影 | `TranscriptEntry::Message` 的线性序列，过滤 `is_meta`，保持追加顺序 | 完整条目树、当前分支、最新 Compaction 与 ContextEdit |
| system | 分节装配并参与 `to_wire` | 追加工具差量声明 |
| 出站转换 | `to_wire` v1 与 `CompletionRequest` | 扩展 M1 领域条目；协议转换边界不变 |
| 清理与压缩 | 不实现，也不设置含糊的 no-op `microcompact` / `autocompact` | `microcompact`、请求前/轮间/run 末 autocompact 与恢复协作 |
| 模型 | 只读取 run 前已选模型的元数据 | 同样固定模型；不路由、不轮间换模型 |

M0 的 `ContextEngine::prepare_request` 直接产出 `CompletionRequest`。M1 才将计划扩展为 `Ready(CompletionRequest)` 或 `NeedsCompaction`；不能用永远为 false 的标记冒充 M0 压缩实现。上述表格、`Ready` / `NeedsCompaction`、`microcompact` / `autocompact` 与管线中的 `declare_tool_changes` / `project_tree` / `normalize` 均为 **Pixbot 自身管线与状态命名**（pi 源码未体现），不得当作 pi 标识符。

## 消息双轨与 M0 管线

领域消息可扩展，wire 消息只有 system/user/assistant/tool 四种角色。只有 `ContextEngine::to_wire` 理解领域变体并决定协议目标角色；Provider 适配器仅做 wire 到厂商 JSON 的字段映射。

```text
已选 ChatModel + 不可变 ModelMetadata（run 前确定）
  → project_linear             # M0：有效 Message 的线性投影
  → assemble_system            # 稳定分节在前，动态分节在后
  → to_wire_v1                 # 唯一领域 → wire 边界
  → CompletionRequest          # contracts 中立 DTO
  → agent::recover::stream_assistant → ChatModel::stream
```

`CompletionRequest` 由 Context 产出、Provider 消费，定义在 `contracts`，不是 Provider 私有类型。它携带 wire 消息、工具声明和已选模型的请求元数据；Context 不查询目录、不选择协议，也不返回新模型。

| M0 领域形态 | wire 规则 |
|---|---|
| system 基础 prompt | 折叠为前导 system 块 |
| user / assistant | 保留角色，内容块按中立 wire 模型映射 |
| `tool_result` 消息 | `to_wire` 按目标协议决定 Anthropic user 内容块或 OpenAI tool role；Provider 只映射字段 |
| `is_meta=true` | 不进入模型请求，仅供前端与落盘 |

M0 目标增量只有 `Role::System`、工具结果 `details` 及上述 v1 转换；`Content::Image`、工具差量附加字段和完整树条目属于 M1。新增持久化字段必须使用 serde 默认值。此处「工具结果」指工具执行结果 DTO `ToolResult` 的 `details`，与消息内容块变体 `Content::ToolResult`、回填的「`tool_result` 消息」三者不同。pi 快照实际行为：wire 字段映射见 `resources/pi/packages/ai/src/api/openai-completions.ts`、`resources/pi/packages/ai/src/api/anthropic-messages.ts`，领域到 LLM Message 的转换对应物见 `resources/pi/packages/ai/src/api/transform-messages.ts`。

## system 装配与缓存元数据

pi 快照实际行为：system prompt 的分节顺序为 `preamble → tools → rules → docs →（可选 addendum）→ project_context → skills → cwd → custom sections`（`resources/pi/packages/coding-agent/src/core/system-prompt.ts`）。Pixbot 保留「**稳定前缀在前、动态在后**」的定性：cwd、日期、M1 工具差量和记忆等动态内容在后；稳定注册序与分节确定性共同构成稳定前缀。

`cacheable` 是**请求装配期的运行时元数据**，由确定性分节计算，不是 `Message` 或 `TranscriptEntry` 的持久化字段；该名称是 **Pixbot 自身命名，pi 源码未体现同名标识符**。首个 `cacheable=false` 是动态边界；缓存标记不改变转录事实。pi 快照实际行为：Anthropic 适配器可在稳定边界设置 `cache_control`（`resources/pi/packages/ai/src/api/anthropic-messages.ts`），OpenAI 依赖前缀缓存并有 `prompt_cache_key` 与 `clampOpenAIPromptCacheKey`（pi 标识符，`resources/pi/packages/ai/src/api/openai-prompt-cache.ts`、`resources/pi/packages/ai/src/api/openai-completions.ts`）。

## M1 请求管线

```text
declare_tool_changes       # 追加工具差量 system 消息
  → project_tree           # 当前分支；应用最新压缩与 ContextEdit
  → prepare_request        # 仅用固定模型元数据做阈值预检
  → 需要时由 agent::recover 协调压缩并重走管线
  → microcompact           # 只清出站副本
  → to_wire → normalize → CompletionRequest
  → agent::recover::stream_assistant → ChatModel::stream
```

`prepare_request`（Pixbot 端口方法）不执行模型路由。工具差量先作为领域事实追加，再参与投影；project 不负责计算差量。同名定义变化按“删除 + 新增”记录，回放全部差量即可重建工具面，避免重写历史。此管线整体、`declare_tool_changes` / `project_tree` / `normalize` / `microcompact` 命名均为 Pixbot 自身设计，pi 源码未体现。

M1 降级规则：工具差量 system 渲染为文本但不进入工具列表；Compaction 转为带前后缀的 user 摘要，原文不再进入投影；ContextEdit 只改变投影，不生成 wire 消息。以上为 Pixbot 目标设计，pi 压缩参考见 `resources/pi/packages/coding-agent/src/core/compaction/`。

## M1 压缩与清理

压缩阈值 `reserve_tokens=16384`、`keep_recent_tokens=20000`，当估算上下文超过 `context_window - reserve_tokens` 时触发。pi 快照实际行为：对应 pi 标识符为 `reserveTokens` / `keepRecentTokens`（camelCase），`shouldCompact = contextTokens > contextWindow - reserveTokens`（`resources/pi/packages/coding-agent/src/core/settings-defaults.ts`、`resources/pi/packages/coding-agent/src/core/compaction/compaction.ts`）；Pixbot 端口使用 snake_case。切点从最新消息回溯，并对齐合法条目边界；绝不切在「`tool_result` 消息」，带 `tool_use` 的 assistant 与其结果必须同侧。单个超预算 user 以该条目为切点；没有可摘要内容则不压缩。只有切在 turn 中途才标记 `is_split_turn`（pi 标识符 `isSplitTurn`，见 `resources/pi/packages/coding-agent/src/core/compaction/compaction.ts` 的 `findCutPoint`）。

摘要使用本次 run 已选模型，追加 `first_kept_uuid` / `tokens_before`（Pixbot snake_case；pi 标识符为 `firstKeptEntryId` / `tokensBefore`，见 `resources/pi/packages/coding-agent/src/core/session-manager.ts` 的 `appendCompaction`）。pi 快照实际行为：摘要模板分节为 `Goal` / `Constraints & Preferences` / `Progress` / `Key Decisions` / `Next Steps` / `Critical Context`（`SUMMARIZATION_PROMPT`，`resources/pi/packages/coding-agent/src/core/compaction/compaction.ts`），工具结果裁剪 `TOOL_RESULT_MAX_CHARS=2000`（`resources/pi/packages/coding-agent/src/core/compaction/utils.ts`）。投影只采用最新摘要，不叠加历史摘要。瞬时断流可经 `agent::recover::stream_assistant` 按调用级策略重试；确定性错误、abort、完整但 `Error/Length` 的响应不重试，发送 Notice 并保留原上下文。压缩成功且原先因溢出中断时，recover 消耗一次 Continue 预算。以上重试与预算语义为 Pixbot 自身设计。

`microcompact` 只清理出站副本中的遮蔽条目和过期注入段，不改写转录。该名称是 Pixbot 对轻量投影清理的命名，不应到 pi 快照寻找同名函数；pi 快照实际行为中确无同名函数。

token 估算从保守的字符/字节启发式起步，以 Provider `Done` 的 usage 校准，只用于阈值和切点，不作为计费依据。

## pi 对齐索引

事实出处均在 `resources/pi/` 源码。本文不复制逐行映射，只给出实测对应关系（pi 标识符保留 camelCase）：`convertToLlm` 是领域消息 → LLM Message 的转换（对应 `to_wire` 的领域侧语义），`transformContext` 是 LLM 前的上下文变换，`prepareRequest` 是**每次对话式 provider 请求前**返回 `AgentRequestUpdate` 的运行时状态替换，**不是压缩预检，也不对应 `NeedsCompaction`**（`resources/pi/packages/agent/src/types.ts`；调用点 `resources/pi/packages/agent/src/agent-loop.ts`）。Pixbot 的 M1 `prepare_request` 阈值预检、`declare_tool_changes` 工具差量回放与 `microcompact` 投影清理均为自身设计，与上述 pi 回调语义不同；压缩策略可参考 `resources/pi/packages/coding-agent/src/core/compaction/` 与 `resources/pi/packages/coding-agent/src/core/session-manager.ts` 的压缩条目语义。
