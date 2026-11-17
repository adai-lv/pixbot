#![allow(clippy::panic)]
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use contracts::error::{ContractError, ErrorDomain};
use contracts::event::ModelStopReason;
use contracts::model::{
    ChatModel, ChatStream, ModelMetadata, ModelRef, ProviderRuntime, StreamEvent, WireContent,
    WireMessage, WireRole,
};
use contracts::request::CompletionRequest;
use contracts::tool::ToolDefinition;
use contracts::usage::Usage;
use futures::{StreamExt, stream};

fn metadata(id: &str) -> ModelMetadata {
    ModelMetadata {
        id: id.into(),
        api: "anthropic-messages".into(),
        context_window: 200_000,
        max_output_tokens: 8_192,
    }
}

fn request(model: ModelMetadata) -> CompletionRequest {
    CompletionRequest {
        model,
        messages: vec![WireMessage {
            role: WireRole::User,
            content: vec![WireContent::Text {
                text: "拥有这些文本".into(),
            }],
        }],
        tools: vec![ToolDefinition {
            name: "read".into(),
            description: "读取".into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
        }],
    }
}

#[test]
fn wire_model_has_four_roles_and_independent_content_variants() {
    let roles = [
        (WireRole::System, "system"),
        (WireRole::User, "user"),
        (WireRole::Assistant, "assistant"),
        (WireRole::Tool, "tool"),
    ];
    for (role, expected) in roles {
        assert_eq!(serde_json::to_value(role).unwrap(), expected);
    }

    let contents = vec![
        WireContent::Text {
            text: "文本".into(),
        },
        WireContent::Thinking {
            thinking: "思考".into(),
            signature: Some("sig".into()),
        },
        WireContent::ToolUse {
            id: "call-1".into(),
            name: "read".into(),
            input: serde_json::json!({"path": "a.txt"}),
        },
        WireContent::ToolResult {
            tool_use_id: "call-1".into(),
            content: "结果".into(),
            is_error: true,
        },
    ];

    assert_eq!(serde_json::to_value(&contents[0]).unwrap()["type"], "text");
    assert_eq!(
        serde_json::to_value(&contents[1]).unwrap()["type"],
        "thinking"
    );
    assert_eq!(
        serde_json::to_value(&contents[2]).unwrap()["type"],
        "tool_use"
    );
    assert_eq!(
        serde_json::to_value(&contents[3]).unwrap()["type"],
        "tool_result"
    );
    let json = serde_json::to_string(&contents).unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<WireContent>>(&json).unwrap(),
        contents
    );
}

#[test]
fn completion_request_owns_model_messages_and_tools_without_lifetime() {
    fn consume_owned(value: CompletionRequest) -> CompletionRequest {
        value
    }

    let request = consume_owned(request(metadata("model-owned")));
    let model = request.model.clone();
    let messages = request.messages.clone();
    let tools = request.tools.clone();

    assert_eq!(model.id, "model-owned");
    assert_eq!(messages.len(), 1);
    assert_eq!(tools.len(), 1);
    assert_eq!(request.model.context_window, 200_000);
    assert_eq!(request.model.max_output_tokens, 8_192);

    let json = serde_json::to_string(&request).unwrap();
    assert_eq!(
        serde_json::from_str::<CompletionRequest>(&json).unwrap(),
        request
    );
}

struct FakeChatModel {
    metadata: ModelMetadata,
}

#[async_trait::async_trait]
impl ChatModel for FakeChatModel {
    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    async fn stream(&self, request: CompletionRequest) -> ChatStream {
        let text = match &request.messages[0].content[0] {
            WireContent::Text { text } => text.clone(),
            _ => String::new(),
        };
        Box::pin(stream::iter(vec![
            StreamEvent::Start,
            StreamEvent::TextDelta(text),
        ]))
    }

    async fn stream_simple(&self, _request: CompletionRequest) -> ChatStream {
        Box::pin(stream::iter(vec![StreamEvent::Done {
            stop_reason: ModelStopReason::Stop,
            usage: Usage::default(),
            model: self.metadata.id.clone(),
        }]))
    }
}

struct FakeProviderRuntime;

#[async_trait::async_trait]
impl ProviderRuntime for FakeProviderRuntime {
    async fn resolve(&self, model: &ModelRef) -> Result<Arc<dyn ChatModel>, ContractError> {
        Ok(Arc::new(FakeChatModel {
            metadata: metadata(&model.model),
        }))
    }
}

#[test]
fn chat_model_and_provider_runtime_are_consumable_without_transport() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_send<T: Send>() {}
    assert_send_sync::<Arc<dyn ChatModel>>();
    assert_send_sync::<Arc<dyn ProviderRuntime>>();
    assert_send::<ChatStream>();

    let reference = ModelRef {
        provider: "test".into(),
        model: "selected".into(),
    };
    let runtime = FakeProviderRuntime;
    let model = futures::executor::block_on(runtime.resolve(&reference)).unwrap();
    assert_eq!(model.metadata().id, "selected");

    let mut events = futures::executor::block_on(model.stream(request(metadata("selected"))));
    assert!(matches!(
        futures::executor::block_on(events.next()),
        Some(StreamEvent::Start)
    ));
    match futures::executor::block_on(events.next()) {
        Some(StreamEvent::TextDelta(text)) => assert_eq!(text, "拥有这些文本"),
        other => panic!("unexpected stream event: {other:?}"),
    }

    let mut simple =
        futures::executor::block_on(model.stream_simple(request(metadata("selected"))));
    match futures::executor::block_on(simple.next()) {
        Some(StreamEvent::Done {
            stop_reason, model, ..
        }) => {
            assert_eq!(stop_reason, ModelStopReason::Stop);
            assert_eq!(model, "selected");
        }
        other => panic!("unexpected simple stream event: {other:?}"),
    }
}

#[test]
fn stream_event_carries_neutral_errors_and_all_delta_shapes() {
    let error = ContractError::Connection {
        domain: ErrorDomain::Provider,
        message: "连接中断".into(),
        retryable: true,
    };
    let events = vec![
        StreamEvent::ThinkingDelta("思考".into()),
        StreamEvent::ThinkingSignature("签名".into()),
        StreamEvent::ToolUseStart {
            id: "call-1".into(),
            name: "read".into(),
        },
        StreamEvent::ToolUseDelta {
            id: "call-1".into(),
            partial_json: "{\"path\":".into(),
        },
        StreamEvent::ToolUseEnd {
            id: "call-1".into(),
        },
        StreamEvent::Error(error.clone()),
    ];

    match &events[5] {
        StreamEvent::Error(actual) => assert_eq!(actual, &error),
        other => panic!("expected error event, got {other:?}"),
    }
    let json = serde_json::to_string(&events).unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<StreamEvent>>(&json).unwrap(),
        events
    );
}
