//! Agent-loop and session events.

use chrono::{DateTime, Utc};
use cortex_common::{CheckpointId, CorrelationId, MessageId, RunId, SessionId, ToolCallId};
use cortex_core::Event;
use cortex_models::{Plan, Role, ToolCall};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Phases of the agent loop (OPEVR-compatible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopPhase {
    /// Waiting for work.
    Idle,
    /// Gathering observations / inputs.
    Observing,
    /// Planning next actions (LLM).
    Planning,
    /// Executing tools.
    Executing,
    /// Verifying outcomes.
    Verifying,
    /// Reflecting / updating memory.
    Reflecting,
    /// Persisting a checkpoint.
    Checkpointing,
    /// Run finished successfully.
    Done,
    /// Run failed.
    Failed,
}

/// A user message was accepted into a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserMessageReceived {
    /// Session id.
    pub session_id: SessionId,
    /// Optional run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// Message id.
    pub message_id: MessageId,
    /// Message text.
    pub content: String,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl UserMessageReceived {
    /// Create a new event.
    pub fn new(session_id: SessionId, message_id: MessageId, content: impl Into<String>) -> Self {
        Self {
            session_id,
            run_id: None,
            message_id,
            content: content.into(),
            timestamp: Utc::now(),
        }
    }

    /// Attach a run id.
    pub fn with_run_id(mut self, run_id: RunId) -> Self {
        self.run_id = Some(run_id);
        self
    }
}

impl Event for UserMessageReceived {
    fn kind(&self) -> &'static str {
        "agent.user_message"
    }
}

/// An assistant message was produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessageProduced {
    /// Session id.
    pub session_id: SessionId,
    /// Optional run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// Message id.
    pub message_id: MessageId,
    /// Text content.
    pub content: String,
    /// Tool calls requested in this message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl AssistantMessageProduced {
    /// Create a new event.
    pub fn new(session_id: SessionId, message_id: MessageId, content: impl Into<String>) -> Self {
        Self {
            session_id,
            run_id: None,
            message_id,
            content: content.into(),
            tool_calls: Vec::new(),
            timestamp: Utc::now(),
        }
    }

    /// Attach tool calls.
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = tool_calls;
        self
    }

    /// Attach a run id.
    pub fn with_run_id(mut self, run_id: RunId) -> Self {
        self.run_id = Some(run_id);
        self
    }
}

impl Event for AssistantMessageProduced {
    fn kind(&self) -> &'static str {
        "agent.assistant_message"
    }
}

/// Streaming text delta from the model (when token streaming is enabled).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantTextDelta {
    /// Session id.
    pub session_id: SessionId,
    /// Optional run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// Incremental text fragment.
    pub text: String,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl AssistantTextDelta {
    /// Create a new delta event.
    pub fn new(session_id: SessionId, text: impl Into<String>) -> Self {
        Self {
            session_id,
            run_id: None,
            text: text.into(),
            timestamp: Utc::now(),
        }
    }

    /// Attach a run id.
    pub fn with_run_id(mut self, run_id: RunId) -> Self {
        self.run_id = Some(run_id);
        self
    }
}

impl Event for AssistantTextDelta {
    fn kind(&self) -> &'static str {
        "agent.assistant_text_delta"
    }
}

/// A tool call is about to be executed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallRequested {
    /// Session id.
    pub session_id: SessionId,
    /// Optional run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// Tool call id.
    pub tool_call_id: ToolCallId,
    /// Tool name.
    pub name: String,
    /// Arguments.
    pub arguments: Value,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl ToolCallRequested {
    /// Create from a [`ToolCall`].
    pub fn from_call(session_id: SessionId, call: &ToolCall) -> Self {
        Self {
            session_id,
            run_id: None,
            tool_call_id: call.id,
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            timestamp: Utc::now(),
        }
    }

    /// Attach a run id.
    pub fn with_run_id(mut self, run_id: RunId) -> Self {
        self.run_id = Some(run_id);
        self
    }
}

impl Event for ToolCallRequested {
    fn kind(&self) -> &'static str {
        "agent.tool_call.requested"
    }
}

