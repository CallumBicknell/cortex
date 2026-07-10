//! Durable session container for agent conversations.

use crate::message::Message;
use chrono::{DateTime, Utc};
use cortex_common::SessionId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Lifecycle status of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Active and accepting turns.
    Active,
    /// Paused (e.g. waiting for approval).
    Paused,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
    /// Archived / soft-deleted.
    Archived,
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Archived => "archived",
        };
        f.write_str(s)
    }
}

/// An agent session (conversation + metadata).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Session id.
    pub id: SessionId,
    /// Absolute or logical workspace path.
    pub workspace: String,
    /// Model alias used for this session (e.g. `"default"`, `"ollama/qwen"`).
    pub model: String,
    /// Status.
    pub status: SessionStatus,
    /// Message history (in-memory; durable store comes in Phase 6).
    pub messages: Vec<Message>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last update time.
    pub updated_at: DateTime<Utc>,
}

impl Session {
    /// Create a new active session.
    pub fn new(workspace: impl Into<String>, model: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            workspace: workspace.into(),
            model: model.into(),
            status: SessionStatus::Active,
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Append a message and bump `updated_at`.
    pub fn push_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    /// Number of messages in the session.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

/// A single agent turn (one model response cycle, optionally with tools).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    /// Zero-based turn index within a run.
    pub index: u32,
    /// Assistant message produced this turn (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_message: Option<Message>,
    /// Tool result messages produced this turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_messages: Vec<Message>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;

    #[test]
    fn session_push_message() {
        let mut session = Session::new("/tmp/ws", "default");
        session.push_message(Message::user("hello"));
        assert_eq!(session.message_count(), 1);
        assert_eq!(session.status, SessionStatus::Active);

        let raw = serde_json::to_string_pretty(&session).unwrap();
        let back: Session = serde_json::from_str(&raw).unwrap();
        assert_eq!(session, back);
    }
}
