# 文档工程指南

> 本文件只负责 `docs/` 的目录职责、唯一来源、写作格式和维护导航。项目规则与 DoD 以 [AGENTS.md](../AGENTS.md) 为唯一权威来源，本文件不复制其清单。

## 目录职责与唯一来源

| 路径 | 唯一职责 |
|---|---|
| [architecture.md](architecture.md) | 全局技术架构、跨领域约束与领域设计导航 |
| [design/](architecture.md#领域设计导航) | Agent、上下文、会话、工具、安全与 Provider 的现行目标设计 |
| [product.md](product.md) | 产品定位与边界 |
| [roadmap.md](roadmap.md) | 里程碑、版本范围、出口标准与产品迭代节奏 |
| [development.md](development.md) | 开发、测试和发布验收的操作细则 |
| [api/](api/README.md) | Rust 契约导航与必要说明，不复制类型全文 |
| [decisions.md](decisions.md) | 追加式 ADR 与长期技术取舍 |
| [changelog.md](changelog.md) | 用户可感知变化、升级说明与实际发布记录 |
| [runbook.md](runbook.md) | 已验证的运行和排障操作 |
| `docs/README.md` | 本文档工程规则与导航 |

单次工作文档位于 `docs/` 之外：[工作文档索引](../.agents/README.md) 统一导航 Spec、Plan 和 Issue，[Memory 编写规范与模板](../.agents/memory/README.md) 规定每日文件的组织、任务块模板与内容边界，[TODO](../.agents/todo.md) 只保存尚未立项的行动。

信息需要复用时链接到上述唯一来源，不另贴正文。当前已实现的行为以 Rust 代码和测试为准；目标／规划以架构总览和相关领域设计为准；pi 快照实际行为以 `resources/pi/` 源码为唯一事实来源，`resources/pi-architecture.md` 是对齐用的出处索引；完成后的 Spec 不得反向充当现行架构、接口或行为依据。

### 单一权威硬约束

1. **单句事实只出现一次**：同一句事实（当前实现状态、pi 事实口径、非目标、字段定义、规则条款）只在一份文档写成正文，其余位置用链接或行内标记引用。发现第二份正文时改写后者，保留前者。
2. **篇幅受控**：单份文档不超过 450 行，超限先拆分或裁剪重复段。篇幅增长必须由新的可判定内容解释，不能由复述解释；`resources/pi-architecture.md` 是出处索引，只允许替换与勘误，不再追加章节。
3. **删除文件前先查入站引用**：`grep -rn "<文件名>" --include="*.md" .` 归零后才可删除，否则先改引用；新增文档必须在上表登记职责，否则视为无主文档。

## 写作与命名

- 正文使用简体中文；命令、类型和标识符保留原文并使用行内代码。
- 明确区分「当前已实现」「目标／规划」「pi 快照实际行为」三类事实，并用「已验证／未执行」标注验收状态；未实现或未测量的内容不得写成事实。
- 文档文件使用小写 kebab-case `.md`；目录导航固定为 `README.md`；工作文档分别命名为 `YYYYMMDDHH-design-<topic>.md`、`YYYYMMDDHH-plan-<topic>.md`、`YYYYMMDDHH-issue-<topic>.md`，时间前缀使用本地首次创建小时，Plan 沿用 Spec 的完整前缀和 topic。
- ADR 编号从 `ADR-001` 起连续编排，新增条目在末尾续编，既有编号不重排、不复用于不同决策。新条目追加到 [decisions.md](decisions.md)。CHANGELOG 只记录用户可感知变化、破坏性变更、升级步骤和实际发布内容。
- 新能力、设计取舍、跨模块协调、公共契约或高风险改动先确认 Spec；Spec 可独立完成，多阶段、跨会话或高风险实施才创建 Plan。局部修正和 bug 使用 Issue，可包含实现、回归测试和必要文档；复杂问题升级为 Spec，按需增加 Plan。完整门槛以 AGENTS.md 为准。
- TODO 只记录尚未创建 Spec 或 Issue 的可执行行动，立项后移除对应条目，不保留完成历史。

## 格式

- 每份文档只有一个 H1，标题逐级递进，不跳级。
- 代码围栏标注语言并正确闭合；表格用于结构化对照，步骤使用列表。
- 文档间使用相对链接，路径大小写与磁盘一致；显式锚点必须对应目标标题。
- 引用代码使用仓库根目录相对路径，例如 `apps/crates/contracts/src/message.rs`，不粘贴大段源码；不新增指向 `resources/pi/` 具体源码文件的 Markdown 相对链接，核对依据以行内代码标注源码相对路径。
- ADR 和已发布 CHANGELOG 条目追加维护，不改写既有决策语义；字段统一、去重或合并等结构整合必须留下编号映射与整合记录；修正未发布草稿时应保持事实可追溯。

```markdown
[架构总览](architecture.md)
[项目规则](../AGENTS.md)
[Specs 编写规范](../.agents/specs/README.md)
```

## 维护流程

1. 改动前确认权威来源、任务边界和需要同步的导航。
2. 改动中只更新受影响文档；长期取舍写入 ADR，可复用规则回写 AGENTS.md，排障结论写入 runbook。
3. 里程碑收尾检查文档是否与代码一致、是否产生重复权威来源、链接是否仍有效。
4. 完成后保留 Spec、可选 Plan 或 Issue 作为历史记录，将每日 memory 的任务块转换为摘要与经验索引并清理中间态；TODO 条目在立项时移除，不保存完成历史。

## 验收导航

- 完整完成定义只查 [AGENTS.md 的“完成定义（DoD）”](../AGENTS.md)。
- 开发、测试和发布操作判定查 [development.md](development.md)。
- 工作文档模板分别查 [Specs](../.agents/specs/README.md)、[Plans](../.agents/plans/README.md) 和 [Issues](../.agents/issues/README.md)。
- 纯文档改动至少实际检查 Markdown 相对链接与显式锚点、路径大小写、单 H1、标题层级、代码围栏，并在仓库根目录执行：

```bash
git diff --check
```

未运行代码测试时，必须在交付或提交说明中明确写出“未运行代码测试”；链接和 diff 检查不能替代 Cargo 测试，也不能把未执行的检查报告为通过。
