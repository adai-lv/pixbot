# 会话与持久化设计

> 本文件只权威定义会话条目、JSONL 布局、append-only 写入、加载容错和独立用量台账，不复制上下文投影或压缩算法。以下均为目标设计，实际行为以 Rust 代码与测试为准。

## 条目分期

| 阶段 | 持久化目标 |
|---|---|
| M0 | 基础 `TranscriptEntry::Message(Message)`；当前会话按 JSONL 线性追加和读取，不支持 resume/fork、分支导航或完整树投影 |
| M1 | 在同一 tagged union 追加 `Compaction`、`Label`、`ContextEdit`；支持分支、fork 与当前叶子的树投影 |

M0 消息仍保留 `uuid` / `parent_uuid` 以稳定标识事实，但不据此宣称已实现树能力；这两个字段是 Pixbot `apps/crates/contracts/src/message.rs` 的当前 `Message` 字段。M1 Compaction 保存摘要、`first_kept_uuid` 和 `tokens_before`（Pixbot snake_case）；ContextEdit 以追加条目表达遮蔽，原消息永不删除。投影与压缩规则由[上下文设计](context.md)承载。pi 快照实际行为：pi 会话条目字段是 `id` / `parentId`（pi 标识符，**不是** `uuid` / `parent_uuid`），压缩条目参数为 `firstKeptEntryId` / `tokensBefore`（`resources/pi/packages/coding-agent/src/core/session-manager.ts`），两者不得混写。

`UsageRecord` 写入独立用量台账，**不是** `TranscriptEntry` 变体，也不与会话消息混写。token、cost 等字段沿用 contracts 的中立 Usage DTO；记录失败只告警，不影响回合结果。以上为 Pixbot 自身设计。

## 目标布局

| 数据 | 路径 | 责任边界 |
|---|---|---|
| 会话转录 | `~/.pixbot/sessions/<sid>/context.jsonl`；M1 fork 使用 `fork-<n>.jsonl` | `session` |
| 用量台账 | `~/.pixbot/usage-ledger.jsonl` | `session` 的 usage 模块 |
| 工具大输出 | `~/.pixbot/sessions/<sid>/tool-results/` | **Pixbot 目标布局，非 pi 行为，当前未创建**；`tools` 生成，`session` 提供位置约定 |
| 配置 | M0 为 `~/.pixbot/settings.toml` 与显式 CLI 配置；M2 才增加受 project trust 控制的 `<cwd>/.pixbot/settings.toml` / `settings.local.toml` | `config` |

`~/.pixbot/...` 路径均为 Pixbot 目标布局，这些路径不代表当前已经创建文件；真实用户数据与本地会话文件不得提交。pi 快照实际行为：pi 的大输出不写入项目内 `tool-results/` 目录，而是落在 OS `tmpdir()` 下 `pi-*.log` 临时文件、权限 `0o600`（`resources/pi/packages/coding-agent/src/utils/output-files.ts`）。

## 写入与加载纪律

以下六条为 **Pixbot 目标设计**，pi 与之不同的行为见本节末对照：

1. **惰性建文件**：首条 user/assistant 消息出现前不创建空会话文件。
2. **append-only**：不原地改写；M1 branch 通过新 `parent_uuid` 追加，fork 新建文件。
3. **坏行容错**：加载时跳过无法解析的行并记录诊断，不阻断其余有效条目。
4. **孤儿处理**：缺父条目的记录保留在原文件中并标记诊断，不伪造父链、不纳入当前投影。
5. **防抖 writer**：100ms 窗口，且 1s 或累计 100 行时强制 flush；退出时再次强制写并等待完成。
6. **写盘 fail-open**：追加、flush 或台账记录失败只产生 warn/Notice，不阻断模型与工具循环，也不回滚内存转录。

防抖 writer（100ms / 1s / 100 行）、独立 `UsageRecord` 台账、`~/.pixbot/...` 路径、坏行／孤儿处理与「写盘 fail-open」均为 **Pixbot 目标设计／相对 pi 的显式差异**。防抖是 Pixbot 相对 pi 同步逐条写盘的明确差异：它降低高频写入开销，但扩大崩溃时未落盘窗口，因此退出 flush 与故障告警不可省略。新增持久化字段必须 `#[serde(default)]`，重复加载不得改写或重排原条目。

pi 快照实际行为：会话 JSONL 坏行**静默跳过、不记录诊断**（`parseSessionEntries` / `parseSessionEntryLine`），缺父条目（孤儿）在 `getTree()` 中**作为 root 返回**（不丢弃、不标记诊断）；首条 `user`/`assistant` 消息前惰性不建文件（`_hasConversation()` / `openSync(...,"wx")`），其后 `appendFileSync` 同步逐条追加；**写失败时回滚内存并 `throw`（fail-closed），不是 fail-open**（`_appendEntry` 的 catch 回滚 `fileEntries` / `byId` / `leafId`）；压缩条目 `appendCompaction(summary, firstKeptEntryId, tokensBefore, details?, fromHook?, usage?)`（pi 标识符）——以上均见 `resources/pi/packages/coding-agent/src/core/session-manager.ts`。因此「坏行记录诊断」与「写盘 fail-open」是 Pixbot 相对 pi 的显式差异，二者的语义与 pi 行为并不相同。pi 的 `loadEntriesFromFile()` 在加载时若文件末行缺换行会 `appendFileSync(filePath, "\n")` 修文件（`resources/pi/packages/coding-agent/src/core/session-manager.ts:661-668`），即 pi 的读路径并非纯只读；Pixbot 的加载路径应保持只读、不修文件，坏行跳过并记诊断（可作为 M0 加载用例）。

## pi 对齐索引

事实出处均在 `resources/pi/` 源码。pi 的 `SessionManager`（pi 标识符，`resources/pi/packages/coding-agent/src/core/session-manager.ts`）提供条目树、惰性落盘和压缩条目的参考语义（条目字段 `id` / `parentId`，`branch()` / `forkFrom()` 分支能力）；Pixbot 保留 append-only 与容错原则，并以独立 `UsageRecord` 台账、防抖 writer 与写盘 fail-open 作为显式差异。本文不维护第二份逐行映射。
