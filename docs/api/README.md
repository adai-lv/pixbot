# 接口与数据契约

> 本目录只提供契约导航，不复制维护 Rust 类型全文。已实现契约以代码、serde 属性与测试为唯一权威；目标契约的索引统一在[架构总览的领域设计导航](../architecture.md#领域设计导航)，本文件不重复。

## 当前权威来源

| 契约 | 代码位置 | 当前范围 |
|---|---|---|
| 消息模型 | [message.rs](../../apps/crates/contracts/src/message.rs) | `Role`、`Content`、`Message`、system 构造及 serde 兼容行为 |
| 转录条目 | [transcript.rs](../../apps/crates/contracts/src/transcript.rs) | 基础 `TranscriptEntry::Message` 及精确 JSON；不含树、压缩或文件加载 |
| 用量 | [usage.rs](../../apps/crates/contracts/src/usage.rs) | `Usage` 计量 DTO；不含台账或计费 |
| 工具契约 | [tool.rs](../../apps/crates/contracts/src/tool.rs) | 工具结果、定义、上下文与端口；不含工具执行适配器 |
| 模型与请求 | [model.rs](../../apps/crates/contracts/src/model.rs)、[request.rs](../../apps/crates/contracts/src/request.rs) | 拥有型请求、wire DTO、流事件与模型端口；不含 transport |
| Agent 事件与端口 | [event.rs](../../apps/crates/contracts/src/event.rs) | 事件、结果、两个纯构造器与同步事件端口；不含运行循环 |
| 上下文与会话端口 | [ports.rs](../../apps/crates/contracts/src/ports.rs) | 投影、会话及用量端口；不含算法或持久化实现 |
| 中立错误 | [error.rs](../../apps/crates/contracts/src/error.rs) | `ContractError`、领域分类与重试／中止判定 |
| 配置快照 | [config.rs](../../apps/crates/contracts/src/config.rs) | 进程内净化快照；不读取环境或配置文件 |
| CLI 参数 | [cmd.rs](../../apps/crates/cli/src/cmd.rs) | `-p/--prompt`、`-r/--repl`、`-u/--upgrade` 三个参数 |
| CLI 执行行为 | [plan.rs](../../apps/crates/cli/src/plan.rs) | 打印模式名或 `HEADLESS: <prompt>`、`UPGRADE: <version>` 与一行耗时 |
| workspace 配置 | [apps/Cargo.toml](../../apps/Cargo.toml) | 已声明依赖、lint、工具链与 release 配置 |

目标设计已取消 repl，`cli` 中的该参数不得据此扩展；`-u` 的升级语义以发布规格为准。三个参数中 `-p` / `-u` 接受字符串值、`-r` 为布尔开关，且三者互斥。

## 变更纪律

1. 修改已实现契约时，先改 Rust 类型与序列化／契约测试，再更新本索引与任务规格。
2. 持久化新增字段必须 `#[serde(default)]`；新增枚举变体必须验证旧 JSONL 可读。
3. 领域消息只在 `ContextEngine::to_wire` 降级，provider 不感知扩展变体。
4. 数据或接口迁移、回滚与影响面写入任务规格；涉及运行操作时同步[运行手册](../runbook.md)。
5. 若改用代码生成契约文档，只能保留一个生成源，并在此记录生成命令与一致性检查。
