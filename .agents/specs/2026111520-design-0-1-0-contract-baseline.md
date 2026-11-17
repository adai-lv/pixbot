# 0.1.0 工程与契约基线

> 状态：已完成。用户已确认本文全部范围、11 条验收标准及两项窄路径例外；完整 P-06 QA 和 P-07 出口均通过，11 条 AC 全部有实际证据。本文件作为历史记录，不作为现行架构或行为的权威来源。
> 关联计划：[0.1.0 工程与契约基线实施计划](../plans/2026111520-plan-0-1-0-contract-baseline.md)。
> 关联 Issue：无。

## 目标（可验证）

建立精确包含 `contracts`、`agent`、`context`、`providers`、`tools`、`session`、`config`、`bootstrap`、`headless`、`cli` 的 10-crate workspace，使中立契约可编译、可离线测试，workspace lint 统一生效，依赖守卫既能接受合法图，也能对非法依赖和运行故障给出可诊断的非零退出。

本版完成的一句话出口是「契约可测试，依赖守卫可拒绝非法依赖；不能调用模型」。骨架、DTO 和纯函数通过验收不代表 Agent、Provider、工具或会话运行时已经实现。

## 背景与证据

- 问题或动机：后续模型对话和工具闭环需要先固定分层、消息兼容和中立端口，避免实现并行增长后依赖方向与错误语义不可判定。
- 权威依据与设计：[AGENTS.md](../../AGENTS.md)、[架构的工程结构与核心不变量](../../docs/architecture.md)、[roadmap §2 与 §4.1](../../docs/roadmap.md)、[开发操作规范](../../docs/development.md)，以及[上下文](../../docs/design/context.md)、[Agent](../../docs/design/agent.md)、[会话](../../docs/design/session.md)、[工具](../../docs/design/tools.md)、[Provider](../../docs/design/providers.md) 五份领域设计。
- 相关代码：`apps/Cargo.toml`、`apps/crates/contracts/`、`apps/crates/cli/`；当前没有 `scripts/` 目录。
- 当前行为：已核实现有代码只有 `contracts` 的 `Message`／`Role`／`Content` 与 serde 测试，以及 `cli` 参数和占位执行骨架；`contracts` 继承 workspace lints，`cli` 尚未继承，restriction lints 尚未配置。
- 证据：开工时 `main` 工作树干净；历史每日 memory 没有进行中、中断或待核对任务。本 Spec 只把上述文件观察写成现状，不引用目标设计冒充实现。
- 假设与未决项：无。用户已明确确认本 Spec 全部范围与 11 条 AC，并批准 `apps/Cargo.lock` 仅由 Cargo 因新增 workspace 成员自动更新，不新增或升级外部依赖。实施预审发现 `docs/design/providers.md` 的借用型请求签名与已确认拥有型契约冲突；2026-11-16 用户明确批准仅同步该文件第 15～16 行为拥有型 `CompletionRequest`，不改变其他 Provider 语义，不回退代码契约。

## 范围

- 要改（生产）：`apps/crates/**`、`apps/Cargo.toml`、`scripts/**`。
- 已确认的路径例外：`apps/Cargo.lock` 仅允许 Cargo 因新增 workspace 成员自动更新，不新增或升级外部依赖；`docs/design/providers.md` 仅允许第 15～16 行 `stream`／`stream_simple` 的 `CompletionRequest<'_>` 同步为拥有型 `CompletionRequest`。
- 要改（工作文档）：本文、`.agents/README.md`、`.agents/todo.md`、`.agents/memory/2026-11-15.md`；确认后新增同前缀 Plan 并登记链接。
- 要改（实施收尾时按实际变化）：`AGENTS.md` 中守卫命令导航、`docs/api/README.md` 的已实现契约导航、`docs/development.md` 的 lint／守卫现行状态、`docs/roadmap.md` 的版本实际状态。
- 不碰：`resources/pi/**`、`apps/target/**`、其他历史 memory 任务块、除上述两行外的领域设计正文、外部服务与真实用户目录；不修改 `docs/changelog.md`，不执行 commit、push、标签或发布。
- 范围变更记录：用户已批准 `apps/Cargo.lock` 自动锁定例外；2026-11-16 又明确选择“允许同步两行”，批准上述 Provider 文档窄路径例外。两项例外均不授权新增或升级依赖、改变其他 Provider 语义或回退代码契约。

