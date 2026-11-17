# Pixbot

Pixbot 是面向开发者的本地优先、单二进制 Rust 编码 Agent，目标是在命令行中通过“模型 ⇄ 工具”循环完成读写文件、执行命令和检索等软件任务。

## 环境要求

- Rust 1.93 或更高版本（workspace 使用 Edition 2024）。
- 构建与运行不需要额外系统依赖；后续 `grep` / `find` 等工具的外部二进制要求以[工具系统设计](docs/design/tools.md)为准。

## 构建与运行

```bash
cd apps
cargo build --workspace
cargo run -p cli -- -p "你好"
```

## 常用命令

> 命令以 [AGENTS.md](AGENTS.md) 的「命令」一节为权威来源；以下为 `apps/` 工作目录下的快速参考。

以下命令均在 `apps/` 下执行：

- 测试：`cargo test --workspace`
- 构建：`cargo build --workspace`
- 格式化：`cargo fmt --all`
- 格式检查：`cargo fmt --all -- --check`
- lint：`cargo clippy --workspace --all-targets -- -D warnings`

## 配置

规划中的配置与凭据边界见[安全设计](docs/design/security.md)和 [Provider 设计](docs/design/providers.md)；真实密钥不得写入仓库。

## 文档入口

- 项目规则与 DoD：[AGENTS.md](AGENTS.md)
- 文档工程指南与口径：[docs/README.md](docs/README.md)
- 产品定义：[docs/product.md](docs/product.md)
- 全局架构总览（含六份领域设计导航）：[docs/architecture.md](docs/architecture.md)
- 开发路线图：[docs/roadmap.md](docs/roadmap.md)
- 决策记录：[docs/decisions.md](docs/decisions.md)
- 开发操作规范：[docs/development.md](docs/development.md)
- 接口与数据契约：[docs/api/README.md](docs/api/README.md)
- pi 行为参考：[resources/pi-architecture.md](resources/pi-architecture.md)
- 工作文档：[索引](.agents/README.md)、[MEMORY 规范](.agents/memory/README.md)、[TODO](.agents/todo.md)
- 运行与发布：[docs/runbook.md](docs/runbook.md)、[docs/changelog.md](docs/changelog.md)

## 文档维护

新能力、设计取舍、跨模块协调、公共契约或高风险改动先确认 Spec；Spec 可独立完成，多阶段、跨会话或高风险实施才创建 Plan。边界清晰的局部修正和 bug 使用 Issue，可同步修改实现、回归测试和必要文档；复杂问题升级为 Spec，按需增加 Plan。TODO 只记录尚未立项的行动。结构变化更新架构总览与相关领域设计，长期技术取舍追加决策记录。详细规则见 [AGENTS.md](AGENTS.md)。
