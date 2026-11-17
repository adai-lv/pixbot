# 产品定义

> 本文件只定义 Pixbot 的产品定位、用户、取舍与非目标、成功判据与需求依据；技术语义见[架构](architecture.md)，版本与出口见[路线图](roadmap.md)，使用与运行形态见 [README](../README.md)，文档分工见[文档工程指南](README.md)。除「当前已实现」外均为**目标／规划**。

## 定位与用户

Pixbot 是面向本机开发者的终端 AI 编码 Agent：用自然语言下达编码任务，由“模型 ⇄ 工具”循环完成定位、读取、修改、执行与检索。目标用户熟悉命令行与版本控制，既要在交互场景中把机械性改动交给 Agent，也要求同一套能力能在 CI 中以非交互单发方式运行。

三条价值主张：

1. **能真实干活**：价值不在聊天，而在模型能驱动工具读写文件、执行命令并检索代码，形成可验证的编码闭环。
2. **本地优先**：代码、凭据与会话默认留在本机，除模型请求外不依赖托管服务；用量只计量，不内置计费或限额判定。
3. **可掌握、可交付**：内核小而可审计，交付形态是单一静态二进制，可在无额外语言运行时的环境中运行。

需要同时说清两条边界：本地优先不意味着模型请求必然离线；零额外语言运行时也不等于零外部工具，`rg`、`fd`、shell 的获取、诊断与降级以[工具设计](design/tools.md)为准。

**不以本产品为目标的对象**：需要图形界面或 IDE 深度集成的用户；需要内建逐工具审批、审计与多租户隔离的组织（这类约束应在 hooks 与容器化中自行实现，不是产品内建能力）；需要云端托管编码服务或多人协作服务端的场景。

## 行为蓝本与技术栈

Pixbot 以**固定 pi 快照**为行为蓝本，继承其运行语义而自建实现：不移植 pi 的 TypeScript 代码，也不自动跟踪上游新增能力——只有这样“对齐完成”才有可判定的终止条件。逐项对照见[架构的对标取舍](architecture.md#对标-pi-的取舍)。

选择 Rust 2024、单一静态二进制与零额外语言运行时是产品决策而非实现偏好：它让 Pixbot 能直接进入 CI 与非交互流水线，并在终端中长期驻留。代价是实现成本高于脚本方案，因此以“固定蓝本 + 分期切片”控制范围；Rust 也不构成对未实测性能的宣称。

## 产品取舍

| 维度 | 产品选择 | 理由 |
|---|---|---|
| 安全 | 不内置逐工具权限／审批；通用 hooks、OS sandbox 与 project trust 后置 | 权限策略因团队而异，应由扩展与执行边界承接，而不是进入内核控制流 |
| 扩展 | 编译期端口 + 进程外协议；不嵌入 JS/TS 等语言运行时 | 避免同时引入额外运行时与更宽的权限面 |
| 前端 | headless 先行、TUI 后置，共用 `AgentSession`；不做 repl | repl 会引入第二条驱动路径与独立运行状态，破坏“前端只消费事件流” |
| 会话 | M0 基础 `Message` 线性追加；后续增加恢复、分叉与压缩，历史不原地改写 | 追加式模型让恢复、分叉与审计都退化为新条目 |
| provider / tools | 对齐固定 pi 快照，但 M0 只交付首批切片 | 首批实现是验证闭环的最小集合，不是全量兼容的证明 |
| 结束语义 | 前端消费 `AgentEvent` 流并取得 `run()` 返回的 `RunOutcome`，无需第二个结束点 | 结束点唯一，避免前端等待两套收敛信号 |
| 计量 | 只记录用量，不内置计费与限额判定 | 账单规则随厂商变化，产品不替用户做计费决策 |

能力设计与分期只查对应文档：领域语义见[架构的领域设计导航](architecture.md#领域设计导航)，M0 范围与出口见[路线图](roadmap.md)。M0 不包含压缩、工具差量、完整树投影与通用扩展。

## 成功判据

**M0**：

- 按[路线图 M0 smoke](roadmap.md#5-m0-总体边界)从 `apps/` 读取 `../README.md` 并完成真实工具任务。
- 配对、截断、错误回填、中立请求、取消与结束语义有回归覆盖；六个工具达到 M0 最低行为。
- headless 能给出可诊断退出码，基础消息线性落盘失败不阻断回合。

**后续**：

- TUI 与 headless 复用同一内核，恢复／分叉不改写历史。
- 按固定 pi 快照完成 provider 与 tool 行为对齐，不用同名或单次 smoke 宣称全量兼容。
- 性能首次测量建立基线；未实测前不得宣称达标。

判断口径同样重要：设计文档已描述、会话文件已生成、单个 smoke 通过或六个工具可用，都不能证明能力已实现、pi 全量兼容或性能达标。标签与验收状态只以代码、测试和[路线图出口](roadmap.md)为准。

## 需求与追溯

没有书面 PRD。下表标签只说明需求依据：`[Confirmed]` 为已确认产品约束，`[Data-backed]` 为可核对的参考行为，`[Hypothesis]` 为待验证假定，其余为 `[Expert judgment]`；**标签不是当前能力已实现的证据**。pi 快照实际行为以 `resources/pi/` 源码为唯一事实来源。

| # | 需求 | 真实依据 | 标签 |
|---|---|---|---|
| R1 | 单二进制、零额外语言运行时，可在 CI 非交互运行 | [README 产品目标](../README.md)、[AGENTS 项目定义](../AGENTS.md)、[workspace release 配置](../apps/Cargo.toml) | `[Confirmed]` |
| R2 | 不内置逐工具权限／审批子系统，通用约束后置（Pixbot 自身产品决策） | 已确认产品决策；[安全设计](design/security.md) | `[Confirmed]` |
| R3 | headless/TUI 共用内核，不做 repl | 已确认产品决策；[架构](architecture.md) | `[Confirmed]` |
| R4 | 后续会话可恢复、可分叉且历史不改写 | pi 追加式会话参考（`resources/pi/packages/coding-agent/src/core/session-manager.ts`）；[会话设计](design/session.md) | `[Data-backed]` |
| R5 | prompt cache 是长编码会话成本主项 | pi 缓存纪律与长会话形态；[上下文设计](design/context.md) | `[Hypothesis]` |
| R6 | 依赖方向由编译器和守卫强制 | [AGENTS 分层规则](../AGENTS.md)与[架构守卫设计](architecture.md#工程结构与分层) | `[Expert judgment]` |
| R7 | MCP/hooks 后置，不阻塞首个闭环 | [路线图](roadmap.md) | `[Expert judgment]` |
| R8 | providers/tools 最终对齐固定 pi 基线，分期交付 | 已确认产品决策；[Provider](design/providers.md)与[工具设计](design/tools.md) | `[Confirmed]` |
| R9 | 无共享 testkit，测试替身各 crate 私有 | [AGENTS 禁止事项](../AGENTS.md) | `[Confirmed]` |

## 明确不做

1. 不做子代理编排 DSL、plan mode 专用语义、多租户服务端、permission／approval 子系统或 repl。
2. 不嵌入运行时脚本语言，不照搬 pi 的 codemode／durable／env／chord／telemetry 产品线（这些包在 `resources/pi/packages/` 下确实存在，属于「明确不做」的对象）。
3. 不自动跟踪 pi 最新版本；终态只覆盖固定基线。
4. 不内置计费／限额判定，只计量；不建设共享 `testkit`。
5. M0～M2 不提前抽象 `fs`／`process`／`http` 平台 crate，按真实消费者演进。
