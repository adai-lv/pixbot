# 0.1.0 工程与契约基线实施计划

> 状态：已完成。
> 关联规格：[0.1.0 工程与契约基线](../specs/2026111520-design-0-1-0-contract-baseline.md)。

## 实施策略

按 lint → 骨架 → 契约 → 守卫 → 独立评审 → 全量出口的顺序，在同一工作树顺序实施。SE 负责 P-01～P-04 及必要修复，TE 负责 P-05 和真实验证，QA 在 P-06 独立审查；每步先写或更新可失败的回归测试，再实现并运行对应检查。Plan 不改变 Spec 的类型形状、允许路径或验收标准。

## 实施准备

- 前置条件：Spec 与 `apps/Cargo.lock` 路径例外已获用户正式确认；2026-11-16 用户明确选择“允许同步两行”，批准 `docs/design/providers.md` 仅第 15～16 行改为拥有型 `CompletionRequest`，不改变其他 Provider 语义，不回退代码契约。任务已恢复，无额外环境或凭据要求。

## 实施步骤

### P-01 统一 lint 继承与测试窄豁免

- 状态：已完成
- 依赖：无
- 文件：修改 `apps/Cargo.toml`、`apps/crates/cli/Cargo.toml`、`apps/crates/contracts/Cargo.toml`、现有测试目标根部；后续新 crate 沿用同一配置。
- 操作：落地 Spec 指定的七项 clippy restriction lints，修正 cli 继承，并只对具体测试模块／目标添加实际需要的 unwrap／expect／panic 豁免。
- 验证：在 `apps/` 执行 `cargo clippy --workspace --all-targets -- -D warnings`；检查 manifest 继承和豁免位置。
- 预期：现有两个 crate 零告警，生产代码无宽泛豁免。
- 实际结果：七项 deny 与十个 manifest 继承已核对。SE 先启用 lint 后 clippy 以 101 命中既有测试 unwrap，仅在具体测试模块／目标添加窄豁免；PEL 最终独立复跑同一 clippy 退出 0。

### P-02 建立 10-crate 骨架与锁文件

- 状态：已完成
- 依赖：P-01
- 文件：新增 `apps/crates/{agent,context,providers,tools,session,config,bootstrap,headless}/`；修改既有 contracts／cli 职责文档和内部依赖；允许 Cargo 自动更新 `apps/Cargo.lock`。
- 操作：建立 Spec 精确指定的十个成员，只声明实际使用的依赖，不创建运行实现、noop 适配器或额外界面包。
- 验证：`cargo build --workspace`；结构化核对成员集合和内部依赖。
- 预期：十个指定 crate 可构建，锁文件只发生成员及内部边所需变化。
- 实际结果：PEL 的 `cargo build --workspace` 退出 0；`cargo metadata --locked --no-deps --format-version=1` 与静态清单断言确认精确十成员、内部边白名单、lint 继承和版本来源。锁文件仅由 Cargo 更新，包含新成员、内部边及激活 Spec 已允许的既有 workspace 外部声明和传递包，无新增或升级依赖声明。P-02 未单独留存实现前失败日志。

### P-03 落地基础数据、错误与配置契约

- 状态：已完成
- 依赖：P-02
- 文件：修改 `apps/crates/contracts/src/{lib,message}.rs`，新增 `transcript.rs`、`usage.rs`、`tool.rs`、`config.rs`、`error.rs` 及对应单元测试。
- 操作：先固定旧 payload、无效输入、精确 JSON 和默认值测试，再实现 Role::System、TranscriptEntry、Usage、ToolResult／ToolContext、ConfigSnapshot 和 ContractError。
- 验证：`cargo test -p contracts`，覆盖 AC-03、AC-04 及本步 AC-06 类型。
- 预期：新增测试在实现前因契约缺失而失败，实现后全部通过；无效 serde 输入不静默成功。
- 实际结果：SE 留存实现前 `cargo test -p contracts` 退出 101 的契约缺失预期失败；实现后本步 29 个单元测试通过。PEL 最终复跑 `cargo test -p contracts` 退出 0，共 29 个单元测试与 13 个集成测试通过，覆盖旧 payload、精确 JSON、默认值及无效输入。

### P-04 落地模型、事件与端口契约