/// A tool call completed successfully (or with tool-level error flag).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallCompleted {
    /// Session id.
    pub session_id: SessionId,
    /// Optional run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// Tool call id.
    pub tool_call_id: ToolCallId,
    /// Tool name.
    pub name: String,
    /// Output text.
    pub output: String,
    /// Whether the tool reported an error.
    pub is_error: bool,
    /// Duration in milliseconds, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl ToolCallCompleted {
    /// Create a completion event.
    pub fn new(
        session_id: SessionId,
        tool_call_id: ToolCallId,
        name: impl Into<String>,
        output: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            session_id,
            run_id: None,
            tool_call_id,
            name: name.into(),
            output: output.into(),
            is_error,
            duration_ms: None,
            timestamp: Utc::now(),
        }
    }
}

impl Event for ToolCallCompleted {
    fn kind(&self) -> &'static str {
        "agent.tool_call.completed"
    }
}

/// A tool call failed at the runtime/infrastructure level (timeout, cancel, panic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallFailed {
    /// Session id.
    pub session_id: SessionId,
    /// Optional run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// Tool call id.
    pub tool_call_id: ToolCallId,
    /// Tool name.
    pub name: String,
    /// Error message.
    pub error: String,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl ToolCallFailed {
    /// Create a failure event.
    pub fn new(
        session_id: SessionId,
        tool_call_id: ToolCallId,
        name: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            session_id,
            run_id: None,
            tool_call_id,
            name: name.into(),
            error: error.into(),
            timestamp: Utc::now(),
        }
    }
}

impl Event for ToolCallFailed {
    fn kind(&self) -> &'static str {
        "agent.tool_call.failed"
    }
}

/// The agent loop transitioned between phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopPhaseChanged {
    /// Session id.
    pub session_id: SessionId,
    /// Optional run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// Previous phase.
    pub from: LoopPhase,
    /// New phase.
    pub to: LoopPhase,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl LoopPhaseChanged {
    /// Create a phase change event.
    pub fn new(session_id: SessionId, from: LoopPhase, to: LoopPhase) -> Self {
        Self {
            session_id,
            run_id: None,
            from,
            to,
            timestamp: Utc::now(),
        }
    }

    /// Attach a run id.
    pub fn with_run_id(mut self, run_id: RunId) -> Self {
        self.run_id = Some(run_id);
        self
    }
}

impl Event for LoopPhaseChanged {
    fn kind(&self) -> &'static str {
        "agent.loop.phase_changed"
    }
}

/// A checkpoint of loop/session state was saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointSaved {
    /// Session id.
    pub session_id: SessionId,
    /// Optional run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// Checkpoint id.
    pub checkpoint_id: CheckpointId,
    /// Optional human label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl CheckpointSaved {
    /// Create a checkpoint event.
    pub fn new(session_id: SessionId, checkpoint_id: CheckpointId) -> Self {
        Self {
            session_id,
            run_id: None,
            checkpoint_id,
            label: None,
            timestamp: Utc::now(),
        }
    }
}

impl Event for CheckpointSaved {
    fn kind(&self) -> &'static str {
        "agent.checkpoint.saved"
    }
}

/// A structured error was raised during a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorRaised {
    /// Session id, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    /// Optional run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// Error category (e.g. `"provider"`, `"tool"`, `"internal"`).
    pub category: String,
    /// Error message.
    pub message: String,
    /// Whether the run can continue.
    pub recoverable: bool,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl ErrorRaised {
    /// Create an error event.
    pub fn new(category: impl Into<String>, message: impl Into<String>, recoverable: bool) -> Self {
        Self {
            session_id: None,
            run_id: None,
            category: category.into(),
            message: message.into(),
            recoverable,
            timestamp: Utc::now(),
        }
    }

    /// Attach session id.
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }
}

impl Event for ErrorRaised {
    fn kind(&self) -> &'static str {
        "agent.error"
    }
}

/// A plan was created or updated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanUpdated {
    /// Session id.
    pub session_id: SessionId,
    /// Optional run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// The plan snapshot.
    pub plan: Plan,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl PlanUpdated {
    /// Create a plan-updated event.
    pub fn new(session_id: SessionId, plan: Plan) -> Self {
        Self {
            session_id,
            run_id: None,
            plan,
            timestamp: Utc::now(),
        }
    }
}

impl Event for PlanUpdated {
    fn kind(&self) -> &'static str {
        "agent.plan.updated"
    }
}

