# AGENTS.md

> 本文件是 Agent 的唯一权威规则入口。`CLAUDE.md` 只引用本文件，不复制规则。

## 项目是什么

Pixbot 是面向开发者的本地优先、单二进制 Rust 编码 Agent，以“模型 ⇄ 工具”决策循环完成读、写、执行和检索等编码任务。

## 权威来源与阅读顺序

1. 规则与 DoD：本文件。
2. 目标技术设计：`docs/architecture.md` 及当前任务相关的 `docs/design/` 领域设计。
3. 里程碑与出口标准：`docs/roadmap.md`。
4. 产品定位、取舍与非目标：`docs/product.md`；涉及范围扩张、非目标判定或产品取舍时先读。
5. pi 行为语义：`resources/pi-architecture.md`；`resources/pi/` 是固定参考源码快照。
6. 当前行为：Rust 代码与测试；历史 Spec、Plan 和 Issue 只说明当时的设计、执行或问题记录。
7. 任务恢复：开工先读 `.agents/memory/README.md`，并查找每日文件中的未完成任务块；继续当前或中断任务时，先按本文件的恢复流程核对实际状态。
8. 开工前：涉及新能力、设计取舍、跨模块协调、公共契约或高风险改动时，先读 `.agents/specs/` 中对应 Spec；没有则先创建。单纯跨文件、机械重命名或链接修正不强制创建 Spec，局部修正和 bug 记录位于 `.agents/issues/`。

## 目录职责

- `apps/crates/contracts/` — 中立 DTO、端口 trait、事件协议与纯函数。
- `apps/crates/cli/` — 参数解析、前端选择与退出码。
- `apps/crates/<name>/` — M0 目标 crate 由 `apps/Cargo.toml` 的 `members = ["crates/*"]` 自动收纳。
- `apps/target/` — Cargo 生成物，不编辑、不提交。
- `.agents/README.md` — 工作文档统一导航和 Spec、Plan、Issue 唯一实例索引；索引不记录正文状态。
- `.agents/specs/` — 设计规格、生成规范与模板；完成后的 Spec 是历史记录。
- `.agents/plans/` — 已确认 Spec 的可选实施计划、生成规范与模板；Plan 不重新定义范围，只在步骤存在明显依赖、需要跨会话实施或高风险操作必须逐步核对时保存步骤级执行历史。
- `.agents/issues/` — 边界清晰、低风险的局部修正和 bug 记录、生成规范与模板；可包含实现、回归测试和必要文档，复杂问题升级为 Spec，按需增加 Plan。
- `.agents/todo.md` — 尚未创建 Spec 或 Issue 的可执行行动；立项后移除，不保留完成历史。
- `.agents/memory/` — 按任务开始日期保存的每日任务记录；未完成时保存恢复中间态，完成后原地转换为摘要与经验索引，编写规范见 `.agents/memory/README.md`。
- `docs/api/` — 契约导航与必要说明，不重复维护 Rust 类型全文。
- `resources/pi/` — 第三方参考源码快照，不是 pixbot 实现代码。
- `docs/architecture.md` — 全局技术架构与领域导航。
- `docs/design/` — Agent、上下文、会话、工具、安全与 Provider 六份现行目标领域设计。

## 命令

所有 Cargo 命令的工作目录均为 `apps/`：

```bash
cd apps
```

- 构建：`cargo build --workspace`
- 测试：`cargo test --workspace`（提交前必须全绿）
- 格式化：`cargo fmt --all`
- 格式检查：`cargo fmt --all -- --check`
- lint：`cargo clippy --workspace --all-targets -- -D warnings`
- 依赖守卫：`../scripts/check-layering.sh --root ..`；完整正负例自测追加 `--self-test`
- CLI 单发：`cargo run -p cli -- -p "你好"`

## 代码风格

- 使用 Rust 2024；工具链最低版本为 Rust 1.93。
- 依赖版本只在 `apps/Cargo.toml` 的 `[workspace.dependencies]` 声明，crate 内使用 `{ workspace = true }`。
- 跨 crate 通信只经 `contracts` 的端口与 DTO；依赖方向为 `L5 → L4 → L3 → L1/L2 → L0`，L1/L2 层内禁呼。
- lib 代码零 `unwrap` / `expect` / `panic!`；锁中毒用 `unwrap_or_else(|e| e.into_inner())` 恢复。
- 每 crate 使用一个 `thiserror` 错误枚举；`anyhow` 只用于 `cli` / `bootstrap` 边界。
- 持久化模型新增字段必须使用 `#[serde(default)]`，保持旧 JSONL 可读；转录只追加，不原地改写。
- 测试替身由各 crate 私有维护，不放入生产 `contracts`，不建立共享 `testkit`。

## 禁止事项