- 状态：已完成
- 依赖：P-03
- 文件：新增 `apps/crates/contracts/src/{model,event,ports,request}.rs` 及 `apps/crates/contracts/tests/{event,model_request,ports}.rs`。
- 操作：先写拥有型请求、双轨内容、事件构造、错误分类、DTO 与端口线程边界测试，再实现 DTO、两个纯构造器和全部本版端口。
- 验证：`cargo test -p contracts`，覆盖 AC-05、AC-06；不建立 transport 或生产适配器。
- 预期：DTO／fake 可构造和匹配，端口对象满足 Send + Sync，纯函数输出确定且无运行行为。
- 实际结果：SE 留存实现前退出 101 的 event／ports／request 模块缺失预期失败；实现后 42 个 contracts 测试通过。PEL 复跑 `cargo test -p contracts`、`cargo test --workspace` 均退出 0，后者 50 个单元／集成测试通过；对象安全、Send + Sync、纯构造映射与私有 fake 均有断言。SE 首轮完整 clippy 检出测试 `useless_vec`，修正后 PEL 复跑 clippy 与 fmt 均退出 0。

### P-05 实现依赖守卫与临时 fixture 自测

- 状态：已完成
- 依赖：P-04
- 文件：新增 `scripts/check-layering.sh`、`apps/crates/contracts/tests/architecture_guard.rs` 及 `scripts/**` 内私有测试辅助文件（如确需）。
- 操作：通过 cargo metadata 和原始 manifest／源码检查实现真实守卫；用内存 JSON fixture、临时 workspace 和命令 runner 替身覆盖合法图、非法边、端口／版本违例及全部运行故障。普通测试不得递归进入 `--self-test`。
- 验证：在 `apps/` 分别执行 `../scripts/check-layering.sh --root ..` 和 `../scripts/check-layering.sh --root .. --self-test`。
- 预期：真实图与合法 fixture 返回 0；所有非法／故障子守卫稳定非零且有诊断，自测 harness 仅在全部符合预期时返回 0。
- 实际结果：TE 留存实现前聚焦测试退出 101 的预期失败，实现后 6 个架构测试通过。真实图命令退出 0；自测退出 0，覆盖真实图 1、合法 fixture 1、非法 fixture 11、运行故障 9，共 22 次子守卫调用。脚本为 100755，`sh -n` 通过；普通 workspace 测试不递归。TE 的 `cargo test --workspace`、clippy、fmt 与单线程测试均退出 0，默认和单线程各 56 个单元／集成测试通过。PEL 另核实脚本权限、sh 语法和 fmt，未在暂停后重复运行守卫。

### P-06 独立质保与问题修复

- 状态：已完成
- 依赖：P-05
- 文件：只读评审本版全部 diff；如发现阻断问题，仅在 Spec 已确认路径内修复并补回归测试。
- 操作：QA 独立核对契约语义、范围、测试有效性和 DoD；SE／TE 处理确认的问题，PEL 不代替独立结论。
- 验证：QA 明确列出发现、严重级别和证据；修复后重跑受影响测试。
- 预期：阻断和高优先级问题均有修复证据，或如实保留并阻止完成。
- 实际结果：完整 QA 已审查 tracked diff 和全部 untracked 任务文件，结论为无阻断、高、中、低发现，P-06 通过。QA 独立运行 contracts 测试退出 0（48 passed），真实图与自测均退出 0（`real=1 legal=1 invalid=11 faults=9`），fmt、sh 语法和 tracked diff 空白检查为 0，脚本权限 755。AC-01 build、AC-02 clippy、AC-10 双模式复验和 AC-11 最终出口留待 P-07，不将预审当作最终验收。

### P-07 全量出口验证与文档收尾

- 状态：已完成
- 依赖：P-06
- 文件：验证全 workspace 与 scripts；按实际结果修改 Spec、Plan、memory、`AGENTS.md`、`docs/api/README.md`、`docs/development.md`、`docs/roadmap.md`。
- 操作：分别运行全部必填及补充命令，记录独立退出码；更新现行导航和状态，不写 CHANGELOG，不执行发布操作。
- 验证：Spec AC-11 四项命令、`cargo build --workspace`、`cargo test --workspace -- --test-threads=1`、文档链接／格式检查和 `git diff --check`。
- 预期：所有必填 AC 有真实通过证据，现行文档与代码一致；任何失败均阻止完成。
- 实际结果：build、默认／单线程测试、clippy、fmt、真实图与自测守卫均分别退出 0，两种测试模式各 56 个测试通过；rustdoc、sh 语法和权限检查通过。四份现行文档已更新。首轮文档结构检查通过，尾随空格经 `git diff --check` 检出并修复；首轮复核 10 个 Markdown、128 个相对链接、21 个显式锚点及格式通过。完成态最终复核因 memory 新增两个经验索引链接而为 130 个相对链接、21 个显式锚点，检查与 diff 均退出 0。