/// A message was appended (generic role).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageAppended {
    /// Session id.
    pub session_id: SessionId,
    /// Message id.
    pub message_id: MessageId,
    /// Role of the message.
    pub role: Role,
    /// Optional correlation id for the turn/run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<CorrelationId>,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl MessageAppended {
    /// Create a message-appended event.
    pub fn new(session_id: SessionId, message_id: MessageId, role: Role) -> Self {
        Self {
            session_id,
            message_id,
            role,
            correlation_id: None,
            timestamp: Utc::now(),
        }
    }
}

impl Event for MessageAppended {
    fn kind(&self) -> &'static str {
        "agent.message.appended"
    }
}

/// A nested sub-agent run started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubAgentStarted {
    /// Parent session id.
    pub parent_session_id: SessionId,
    /// Parent run id if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<RunId>,
    /// Child session id.
    pub child_session_id: SessionId,
    /// Child run id.
    pub child_run_id: RunId,
    /// Nesting depth.
    pub depth: u32,
    /// Subtask prompt (truncated by publisher).
    pub prompt: String,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl SubAgentStarted {
    /// Create event.
    pub fn new(
        parent_session_id: SessionId,
        child_session_id: SessionId,
        child_run_id: RunId,
        depth: u32,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            parent_session_id,
            parent_run_id: None,
            child_session_id,
            child_run_id,
            depth,
            prompt: prompt.into(),
            timestamp: Utc::now(),
        }
    }

    /// Attach parent run id.
    pub fn with_parent_run_id(mut self, run_id: RunId) -> Self {
        self.parent_run_id = Some(run_id);
        self
    }
}

impl Event for SubAgentStarted {
    fn kind(&self) -> &'static str {
        "agent.subagent.started"
    }
}

/// A nested sub-agent run finished.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubAgentFinished {
    /// Parent session id.
    pub parent_session_id: SessionId,
    /// Parent run id if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<RunId>,
    /// Child session id.
    pub child_session_id: SessionId,
    /// Child run id.
    pub child_run_id: RunId,
    /// Nesting depth.
    pub depth: u32,
    /// Child status string.
    pub status: String,
    /// Turns consumed.
    pub turns: u32,
    /// Duration ms.
    pub duration_ms: u64,
    /// Final message preview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_message: Option<String>,
    /// Error if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Event time.
    pub timestamp: DateTime<Utc>,
}

impl SubAgentFinished {
    /// Create event from run output fields.
    pub fn new(
        parent_session_id: SessionId,
        child_session_id: SessionId,
        child_run_id: RunId,
        depth: u32,
        status: impl Into<String>,
        turns: u32,
        duration_ms: u64,
    ) -> Self {
        Self {
            parent_session_id,
            parent_run_id: None,
            child_session_id,
            child_run_id,
            depth,
            status: status.into(),
            turns,
            duration_ms,
            final_message: None,
            error: None,
            timestamp: Utc::now(),
        }
    }

    /// Attach parent run id.
    pub fn with_parent_run_id(mut self, run_id: RunId) -> Self {
        self.parent_run_id = Some(run_id);
        self
    }

    /// Attach final message.
    pub fn with_final_message(mut self, msg: impl Into<String>) -> Self {
        self.final_message.replace(msg.into());
        self
    }

    /// Attach error.
    pub fn with_error(mut self, err: impl Into<String>) -> Self {
        self.error.replace(err.into());
        self
    }
}

impl Event for SubAgentFinished {
    fn kind(&self) -> &'static str {
        "agent.subagent.finished"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_core::EventEnvelope;
    use cortex_models::ToolCall;
    use serde_json::json;

    #[test]
    fn tool_call_requested_envelope_roundtrip() {
        let call = ToolCall::new("read_file", json!({"path": "a.rs"}));
        let event = ToolCallRequested::from_call(SessionId::new(), &call);
        assert_eq!(event.kind(), "agent.tool_call.requested");
        let env = EventEnvelope::from_typed(event.clone());
        let back: ToolCallRequested = env.payload_as().unwrap();
        assert_eq!(event.tool_call_id, back.tool_call_id);
        assert_eq!(event.name, back.name);
    }

    #[test]
    fn loop_phase_serde() {
        let event =
            LoopPhaseChanged::new(SessionId::new(), LoopPhase::Planning, LoopPhase::Executing);
        let raw = serde_json::to_string(&event).unwrap();
        let back: LoopPhaseChanged = serde_json::from_str(&raw).unwrap();
        assert_eq!(event, back);
    }
}
