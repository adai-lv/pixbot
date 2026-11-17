# 安全与扩展边界设计

> 本文件只权威定义无内建权限子系统、扩展接入点、通用 hook、sandbox、project trust 与配置密钥边界。M2 接入点均为后续目标，不得描述为现状。执行细节见[工具设计](tools.md)，认证细节见[Provider 设计](providers.md)。

## 威胁模型与内核边界

模型可能生成危险命令、写系统路径或外发密钥；工具默认继承 Pixbot 进程的 OS 权限。项目配置、扩展、MCP 和技能也可能被恶意仓库投毒。Pixbot 不内置逐工具权限/审批或独立 permission 子系统，外部通用 hook 与 OS sandbox 才承接约束——这是 **Pixbot 自身产品取舍**：逐工具审批会侵入 `driver` 控制流，并在首个闭环前预付控制流成本；需要审批的团队可在 hooks 中自行实现，内核不必内置权限模式。pi 快照实际行为：`resources/pi/packages/` 全库未发现专门 permission subsystem。

**安全策略不进入 Agent 内核**。任何阶段都不在 driver 中增加审批分支；M2 的通用 hook 适配也只位于 `agent::dispatch`。before/after 的执行顺序、改参后的最终 schema 校验和三通道字段白名单统一以[工具执行流水线](tools.md#执行流水线)为准，本文不复制第二份流程。

before hook 缺省继续，可修改 input，或返回 block/reason/terminate；block 由 dispatch 生成唯一 `is_error=true` 结果，不进入 execute。可变 input 属于扩展适配层，不给 core 增加 `Modified(args)` 返回变体。dispatch 始终保留配对、取消、双序和错误回填控制。以上是 **Pixbot M2 扩展适配层设计**；pi 快照实际行为：`beforeToolCall` 仅返回 block/reason/terminate、**不修改参数**，且参数校验在 hook 之前（`resources/pi/packages/agent/src/types.ts` 的 `BeforeToolCallResult`）。「改参后最终校验」与「after 三通道白名单」是 **Pixbot 相对 pi 的显式收紧**：pi 的 `AfterToolCallResult`（pi 标识符）可逐字段覆盖 `isError` / `usage` / `terminate`，`tool_execution_end` 带外层 `isError`（`resources/pi/packages/agent/src/types.ts`）。

## 扩展接入点总览

| 扩展面 | 机制 | 接入点 | 明确不做 |
|---|---|---|---|
| 编译期端口 | `contracts` 中的 trait 与 DTO，由 `bootstrap` 注入实现 | 内核与各适配器之间 | 内核直接依赖具体实现 |
| 通用 hooks | 进程外扩展；before／after 结果由 `dispatch` 适配 | `agent::dispatch` | 权限三态、审批端口或内核策略表 |
| MCP | 进程外协议客户端；外部工具作为来源 | `ToolSet` | 改变内核循环；嵌入脚本运行时 |
| skills／context files | 提示资源；skills 受 project trust 控制，context files 随工作目录加载 | `config` 加载为快照，`context` 在 system 装配时使用 | 执行任意代码；项目 skills 绕过 trust |
| OS sandbox | 在 bash／子进程执行前应用隔离 | `tools` 的子进程边界 | 伪装成逐工具审批 |
| project trust | 资源加载门；未信任时不加载项目级配置、扩展、MCP 与 skills | `config`／资源加载 | 判断某个路径能否写，进入 `dispatch` |
| 后台任务 | 工具／会话边界的长期执行单元 | 输出落盘，终态以 `is_meta` 通知回流 | 直接回灌主上下文或进入内核控制流 |

外部开发者可用 hooks 自行实现审批、审计或策略，但 Pixbot 不提供内建权限模式；hook 失败、MCP 不可达或资源加载失败必须产生可诊断结果，不得静默伪装成功。无 hook 时行为与 M0 一致；hook 测试只要求通用继续、改参后校验、阻断、after 白名单与 terminate 语义。

`hooks` / `sandbox` crate 是 **Pixbot 自建的分期目标，不是 pi 能力平移**：pi 无内建 OS sandbox 实现，官方立场是把整个进程放进容器／VM（`resources/pi/packages/coding-agent/docs/{security,containerization}.md`）；`resources/pi/packages/coding-agent/src/bun/sandbox-env-setup.ts` 仅 4 行，是 Bun 编译产物在外部 sandbox 内恢复 `process.env` 的 workaround（配套 `restore-sandbox-env.ts`），**不得作为 pi 有 sandbox 的证据**。pi 快照实际行为中与 trust 相关的文件只有 `project-trust.ts`（默认 `ask`、无 UI 时返回 false）、`trust-manager.ts` 与 `resource-loader.ts`（均在 `resources/pi/packages/coding-agent/src/core/`）。

## 配置与密钥纪律

M0 配置支持 `${VAR}` 插值和显式/ambient provider 凭据；显式选择的 provider 缺少必需凭据时返回可诊断错误，不要求所有 provider 都有静态密钥。pi 快照实际行为：配置值支持 `${VAR}` / `$VAR` / `!command`（`resources/pi/packages/coding-agent/src/core/resolve-config-value.ts` 的 `ENV_VAR_NAME_RE`）。

M2 配置文件写回采用 tmp+rename、0600 与 fsync——这是 **Pixbot M2 目标设计，pi 源码未体现，不得写成 pi 行为或已实现**；pi 快照实际行为：`settings-manager.ts` 以 `writeFileSync` 直接写配置（未见 tmp+rename/fsync），`auth-storage.ts` 写凭据用 `mode:0o600`。项目资源按显式覆盖、已存决策和默认策略处理 trust，TUI 的 trust 提示也不是工具审批。

默认无遥测、不联网上报，不提交密钥、真实用户数据或本地会话——这是 **Pixbot 自身产品决策**，与 pi 的默认开关不同。pi 快照实际行为：默认 `enableInstallTelemetry:true` / `enableAnalytics:false`，可由 `PI_TELEMETRY` 覆盖（`resources/pi/packages/coding-agent/src/core/{settings-defaults,telemetry}.ts`）。凭据解析优先级、OAuth 与 ambient auth 以[Provider 设计](providers.md)为准。

## pi 对齐索引

pi 的实际行为以 `resources/pi/` 源码为唯一事实来源。本设计参考 pi 的「无专门权限系统、无内建 OS 隔离、建议整进程容器化」立场，该取舍本身是 **Pixbot 自身产品决策**；`sandbox` crate 属自建增强。两处显式收紧（before 改参后必须最终校验、after 只能修改三通道内容字段）是 **Pixbot 相对 pi 的差异**。本文不维护逐行映射。