## 目标设计

### 方案与取舍

1. 按 roadmap 顺序先修 lint，再补骨架、契约和守卫。新 crate 只提供职责文档、必要模块边界和契约测试，不用 `todo!()`、假 transport、空执行循环或 no-op 落盘冒充能力；现有 CLI 参数与占位行为保持不变。
2. 十个 crate 均继承 workspace lints。`[workspace.lints.clippy]` 将 `unwrap_used`、`expect_used`、`panic`、`unreachable`、`dbg_macro`、`todo`、`unimplemented` 设为 `deny`；只对 `#[cfg(test)]` 模块或独立测试目标根部按实际需要豁免 `unwrap_used`／`expect_used`／`panic`，不在 workspace 或生产 lib 全量放宽。
3. 不新增第三方依赖。`contracts` 只复用已声明的 `serde`、`serde_json`、`async-trait`、`futures`、`thiserror`、`tokio-util`，不引入 HTTP、协议 SDK、tokio 运行时或 C 编译依赖。测试替身留在各 crate 私有测试代码，不建立共享 `testkit`。
4. 依赖守卫由 `scripts/check-layering.sh` 调用 `contracts` 私有的 `architecture_guard` 集成测试目标，不形成第 11 个 crate，也不把守卫代码放进生产 lib。通过 `cargo metadata --no-deps --format-version=1` 的结构化元数据判定内部依赖图，并检查必需 crate、依赖版本来源和公共端口 trait 位置；不用文本 grep 猜测依赖边。守卫是开发工具，不进入运行时二进制，本版不宣称 CI 已接入。

### 关键流程

Spec 确认后先建立 AC 到步骤的 Plan 映射，再依次实施：lint 继承与窄豁免 → 10-crate 骨架及依赖白名单 → `contracts` DTO、端口与纯函数 → 守卫及临时 fixture 自测 → 四项出口与补充验证 → 按真实结果回写文档。任一步暴露范围扩张或公共语义冲突时停止实施并重新确认，不在实现中静默改契约。

### 接口与数据

公共类型和 trait 只定义在 `contracts`，采用平坦模块组织；领域文档已经给出的端口语义不在此复制第二份全文，本表固定本版必须落地的形状和额外约束。

