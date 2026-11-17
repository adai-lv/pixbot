#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use contracts::error::{ContractError, ErrorDomain};
use contracts::event::{AgentEvent, AgentSession, EventSender, RunOutcome, RunStopReason};
use contracts::message::Message;
use contracts::model::{ModelMetadata, WireContent, WireMessage, WireRole};
use contracts::ports::{ContextEngine, SessionLoad, SessionRepository, UsageRecorder};
use contracts::request::CompletionRequest;
use contracts::tool::{
    Tool, ToolContent, ToolContext, ToolDefinition, ToolExecutionMode, ToolResult, ToolSet,
};
use contracts::transcript::TranscriptEntry;
use contracts::usage::Usage;

fn metadata() -> ModelMetadata {
    ModelMetadata {
        id: "model".into(),
        api: "api".into(),
        context_window: 100,
        max_output_tokens: 20,
    }
}

fn request() -> CompletionRequest {
    CompletionRequest {
        model: metadata(),
        messages: vec![],
        tools: vec![],
    }
}

struct FakeContext;

#[async_trait::async_trait]
impl ContextEngine for FakeContext {
    fn project(&self, entries: &[TranscriptEntry]) -> Result<Vec<Message>, ContractError> {
        Ok(entries
            .iter()
            .map(|entry| match entry {
                TranscriptEntry::Message(message) => message.clone(),
            })
            .collect())
    }

    fn to_wire(&self, _messages: &[Message]) -> Result<Vec<WireMessage>, ContractError> {
        Ok(vec![WireMessage {
            role: WireRole::System,
            content: vec![WireContent::Text {
                text: "fake-port-value".into(),
            }],
        }])
    }

    async fn prepare_request(&self) -> Result<CompletionRequest, ContractError> {
        Ok(request())
    }
}

#[test]
fn context_port_returns_owned_projection_wire_and_request() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn ContextEngine>>();

    let context = FakeContext;
    let entries = vec![TranscriptEntry::Message(Message::user("消息"))];
    let projected = context.project(&entries).unwrap();
    assert_eq!(projected, vec![Message::user("消息")]);

    let wire = context.to_wire(&projected).unwrap();
    assert_eq!(wire[0].role, WireRole::System);
    let request = futures::executor::block_on(context.prepare_request()).unwrap();
    assert_eq!(request.model.id, "model");
    assert!(request.messages.is_empty());
}

#[derive(Default)]
struct FakeSession {
    entries: Mutex<Vec<TranscriptEntry>>,
}

#[async_trait::async_trait]
impl SessionRepository for FakeSession {
    async fn append(&self, entry: TranscriptEntry) -> Result<(), ContractError> {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(entry);
        Ok(())
    }

    async fn load(&self) -> Result<SessionLoad, ContractError> {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        Ok(SessionLoad {
            entries,
            diagnostics: vec![ContractError::Protocol {
                domain: ErrorDomain::Session,
                message: "第 2 行损坏".into(),
            }],
        })
    }
}

#[derive(Default)]
struct FakeUsageRecorder {
    records: Mutex<Vec<Usage>>,
}

#[async_trait::async_trait]
impl UsageRecorder for FakeUsageRecorder {
    async fn record(&self, usage: Usage) -> Result<(), ContractError> {
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(usage);
        Ok(())
    }
}

#[test]
fn session_and_usage_ports_preserve_entries_diagnostics_and_records() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn SessionRepository>>();
    assert_send_sync::<Arc<dyn UsageRecorder>>();

    let session = FakeSession::default();
    futures::executor::block_on(session.append(TranscriptEntry::Message(Message::user("持久化"))))
        .unwrap();
    let loaded = futures::executor::block_on(session.load()).unwrap();
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.diagnostics.len(), 1);
    assert_eq!(loaded.diagnostics[0].to_string(), "session: 第 2 行损坏");

    let recorder = FakeUsageRecorder::default();
    let usage = Usage {
        input: 3,
        ..Usage::default()
    };
    futures::executor::block_on(recorder.record(usage)).unwrap();
    assert_eq!(
        recorder
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_slice(),
        &[usage]
    );
}