- 不得修改 `apps/target/` 等生成物；未经明确要求不得改动 `resources/pi/` 参考快照。
- 不得引入新依赖，除非先更新 `docs/architecture.md` 的外部依赖清单；形成长期取舍时同步决策记录。
- 不得在 L1/L2 crate 间建立直接依赖，不得绕过 `contracts` 依赖其他 crate 的具体实现。
- 不得新增逐工具权限／审批子系统、repl、共享 `testkit`、运行时脚本嵌入或 pi 的 codemode / durable / env / chord / telemetry 产品线。
- 不得把 M0 目标 crate、provider、工具或性能目标写成已经实现或已经达到的事实。
- 不得提交密钥、token、真实用户数据或本地会话文件。

## 提交与分支

- 单人开发可在主干小步提交；一次提交只做一件可说明的事。
- 提交信息沿用 Conventional Commits，例如 `docs: 初始化文档工程结构`。
- 只有用户明确要求时才执行 commit 或 push。
- 发布后采用 SemVer 与 `vX.Y.Z` 标签；每次发布核对并更新 `docs/changelog.md`。

## 改动流程

1. 阅读 `docs/architecture.md`、当前任务相关的 `docs/design/` 领域设计、`docs/roadmap.md` 对应内容和当前 Spec 或 Issue；涉及范围扩张、非目标判定或产品取舍时同时阅读 `docs/product.md`。
2. 开工先读 `.agents/memory/README.md`，并查找所有每日文件中状态为“进行中／中断／待核对”的任务块，不能只读取当天文件。恢复当前或中断任务时，将开始日期文件中的快照与关联工作文档、实际文件和 Git 工作树对账；快照过期、目标缺失或状态冲突时先修正记录，不盲目续跑或重放命令。
3. 只改 Spec 或 Issue 范围内的文件；需要扩大范围时先停下来说明原因，并更新对应契约。
4. 新能力、设计取舍、跨模块协调、公共契约或高风险改动，先在 `.agents/specs/` 创建并确认 Spec。Spec 可以独立完成；只有步骤存在明显依赖、需要跨会话实施或高风险操作必须逐步核对时，才在 `.agents/plans/` 创建关联 Plan，并按 Plan 实施和验证。单纯跨文件、机械重命名或链接修正不强制创建 Spec。
5. 边界清晰、低风险的局部修正和局部 bug 使用 `.agents/issues/` 中的 Issue，可以同步修改实现、回归测试和必要文档，默认不创建 Plan；需要跨模块协调、改变公共契约、多阶段协调或范围风险扩大时，保留 Issue 并升级为 Spec，按需增加 Plan，不允许 Issue 直接关联 Plan。
6. 创建 Spec、Plan 或 Issue 时只在 `.agents/README.md` 添加标题和相对链接，不在索引复制状态或进度；完成时只更新对应正文和收尾记录，不再修改索引。
7. `.agents/todo.md` 只记录尚未立项的可执行行动；创建 Spec 或 Issue 后移除对应条目，不保留完成历史。已经明确的工作可直接创建立项文档，不必先写 TODO。
8. 每个实际开始的任务都在开始日期的 `.agents/memory/YYYY-MM-DD.md` 中建立一个任务块，跨天不复制或移动。任务开始、重要副作用操作前、获知异常或中止以及收尾时更新本任务快照并保留其他任务；关键步骤仅在恢复游标、下一步或未核对改动发生实质变化时更新。同一检查点的多个触发原因合并为一次写入，只有时间或措辞变化时不写，跨天本身不触发重写；简单无额外副作用的单会话任务通常只写开始与收尾，但不得省略必要快照。未完成时核心记录恢复游标、下一步、未核对改动和恢复核对项，中断信息按条件记录，不复制 Plan 步骤表、TODO、Spec 背景或完整日志，不记录密钥、真实用户数据或本地会话正文。memory 是最后已知状态，不保证捕获硬中断后的变化。
9. 每个代码改动必须附带或更新测试；能自动化的回归检查写成可执行测试。
10. 收尾时按变化更新架构、契约、决策或运行文档，并回写 Spec、Plan 或 Issue 的实际验收结果；CHANGELOG 只记录用户可感知变化与发布内容。正常完成后将本任务块原地转换为任务摘要、验证摘要和经验索引，删除失效中间态并保留其他任务；已关闭任务记录原因且不得写成已完成，未完成、待核对或中断任务保留恢复信息，不得清理为已完成。
11. 任务结束后把可复用规则回写本文件，长期技术取舍追加 `docs/decisions.md`，已复现的排障结论写入 `docs/runbook.md`。

## 完成定义（DoD）

- [ ] `cargo test --workspace` 全绿。
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 零告警。
- [ ] `cargo fmt --all -- --check` 无 diff。
- [ ] 正常、边界与异常路径均有覆盖；核心契约异常路径包含结果回填断言。
- [ ] 当前行为、目标设计与验收结果没有混写。
- [ ] 相关文档与链接已同步；无重复权威来源（单句事实只写一次与篇幅上限见 `docs/README.md`）。
- [ ] 工作文档状态与 memory 一致；已完成任务只保留摘要、验证摘要和经验索引，无失效中间态，未完成任务保留下一步与恢复核对项。
- [ ] 纯文档改动至少通过 Markdown 相对链接检查（确认无断链与大小写不匹配）与 `git diff --check`，并明确说明未运行代码测试。