| 模块 | 本版契约形状 |
|---|---|
| `message` | `Role` 增加 `System`，JSON 为 `"system"`；增加 system 消息纯构造器，现有 user／meta／assistant／tool_results 构造器语义不变。`Message` 既有 `is_meta`、`uuid`、`parent_uuid` 默认值与序列化行为保持兼容，不增加 `Content::Image`。 |
| `transcript` | 只定义 `TranscriptEntry::Message(Message)`，采用 `type = "message"` 与 `message` payload 的 adjacently tagged union；未知标签、缺 payload 和错误 payload 均反序列化失败，不用 `serde(other)` 吞掉未知条目。不增加树、压缩、标签或 ContextEdit 变体。 |
| `usage` | `Usage` 固定为 `input`、`output`、`cache_read`、`cache_write`、`cache_write_1h`、`reasoning`、`total_tokens`、`cost` 八字段；只计量，不实现台账或计费。数值字段可有意义的零默认值，`cost` 使用 `f64`，不派生 `Eq`。 |
| `tool` | 定义六字段 `ToolResult { content, details, structured_content, usage, is_error, terminate }`。`content` 使用独立 `Vec<ToolContent>`，首版只有 `ToolContent::Text { text }`，不复用领域 `Content`；`details`／`structured_content`／`usage` 为带 serde 默认值的 Option，`terminate` 默认 false，`is_error` 必填而不默认成成功。`ToolExecutionMode` 只有 `Parallel`／`Sequential`。`ToolContext` 固定携带 `call_id: String`、`cwd: PathBuf`、`cancel: tokio_util::sync::CancellationToken`、`on_update: Option<Arc<dyn Fn(ToolResult) + Send + Sync>>`。定义 `ToolDefinition`、`Tool`、`ToolSet`，不加入 hooks、replay 或 constrained sampling。 |
| `agent` | `AssistantDelta` 固定为 `Text(String)`、`Thinking(String)`、`ThinkingSignature(String)`、`ToolUseStart { id, name }`、`ToolUseDelta { id, partial_json }`、`ToolUseEnd { id }`；`NoticeLevel` 为 `Info`／`Warn`／`Error`，`QueueMode` 为 `OneAtATime`／`All`，`ModelStopReason` 为 `Stop`／`ToolUse`／`Length`／`Error`／`Aborted`，`RunStopReason` 为 `Completed`／`Aborted`／`Failed`。按 Agent 设计定义 `AgentEvent`、`RunOutcome`、`AgentSession`，两类 stop reason 不混用，`ToolExecutionEnd` 不含重复外层 `is_error`。`RunInput` 是仅含 `message: Message` 的结构体，不加入 Continue、路由或恢复选项。 |
| `event` | 只提供 `AgentEvent::tool_execution_end` 与 `AgentEvent::turn_complete` 两个纯关联构造器，输入只映射到事件字段，不读取时钟、随机数、全局状态或 IO，不声称保证运行时事件顺序。`EventSink::send(AgentEvent) -> bool` 同步且非阻塞，false 只表示接收端关闭；`EventSender = Arc<dyn EventSink>`，tokio channel 留给 headless 私有适配。 |
| `model` | 定义 `ModelRef { provider, model }` 与最小 `ModelMetadata { id, api, context_window, max_output_tokens }`。`WireRole` 为 `System`／`User`／`Assistant`／`Tool`；`WireContent` 独立定义为 `Text { text }`、`Thinking { thinking, signature }`、`ToolUse { id, name, input }`、`ToolResult { tool_use_id, content, is_error }`，字段类型分别沿用 String、Option<String> 与 JSON 值的现有 M0 语义，不包含领域扩展；`WireMessage { role, content: Vec<WireContent> }`。按 Provider 设计定义 `StreamEvent` 与 `ChatStream`；`ChatModel` 声明 `metadata`、`stream`、`stream_simple`，失败只经 `StreamEvent::Error`；`ProviderRuntime` 只按 ModelRef 解析已选 ChatModel，不自动选模型。image／classifier／deferred 不进入本版。 |
| `request` | `CompletionRequest` 是**拥有型、无生命周期参数**的 DTO，固定包含 `model: ModelMetadata`、`messages: Vec<WireMessage>`、`tools: Vec<ToolDefinition>`。不携带 auth、base URL、HTTP timeout、retry 或厂商扩展，不把领域 Message／Content 直接当作 wire 类型，不做模型路由或协议映射。 |
| `context` | 声明 `ContextEngine` 的线性 `project`、`to_wire`、`prepare_request` 端口，返回拥有型 CompletionRequest，不借用自身临时投影；只定义接口，不实现 system 装配、压缩或请求构造算法。 |
| `session` | 声明 `SessionRepository` 的 `append`／`load` 与 `UsageRecorder` 的记录端口。`load` 返回 `SessionLoad { entries, diagnostics: Vec<ContractError> }` 以保留坏行诊断，整体打开／读取失败仍返回 Err；不提供文件实现。 |
| `config` | 定义净化且不可变的运行时 `ConfigSnapshot { model: ModelRef, cwd: PathBuf }`，两字段必填，不伪造空模型默认值；它是进程内快照，不增加 serde 或配置持久化承诺。快照不保存凭据、base URL、session 路径、prompt、trust、retry、timeout 或 provider catalog，不读取环境或配置文件。 |
| `error` | 只定义一个派生 `thiserror::Error` 的 `ContractError` 枚举和普通 `ErrorDomain` 分类枚举。错误变体表示可行动类别：`Busy`、`InvalidInput`、`NotFound`、`Unsupported`、`Aborted`、`Authentication`、`Connection`、`Timeout`、`RateLimited`、`QuotaExceeded`、`Protocol`、`ContextOverflow`、`Storage`、`Internal`；domain 表示 Agent／Context／Provider／Tool／Session／Usage／Config 归属。各变体携带 owned message，仅需要重试决策的类别携带 `retryable`；提供 `is_retryable`／`is_abort` 纯方法。AgentEvent、RunOutcome、StreamEvent 统一使用 ContractError，不另建端口错误枚举或别名。 |

