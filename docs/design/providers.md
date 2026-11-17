# 模型与 Provider 设计

> 本文件只权威定义 ChatModel、ProviderRuntime、协议适配、目录、认证、流式事件和 provider 层重试。下述分期均为目标，固定 pi 全量基线不是已实现清单。Agent 控制流见[Agent 设计](agent.md)，请求 DTO 的生产规则见[上下文设计](context.md)。

## 已选模型与端口边界

前端／装配层在每次 `run()` 前确定 `ModelRef`。`ProviderRuntime` 只按该引用解析模型元数据、认证和 `model.api` 对应的协议适配器，返回已选 `ChatModel`；它不替调用方选择模型。Agent 在一次 run 内固定使用该实例，Context 只读取其元数据并构造 `CompletionRequest`，双方均不做路由或轮间换模型。

```rust
pub type ChatStream = Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>;

#[async_trait]
pub trait ChatModel: Send + Sync {
    /// 认证/连接失败也编码为流内 Error，不产生同步抛出路径。
    async fn stream(&self, req: CompletionRequest) -> ChatStream;
    async fn stream_simple(&self, req: CompletionRequest) -> ChatStream;
}
```

`CompletionRequest` 是 contracts 中由 Context 产出、Provider 消费的中立 DTO。ChatModel 只把请求转换为流事件；ProviderRuntime、目录和认证均留在 providers 内部，不进入 Agent。image/classifier 是 M3 独立端口，不能伪装成 chat stream。pi 快照实际行为：`streamSimple`（pi 标识符）对应物见 `resources/pi/packages/ai/src/compat.ts`、`resources/pi/packages/ai/src/providers/amazon-bedrock.ts`（协议实现见 `resources/pi/packages/ai/src/api/bedrock-converse-stream.ts`）；image/classifier 端口对应物见 `resources/pi/packages/ai/src/images.ts`、`resources/pi/packages/ai/src/images-api-registry.ts`。

| 模块 | 职责 |
|---|---|
| `gateway` | 按已选 ModelRef 解析、缓存 ChatModel，按 `model.api` 分派 |
| `catalog` | 静态快照、动态 overlay、ModelsStore、离线恢复与刷新 |
| `auth` | apiKey/OAuth/ambient 凭据、存储、刷新与 logout |
| `api/*` | 协议的 stream/streamSimple、错误、usage、工具调用与取消适配 |
| `providers/*` | factory 的模型、baseUrl、headers、auth 与 API 绑定 |
| `retry` | 仅响应头之前的 provider 层重试，不做整轮 assistant 重试 |

## 重试边界

provider 层默认 `maxRetries=0`。启用时只覆盖拿到响应头之前：`x-should-retry:true`、无 status 网络错误、408/409/429/≥500 可重试；退避为 `min(500ms × 2^retryIndex, 8s)`，最多 −25% 乘性抖动，尊重 retry-after/retry-after-ms，超过默认 60s 的 `maxRetryDelayMs` 立即失败。流中途断线不在此层重试，abort 立即停止。以上默认值与阈值均为 pi 快照实际行为：`maxRetries ?? 0`、`x-should-retry`、`408|409|429|>=500`、`Math.min(0.5*2**retryIndex, 8)*1000`、`*(1-Math.random()*0.25)`、`retry-after-ms`/`retry-after`、`DEFAULT_MAX_RETRY_DELAY_MS=60_000` 见 `resources/pi/packages/ai/src/utils/provider-retry.ts`，`maxRetries`/`maxRetryDelayMs` 透传见 `resources/pi/packages/ai/src/api/simple-options.ts`。