## 验收覆盖与最终验证

| Spec 验收 | 覆盖步骤 | 结果与证据位置 |
|---|---|---|
| AC-01 | P-02、P-05、P-07 | 通过：P-07 build 退出 0，十 crate 构建；真实守卫确认成员集合。 |
| AC-02 | P-01、P-05、P-07 | 通过：七项 deny、十成员继承和测试窄豁免经 QA 核对；P-07 clippy 退出 0。 |
| AC-03 | P-03、P-07 | 通过：P-07 56 个 workspace 测试包含旧消息默认值、边界与无效输入断言。 |
| AC-04 | P-03、P-07 | 通过：System、TranscriptEntry 精确 JSON 与无效输入断言通过。 |
| AC-05 | P-04、P-06、P-07 | 通过：两个构造器均表驱动，修复后的三例 `turn_complete` 覆盖经 QA 和 P-07 测试验证。 |
| AC-06 | P-03、P-04、P-06、P-07 | 通过：拥有型契约与 Provider 两行签名一致；完整 QA 无发现，contracts 48 测试通过。 |
| AC-07 | P-05、P-06、P-07 | 通过：P-07 真实图与合法 fixture 均返回 0。 |
| AC-08 | P-05、P-06、P-07 | 通过：P-07 自测 11 个非法 fixture 均非零且带定位诊断。 |
| AC-09 | P-05、P-06、P-07 | 通过：P-07 自测 9 个运行故障均非零且无成功结论。 |
| AC-10 | P-03～P-05、P-07 | 通过：P-07 默认与单线程均为 56 passed、0 failed、0 ignored。 |
| AC-11 | P-07 | 通过：test、clippy、fmt、守卫自测分别退出 0，结果见下表。 |

| 完整检查 | 实际结果 |
|---|---|
| `cargo build --workspace` | P-07 退出 0，十个 crate 全部构建。 |
| `cargo test --workspace` | P-07 退出 0，56 passed、0 failed、0 ignored。 |
| `cargo clippy --workspace --all-targets -- -D warnings` | P-07 退出 0，零告警。 |
| `cargo fmt --all -- --check` | P-07 退出 0，无格式 diff。 |
| `../scripts/check-layering.sh --root ..` | P-07 退出 0，真实十 crate 图通过，入口 1 passed、5 filtered。 |
| `../scripts/check-layering.sh --root .. --self-test` | P-07 退出 0：`real=1 legal=1 invalid=11 faults=9`，22 次子守卫调用均断言状态和诊断。 |
| `cargo test --workspace -- --test-threads=1` | P-07 退出 0，56 passed、0 failed、0 ignored。 |
| 补充构建与脚本检查 | P-07 `cargo doc --workspace --no-deps` 和 `/bin/sh -n scripts/check-layering.sh` 均退出 0；脚本权限 755。实际工具链为 Rust／Cargo 1.98.1。 |
| 文档相对链接／格式与 `git diff --check` | 首轮结构检查 10 个变更 Markdown、128 个相对链接、21 个显式锚点、精确大小写、单 H1、标题层级和代码围栏通过；`git diff --check` 检出 roadmap 新状态行尾随空格，已删除。完成态最终复核为 130 个相对链接、21 个显式锚点，结构检查与 diff 检查均退出 0。 |

## 收尾记录

- 步骤结果：P-01～P-07 全部完成；完整 QA 无阻断、高、中、低发现，11 条 AC 均有最终通过证据。
- 验证结果：最终命令逐项结果见上表；文档结构、链接／锚点与 diff 复核通过。MSRV 1.93、release、覆盖率、性能、CLI smoke 和 CI 验收未执行，真实 Provider／工具／会话 smoke 属版本范围外。
- 偏差说明：`apps/Cargo.lock` 自动锁定和 Provider 文档两行签名同步均经用户明确批准；首轮文档 diff 检查发现的 roadmap 尾随空格已修复，无其他范围偏差。
- 收尾说明：现行文档、Spec 和 memory 已同步；未写 CHANGELOG，未执行 commit、push、标签或发布。
- Spec 回写：11 条 AC 已按 P-07 结果记录通过，Spec 状态已完成。