ContractError 可 `Clone + PartialEq + Serialize + Deserialize`，不直接装 `std::io::Error`、HTTP SDK error 或 trait object source；实现 crate 跨边界时保留脱敏来源链和定位上下文。`Aborted` 永不可重试，`QuotaExceeded` 与 `RateLimited` 分型，`ContextOverflow` 不表示原请求重试。

异步端口使用 `async_trait`，跨 tokio 任务的端口对象满足 `Send + Sync`；`ChatStream` 满足 `Stream<Item = StreamEvent> + Send`。本版只用编译期断言和 crate 私有 fake 验证端口可消费，不提供生产适配器。

crate 依赖只允许下表的有向边；未列出的内部边一律拒绝，外部依赖仍受 workspace 规则约束。

| 来源 | 允许的内部依赖 |
|---|---|
| `contracts` | 无 |
| `agent`、`context`、`providers`、`tools`、`session`、`config` | `contracts`；L1/L2 层内及相互之间禁呼 |
| `bootstrap` | `agent`、`context`、`providers`、`tools`、`session`、`config`、`contracts` |
| `headless` | `bootstrap`、`contracts` |
| `cli` | `headless`；不得直接依赖 `contracts`、`bootstrap` 或 L1/L2 |

### 错误、边界与兼容

- 固定旧消息 payload 验证缺字段默认值；缺 `role`／`content`、未知 role、未知条目标签和错误字段类型必须返回 serde 错误，不静默默认或 panic。内联旧 payload 是兼容回归，不冒充历史 golden 套件。
- 持久化或协议 DTO 做精确 JSON、缺省值和无效输入测试；拥有型请求没有生命周期兼容分支，运行时配置快照和 trait 不强加无需求的 serde。
- 纯构造函数是总函数，异常分支记为不适用；错误 DTO 构造必须保留 `is_error=true` 与分类。运行期工具配对回填、事件排序、取消和 `TurnComplete` 收尾不在本版验证。
- 守卫命令固定为 `../scripts/check-layering.sh --root ..`，自测增加 `--self-test`。合法图返回 0；契约违例与运行故障均稳定非零并输出可区分的可定位诊断，不额外承诺具体非零数值。自测 harness 只有在真实图通过且所有负例符合预期时返回 0。
- 普通 `cargo test --workspace` 不得触发 `--self-test` 或递归调用守卫脚本；完整自测只由显式脚本参数进入。依赖版本来源必须读取原始 manifest 并复用 development.md A.2 的文本守卫，不得仅凭 cargo metadata 判绿；公共 trait 位置扫描在目录／文件读取失败时同样非零。
- 守卫覆盖生产 `[dependencies]` 及 Cargo 归一化后的 target-specific 生产依赖；`dev-dependencies`、`build-dependencies` 不宣称由本版方向守卫覆盖。版本来源与端口位置检查不扩大为所有 Rust 语法的静态分析。

### 安全与可观测性

普通测试完全离线，不访问真实凭据、Provider、用户会话或真实配置目录。配置快照不保存密钥。守卫负例只在系统临时目录创建最小 workspace，并用 `trap` 清理；不在真实 `apps/crates/` 下放置非法 fixture。诊断输出包含失败类别、crate／路径和非法边，不包含凭据或本地会话正文。

## 验收标准

以下 Cargo 命令工作目录均为 `apps/`；守卫命令中的 `..` 因而指向仓库根目录。