pi 的 `retryAssistantCall`（pi 标识符）**实际位于 `resources/pi/packages/ai/src/utils/retry.ts`**，不在 `resources/pi/packages/agent/src/stream-fn.ts`（该文件只有 `setDefaultStreamFn`/`getDefaultStreamFn`）；它对应 Pixbot 的 `agent::recover::stream_assistant`。后者是**调用级整轮重试唯一入口**这一 Pixbot 自身设计，providers 不得在协议适配器、gateway 或目录层再次执行整轮重试。适用错误、退避、abort 与最终收尾语义统一见[Agent 恢复契约](agent.md#m0-主循环)。

## StreamEvent 与用量

```rust
pub enum StreamEvent {
    Start,
    TextDelta(String),
    ThinkingDelta(String),
    ThinkingSignature(String),
    ToolUseStart { id: String, name: String },
    ToolUseDelta { id: String, partial_json: String },
    ToolUseEnd { id: String },
    Done { stop_reason: ModelStopReason, usage: Usage, model: String },
    Error(ProviderError), // contracts 中立错误
}
```

上面这段 `StreamEvent` 是 **Pixbot 自身的目标契约定义**（contracts 中立 DTO），不是 pi 的变体清单。pi 快照实际行为：`AssistantMessageEvent`（pi 标识符）为 `start`/`text_start`/`text_delta`/`text_end`/`thinking_start`/`thinking_delta`/`thinking_end`/`toolcall_start`/`toolcall_delta`/`toolcall_end`/`error`（另有 `done` 类变体），采用 `toolcall_*` 命名，**无独立 `ThinkingSignature` 变体**，也无 `TurnComplete`（`resources/pi/packages/ai/src/types.ts`、`resources/pi/packages/agent/src/types.ts`）。pi 快照实际行为：Anthropic thinking signature 的字段名为 `thinkingSignature`（pi 标识符），`signature_delta` 聚合到该字段、回传时原样回放（`resources/pi/packages/ai/src/types.ts`、`resources/pi/packages/ai/src/api/anthropic-messages.ts`）。

Usage 字段按 pi 快照实测为 8 项：`input` / `output` / `cacheRead` / `cacheWrite` / `cacheWrite1h` / `reasoning` / `totalTokens` / `cost`（`resources/pi/packages/ai/src/types.ts` 的 `Usage`，均为 **pi 标识符**、camelCase）；Pixbot Rust 契约对应使用 snake_case：`input` / `output` / `cache_read` / `cache_write` / `cache_write_1h` / `reasoning` / `total_tokens` / `cost`。`reasoning` 是 output 子集，不重复累计，`cache_write_1h` 仅 Anthropic 拆分上报。Pixbot 只计量不计费，cost 仅透传到独立台账。

## 认证与 M0 协议要求

认证优先级：显式 key → 已存储凭据 → ambient 凭据。已存储凭据具有独占语义，refresh 失败或类型不匹配时返回无凭据，不回退环境变量。ambient 包括环境变量、AWS profile、Google ADC 和 WIF。OAuth 在过期前 5 分钟刷新，锁内双检，单次刷新 15s 超时。以上均为 pi 快照实际行为：认证优先级与 `readCredential` 独占语义见 `resources/pi/packages/ai/src/auth/resolve.ts`，ambient 见 `resources/pi/packages/ai/src/env-api-keys.ts`；`DEFAULT_OAUTH_MINIMUM_VALIDITY_MS=5*60*1000`、`DEFAULT_OAUTH_REFRESH_TIMEOUT_MS=15_000` 与 `AbortSignal.timeout` 见 `resources/pi/packages/ai/src/auth/resolve.ts`（`resources/pi/packages/ai/src/oauth.ts` 仅为 type-only 兼容入口，非行为实现）。

M0 的 OpenAI Completions 与 Responses 必须分别实现和验收，不能用“OpenAI-compatible”代替 Responses；Anthropic Messages 必须保留 thinking signature。三种 API 都覆盖 text、thinking、tool use、partial JSON、usage、error 和 abort，不适用项须显式说明。pi 快照实际行为：三个 API 各自独立存在于 `resources/pi/packages/ai/src/api/openai-completions.ts`、`resources/pi/packages/ai/src/api/openai-responses.ts`、`resources/pi/packages/ai/src/api/anthropic-messages.ts`；「保留 thinking signature」是 Pixbot M0 验收目标，pi 侧字段与回放见上段。

## 固定 pi 基线与分期

上游持续演进，若跟随最新版本，「对齐完成」就没有可判定的终止条件；因此本设计冻结基线、分期验收，M0 首批实现不冒充全量兼容。

| 阶段 | 目标范围 |
|---|---|
| M0 | 首批静态目录与基础 key 认证；`openai-completions`、`openai-responses`、`anthropic-messages`；已选 ModelRef 解析、基础 transport/headers/timeout/cancel 与 provider 层重试 |
| M1 | 其余七个 chat API：`google-generative-ai`、`azure-openai-responses`、`openai-codex-responses`、`bedrock-converse-stream`、`google-vertex`、`mistral-conversations`、`pi-messages` |
| M2 | 42 个 factory 注册/元数据/auth；其中 41 个 chat factory 完整执行对齐；custom provider、混合 API、全量静态/动态目录、离线恢复、ETag/304 与九个 OAuth factory |
| M3 | images、classifiers、deferred 公共契约与完整请求横切观测；typesafe classifier 执行 |

固定基线是 42 个内建 factory、10 个 chat API、1 个 image API、4 个 classifier API。`typesafe` 是 classifier-only，其余 41 个 factory 有 chat 能力；`fauxProvider`（pi 标识符）只是测试设施，不是第 43 个生产 provider。以上计数与 M1 七个 chat API 名称均为 pi 快照实际行为：`builtinProviders()` 42 个 factory 见 `resources/pi/packages/ai/src/providers/all.ts`；`KnownApi` 10 值、`KnownImageApi` 1 值（`openrouter-images`）、`KnownClassifierApi` 4 值、`StopReason` 含 `deferred` 见 `resources/pi/packages/ai/src/types.ts`；`typesafe` 仅 classifier 见 `resources/pi/packages/ai/src/providers/typesafe.ts`；`faux` 测试设施见 `resources/pi/packages/ai/src/providers/faux.ts`。M0 三个 chat API 名称同样以 `resources/pi/packages/ai/src/types.ts` 的 `KnownApi` 为准。补充：`radiusProvider()` 的 id 为动态生成（`options.id ?? "radius"`，见 `resources/pi/packages/ai/src/providers/radius.ts`），`typesafeProvider()` 只注册 classifier、不绑定 chat `api`（`resources/pi/packages/ai/src/providers/typesafe.ts`）；Rust 侧不要按「41 个 chat + 1 个 classifier」硬编码 factory 形态，以免 radius 的动态 id 与 typesafe 的无 `api:` 绑定踩空。

```text
amazon-bedrock, ant-ling, anthropic, azure, baseten, cerebras,
cloudflare-ai-gateway, cloudflare-workers-ai, deepseek, fireworks,
github-copilot, google, google-vertex, groq, huggingface, kimi-coding,
meta, minimax, minimax-cn, mistral, moonshotai, moonshotai-cn, nvidia,
openai, openai-codex, opencode, opencode-go, openrouter, qwen-token-plan,
qwen-token-plan-cn, qwen-token-plan-individual, radius, together, typesafe,
vercel-ai-gateway, xai, xiaomi, xiaomi-token-plan-ams, xiaomi-token-plan-cn,
xiaomi-token-plan-sgp, zai, zai-coding-cn
```

上面 42 个 provider 名称清单与实测逐字一致（`resources/pi/packages/ai/src/providers/all.ts`、`resources/pi/packages/ai/src/providers/*.ts` 的 `id:` 字面值；`radius` 的 id 为动态生成）。九个 OAuth factory 按实测为 `anthropic` / `openaiCodex` / `openaiChatGPT` / `githubCopilot` / `openrouter` / `kimiCoding` / `meta` / `xai` / `radius`（**pi 标识符（OAuth loader 键）**，camelCase，见 `resources/pi/packages/ai/src/auth/oauth/load.ts`）。注意区分两个概念：`openai` 是 42 个 provider factory 的 **provider id**，而 `openaiChatGPT` 是 **OAuth loader 键**；`openai` provider 挂 `loadOpenAIChatGPTOAuth`（`resources/pi/packages/ai/src/providers/openai.ts`），二者不是同一个键，不得混写。

pi 快照实际行为：「其余 41 个 factory 有 chat 能力」按 factory **是否绑定 chat `api`／直接实现 `stream`/`streamSimple`** 的静态绑定口径确证成立，唯一例外是 `typesafe`（classifier-only、无 `api:`），`radius` 直接实现 `stream`/`streamSimple`（`resources/pi/packages/ai/src/providers/all.ts`、`resources/pi/packages/ai/src/models.ts` 的 `createProvider`/`dispatch`/`apiFor`、`resources/pi/packages/ai/src/providers/typesafe.ts`、`resources/pi/packages/ai/src/providers/radius.ts`）；这是 factory 级静态绑定清点，**不是运行时逐模型端到端验证**，不得写成 41 个 provider 已端到端验证可对话。

目录生成产物不得手改（`resources/pi/packages/ai/src/models.generated.ts` 由 `catalog/generate.ts` 生成）。deferred 基线没有生产 chat adapter，不得虚构支持：pi 快照实际行为中 deferred 仅存在于类型契约（`StopReason` 含 `"deferred"`、`AssistantMessage.deferred?`、`ProviderStreams.fetchDeferred?`/`cancelDeferred?`）、capability wrapper（`lazyApi(load, capabilities?)`）与 provider 代理（`resources/pi/packages/ai/src/types.ts`、`resources/pi/packages/ai/src/api/lazy.ts`、`resources/pi/packages/ai/src/models.ts`）；`resources/pi/packages/ai/src/api/` 第一层共 15 个 `*.lazy.ts`：其中 10 个 chat loader 均以单参数调用 `lazyApi`、未传 `capabilities`，另 5 个是 classifier/image lazy factory（直接返回 `ProviderClassifier`／`ProviderImages`，不经过 `lazyApi`）；生产 `api/*.ts` 无 `fetchDeferred`/`cancelDeferred` 实现，只有 `lazy.ts` wrapper 按 `capabilities` 选择性挂载这两个方法，`fauxProvider` 是测试设施且不在 `builtinProviders()` 内；`DEFERRED_TOOL_PLACEHOLDER` 是 `defer_loading` 工具占位、非 response adapter（`resources/pi/packages/ai/src/api/anthropic-messages.ts`）。三处主语均限定为「类型契约／capability wrapper／测试设施」，不得写成 pi 已有生产 deferred 支持。只有表中每项均通过行为验收且无未说明剩余项，才能声明全量对齐；本目标不自动跟踪上游新增 provider。

## pi 对齐索引

事实出处均在 `resources/pi/` 源码。pi 的 `createProvider`、`ModelsStore`（pi 标识符）、`auth`、各 API 模块和两层重试是本设计的固定参考（见 `resources/pi/packages/ai/src/models.ts`、`resources/pi/packages/ai/src/models-store.ts`、`resources/pi/packages/ai/src/utils/provider-retry.ts`、`resources/pi/packages/ai/src/utils/retry.ts`）。Pixbot 的关键差异是模型在 run 前选定，以及调用级重试统一收归 Agent recover——这两处均为 **Pixbot 自身设计**，与 pi 的对应机制不同。本文不维护第二份逐行映射。
