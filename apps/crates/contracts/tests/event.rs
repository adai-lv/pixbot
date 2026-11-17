#![allow(clippy::panic)]

use std::sync::{Arc, Mutex};

use contracts::error::{ContractError, ErrorDomain};
use contracts::event::{
    AgentEvent, AssistantDelta, EventSender, EventSink, ModelStopReason, NoticeLevel, QueueMode,
    RunInput, RunOutcome, RunStopReason,
};
use contracts::message::Message;
use contracts::tool::{ToolContent, ToolResult};
use contracts::usage::Usage;

fn result(text: &str, is_error: bool) -> ToolResult {
    ToolResult {
        content: vec![ToolContent::Text { text: text.into() }],
        details: None,
        structured_content: None,
        usage: None,
        is_error,
        terminate: false,
    }
}

#[test]
fn tool_execution_end_constructor_maps_every_input_deterministically() {
    let cases = [
        ("call-none", "", false, None),
        ("call-zero", "零", false, Some(0)),
        ("调用-最大", "失败 🦀", true, Some(u64::MAX)),
    ];

    for (call_id, text, is_error, duration_ms) in cases {
        let tool_result = result(text, is_error);
        let first = AgentEvent::tool_execution_end(call_id, tool_result.clone(), duration_ms);
        let second = AgentEvent::tool_execution_end(call_id, tool_result.clone(), duration_ms);

        assert_eq!(first, second);
        match first {
            AgentEvent::ToolExecutionEnd {
                call_id: actual_id,
                result: actual_result,
                duration_ms: actual_duration,
            } => {
                assert_eq!(actual_id, call_id);
                assert_eq!(actual_result, tool_result);
                assert_eq!(actual_result.is_error, is_error);
                assert_eq!(actual_duration, duration_ms);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}

#[test]
fn turn_complete_constructor_only_maps_usage() {
    let cases = [
        ("zero usage", Usage::default()),
        (
            "token usage",
            Usage {
                input: 10,
                output: 4,
                total_tokens: 14,
                ..Usage::default()
            },
        ),
        (
            "cache, reasoning, and cost usage",
            Usage {
                cache_read: 6,
                cache_write: 7,
                cache_write_1h: 8,
                reasoning: 2,
                cost: 0.125,
                ..Usage::default()
            },
        ),
    ];

    for (name, usage) in cases {
        let first = AgentEvent::turn_complete(usage);
        let second = AgentEvent::turn_complete(usage);

        assert_eq!(first, second, "non-deterministic event for {name}");
        assert_eq!(
            first,
            AgentEvent::TurnComplete { usage },
            "incorrect usage mapping for {name}"
        );
    }
}

#[test]
fn agent_event_and_outcome_keep_design_variants_and_separate_stop_reasons() {
    let message = Message::assistant(vec![]);
    let events = vec![
        AgentEvent::AgentStart,
        AgentEvent::TurnStart,
        AgentEvent::MessageStart {
            message: message.clone(),
        },
        AgentEvent::MessageUpdate {
            delta: AssistantDelta::Text("增量".into()),
        },
        AgentEvent::MessageEnd { message },
        AgentEvent::ToolExecutionStart {
            call_id: "call-1".into(),
            name: "read".into(),
            args: serde_json::json!({"path": "a.txt"}),
        },
        AgentEvent::ToolExecutionUpdate {
            call_id: "call-1".into(),
            partial: result("部分", false),
        },
        AgentEvent::TurnEnd {
            stop_reason: ModelStopReason::ToolUse,
        },
        AgentEvent::Error {
            error: ContractError::Internal {
                domain: ErrorDomain::Agent,
                message: "失败".into(),
            },
        },
        AgentEvent::Notice {
            level: NoticeLevel::Warn,
            message: "警告".into(),
        },
    ];
    assert_eq!(events.len(), 10);

    let input = RunInput {
        message: Message::user("输入"),
    };
    assert_eq!(input.message, Message::user("输入"));
    let outcome = RunOutcome {
        usage: Usage::default(),
        stop: RunStopReason::Failed,
        error: Some(ContractError::Protocol {
            domain: ErrorDomain::Provider,
            message: "协议错误".into(),
        }),
    };
    assert_eq!(outcome.stop, RunStopReason::Failed);
    assert!(outcome.error.is_some());
    assert_ne!(RunStopReason::Completed, RunStopReason::Aborted);
    assert_ne!(ModelStopReason::Length, ModelStopReason::Aborted);
}

#[test]
fn assistant_delta_notice_and_queue_mode_cover_fixed_shapes() {
    let deltas = [
        AssistantDelta::Text("文本".into()),
        AssistantDelta::Thinking("思考".into()),
        AssistantDelta::ThinkingSignature("签名".into()),
        AssistantDelta::ToolUseStart {
            id: "call-1".into(),
            name: "read".into(),
        },
        AssistantDelta::ToolUseDelta {
            id: "call-1".into(),
            partial_json: "{}".into(),
        },
        AssistantDelta::ToolUseEnd {
            id: "call-1".into(),
        },
    ];
    assert_eq!(deltas.len(), 6);
    assert_ne!(NoticeLevel::Info, NoticeLevel::Error);
    assert_ne!(QueueMode::OneAtATime, QueueMode::All);
    assert_ne!(ModelStopReason::Stop, ModelStopReason::Error);
}

#[derive(Default)]
struct FakeEventSink {
    events: Mutex<Vec<AgentEvent>>,
    closed: bool,
}

impl EventSink for FakeEventSink {
    fn send(&self, event: AgentEvent) -> bool {
        if self.closed {
            return false;
        }
        self.events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(event);
        true
    }
}

#[test]
fn event_sink_is_synchronous_send_sync_and_reports_closed_receiver() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EventSender>();

    let sink = Arc::new(FakeEventSink::default());
    let sender: EventSender = sink.clone();
    let event = AgentEvent::TurnStart;

    assert!(sender.send(event.clone()));
    assert_eq!(
        sink.events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_slice(),
        &[event]
    );

    let closed = FakeEventSink {
        closed: true,
        ..FakeEventSink::default()
    };
    assert!(!closed.send(AgentEvent::TurnComplete {
        usage: Usage::default(),
    }));
    assert!(
        closed
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_empty()
    );
}