| 编号 | 类型 | 场景与操作 | 预期结果 | 验证方式 |
|---|---|---|---|---|
| AC-01 | 正常 | 构建精确 10-crate workspace，并读取结构化成员清单。 | 十个指定 crate 不多不少且全部可构建；每个 lib 有职责说明，无运行能力占位。 | `cargo build --workspace`；守卫成员集合检查。 |
| AC-02 | 契约 | 检查 lint 继承、指定 restriction lints 和测试豁免位置，再运行 clippy。 | 所有成员继承；生产禁止项被 deny，测试无宽泛豁免，当前代码零告警。 | `cargo clippy --workspace --all-targets -- -D warnings`；清单元数据／清单检查。 |
| AC-03 | 正常／边界／异常 | 反序列化固定旧 Message JSON、显式默认／非默认字段及无效 payload，并做新格式往返。 | 旧默认值精确保留；`is_meta=false` 省略序列化、`true` 不丢失；无效输入返回错误。 | `cargo test -p contracts` 的旧消息默认值、边界往返和无效 payload 用例。 |
| AC-04 | 契约 | 检查 `Role::System`、既有角色和 `TranscriptEntry::Message` 的精确 JSON 与无效标签。 | system 使用 `"system"`；旧角色可读；基础条目使用 `type + message` 并沿用 Message 默认值；未知 role／条目、缺 message payload 和错误 payload 均失败。 | `cargo test -p contracts` 的 role、构造器、条目往返与无效输入用例。 |
| AC-05 | 正常／边界／异常 | 表驱动调用两个事件纯构造器，构造 `ToolResult`、错误 DTO、usage 和持续时间边界。 | 变体及全部字段精确映射；同输入结果确定；`None`／`Some(0)`、空文本、Unicode 和错误结果不丢失；失败只从 `result.is_error` 读取。 | `cargo test -p contracts` 的事件构造与错误 DTO 用例；总函数异常分支记录不适用。 |
| AC-06 | 契约 | 构造／匹配其余 DTO、拥有型请求、流事件、运行时配置快照和端口 fake，并做编译期线程边界断言。 | CompletionRequest 无生命周期且可独立拥有；StreamEvent、端口、中立错误和净化配置快照可消费；领域／wire 内容、两类 ToolResult、两类 stop reason 不混用；无网络或 transport。 | `cargo test -p contracts` 的 DTO、端口对象安全和 `Send + Sync` 断言。 |
| AC-07 | 正常 | 对真实仓库和临时合法图分别执行守卫。 | 两者均返回 0；合法 fixture 覆盖全部允许层间边与 workspace 继承表示。 | `../scripts/check-layering.sh --root ..`；`../scripts/check-layering.sh --root .. --self-test`。 |
| AC-08 | 异常／契约 | 从合法 fixture 逐案只改一条非法边或一项端口／版本规则。 | `agent→context`、`providers→tools`、`agent→providers`、`contracts→agent`、`headless→agent`、`headless→providers`、`cli→bootstrap`、`cli→contracts`、端口 trait 越界和私有依赖版本均非零并诊断；harness 仅在全被拒绝时返回 0。 | 同一 `--self-test` 逐案捕获子守卫退出码和 stderr，不靠管道或 grep 命中判绿。 |
| AC-09 | 异常 | 分别构造根／crate 目录缺失、根／成员 manifest 缺失或畸形、元数据解析失败、被调用命令非零。 | 每个运行故障均稳定非零，诊断类别与相关路径；不出现成功结论。 | `--self-test` 的临时目录与命令 runner 私有替身用例。 |
| AC-10 | 边界 | 默认并发和单线程运行全部普通测试，清空真实凭据且不访问外部服务。 | 两种执行方式结果一致，无共享状态、真实目录、凭据或网络依赖。 | `cargo test --workspace`；补充 `cargo test --workspace -- --test-threads=1`。 |
| AC-11 | 契约 | 分别执行四项出口命令并记录退出码。 | test、clippy、fmt 和包含真实图／fixture 的守卫全部各自退出 0；不以复合命令只记录最后状态。 | `cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`../scripts/check-layering.sh --root .. --self-test`。 |

## 明确不做

- 不接入模型、认证、Provider transport、协议映射或重试；不执行工具，不实现 Agent 循环、事件运行时排序、取消恢复或结果回填算法。
- 不写会话、不加载 JSONL、不实现 headless 完整行为，不改变当前 CLI 的可对话能力；不实现 `to_wire`、system 装配、压缩或树投影算法。
- 不新增 crate、第三方依赖、共享 `testkit`、CI 常驻、发布标签或 CHANGELOG；不开展真实 Provider／工具／会话 smoke。
- 不宣称覆盖率、性能、历史基准回归、pi 全量兼容或后续版本能力达标。

## 风险、迁移与回滚

- 风险：契约过度定义会挤压后续实现，或守卫把工具缺失误判为合法图。通过只定义本版 DTO／端口、不提供运行实现，以及真实图与临时正负例双重验收控制；外部命令不可用必须非零退出并诊断。
- 迁移：既有 Message JSON 只做加法式枚举扩展和兼容回归，无数据迁移；没有历史 golden 或会话样本，本版相关 golden 项记录不适用。运行时配置快照不引入配置格式。
- 回滚：撤销本版 manifest、crate、脚本和文档改动即可；本版不写用户数据、不访问外部服务、不发布产物，无数据或发布回滚步骤。

## 收尾记录

| 编号 | 结果 | 实际结果与证据 |
|---|---|---|
| AC-01 | 通过 | P-07 `cargo build --workspace` 退出 0，十个指定 crate 全部构建；真实守卫确认精确成员集合。 |
| AC-02 | 通过 | 七项 deny、十成员继承及测试窄豁免经完整 QA 核对；P-07 clippy 退出 0。 |
| AC-03 | 通过 | P-07 workspace 56 个测试通过，其中 contracts 48 个；旧 Message 默认值、显式字段、精确 JSON、往返与无效输入均有断言。 |
| AC-04 | 通过 | System、system 构造器、TranscriptEntry 精确 JSON、旧角色及无效标签／payload 测试包含在上述通过结果中。 |
| AC-05 | 通过 | 两个构造器均三例表驱动；`turn_complete` 覆盖零值、token、缓存／推理／成本组合，持续时间和错误结果边界由 `tool_execution_end` 覆盖；完整 QA 无发现。 |
| AC-06 | 通过 | DTO、拥有型请求、端口 fake 与线程边界测试通过；Provider 两行签名按用户批准同步，完整 QA 对整个 diff 无阻断、高、中、低发现。 |
| AC-07 | 通过 | P-07 真实图及显式自测均退出 0；合法 fixture 覆盖全部允许边。 |
| AC-08 | 通过 | P-07 自测中 11 个非法 fixture 均非零并带定位诊断，harness 退出 0。 |
| AC-09 | 通过 | P-07 自测中 9 个运行故障均非零且无成功结论，诊断类别可区分。 |
| AC-10 | 通过 | P-07 默认与单线程 workspace 测试均退出 0，各 56 passed、0 failed、0 ignored；普通测试不递归自测。 |
| AC-11 | 通过 | P-07 分别运行 test、clippy、fmt 与守卫自测均退出 0；逐项结果见关联 Plan。 |

- 偏差与范围变化：`apps/Cargo.lock` 自动锁定例外和 `docs/design/providers.md` 第 15～16 行签名同步例外均经用户明确批准；前者仅由 Cargo 更新，后者最终 diff 只有获准两行。预审发现的表驱动与注释缺口已修复，完整 P-06 QA 无阻断、高、中、低发现。
- 补充检查与未执行说明：P-07 rustdoc、shell 语法和脚本 755 权限检查通过。真实 Provider／工具／会话 smoke 为版本范围外；历史 golden 因无兼容样本而不适用。MSRV 1.93、release、覆盖率、性能、CLI smoke 与 CI 验收未执行，不形成通过结论；当前工具链为 Rust／Cargo 1.98.1。
- 文档回写：索引和 TODO 已同步；`AGENTS.md`、`docs/api/README.md`、`docs/development.md`、`docs/roadmap.md` 已按最终代码出口更新。未写 CHANGELOG，未执行 commit、push、标签或发布。