struct FakeAgentSession;

#[async_trait::async_trait]
impl AgentSession for FakeAgentSession {
    async fn run(&self, input: contracts::event::RunInput, tx: EventSender) -> RunOutcome {
        let _accepted = tx.send(AgentEvent::MessageStart {
            message: input.message,
        });
        RunOutcome {
            usage: Usage::default(),
            stop: RunStopReason::Completed,
            error: None,
        }
    }

    fn steer(&self, _message: String) {}

    fn follow_up(&self, _message: String) {}

    fn set_queue_mode(&self, _mode: contracts::event::QueueMode) {}

    fn interrupt(&self) {}

    async fn drain_background(&self, _timeout: Duration) -> usize {
        0
    }
}

struct Sink;

impl contracts::event::EventSink for Sink {
    fn send(&self, _event: AgentEvent) -> bool {
        true
    }
}

#[test]
fn agent_session_port_is_object_safe_and_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn AgentSession>>();

    let session: Arc<dyn AgentSession> = Arc::new(FakeAgentSession);
    let sender: EventSender = Arc::new(Sink);
    session.steer("转向".into());
    session.follow_up("后续".into());
    session.set_queue_mode(contracts::event::QueueMode::All);
    session.interrupt();
    assert_eq!(
        futures::executor::block_on(session.drain_background(Duration::ZERO)),
        0
    );
    let outcome = futures::executor::block_on(session.run(
        contracts::event::RunInput {
            message: Message::user("运行"),
        },
        sender,
    ));
    assert_eq!(outcome.stop, RunStopReason::Completed);
    assert_eq!(outcome.error, None);
}

fn tool_result() -> ToolResult {
    ToolResult {
        content: vec![ToolContent::Text {
            text: "完成".into(),
        }],
        details: None,
        structured_content: None,
        usage: None,
        is_error: false,
        terminate: false,
    }
}

struct FakeTool;

#[async_trait::async_trait]
impl Tool for FakeTool {
    fn name(&self) -> &str {
        "fake"
    }

    fn description(&self) -> &str {
        "测试端口"
    }

    fn input_schema(&self) -> &serde_json::Value {
        static SCHEMA: std::sync::LazyLock<serde_json::Value> =
            std::sync::LazyLock::new(|| serde_json::json!({"type": "object"}));
        &SCHEMA
    }

    async fn execute(
        &self,
        _context: ToolContext,
        _input: serde_json::Value,
    ) -> Result<ToolResult, ContractError> {
        Ok(tool_result())
    }
}

struct FakeToolSet {
    tool: Arc<dyn Tool>,
}

impl ToolSet for FakeToolSet {
    fn definitions(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: self.tool.name().into(),
            description: self.tool.description().into(),
            input_schema: self.tool.input_schema().clone(),
            output_schema: self.tool.output_schema().cloned(),
        }]
    }

    fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        (name == self.tool.name()).then(|| Arc::clone(&self.tool))
    }
}

#[test]
fn tool_ports_support_defaults_lookup_and_private_fake_execution() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn Tool>>();
    assert_send_sync::<Arc<dyn ToolSet>>();

    let set = FakeToolSet {
        tool: Arc::new(FakeTool),
    };
    let definitions = set.definitions();
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].name, "fake");
    assert!(set.get("missing").is_none());

    let tool = set.get("fake").unwrap();
    assert_eq!(tool.execution_mode(), ToolExecutionMode::Parallel);
    assert_eq!(tool.output_schema(), None);
    let input = serde_json::json!({"value": 1});
    assert_eq!(tool.prepare_arguments(input.clone()).unwrap(), input);
    let context = ToolContext {
        call_id: "call-1".into(),
        cwd: PathBuf::from("/work"),
        cancel: tokio_util::sync::CancellationToken::new(),
        on_update: None,
    };
    let result = futures::executor::block_on(tool.execute(context, input)).unwrap();
    assert!(!result.is_error);
}
