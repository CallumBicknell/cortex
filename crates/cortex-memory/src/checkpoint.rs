//! Checkpoint records for durable loop state.

use chrono::{DateTime, Utc};
use cortex_common::{CheckpointId, RunId, SessionId};
use serde::{Deserialize, Serialize};

/// Snapshot of agent-loop progress for resume/debug.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckpointState {
    /// Last run id, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// Last known loop phase name.
    pub phase: String,
    /// Turns completed in the last run.
    pub turns: u32,
    /// Optional free-form notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A stored checkpoint row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Checkpoint id.
    pub id: CheckpointId,
    /// Session id.
    pub session_id: SessionId,
    /// Optional label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Loop state payload.
    pub state: CheckpointState,
    /// Message count at checkpoint time.
    pub message_count: u32,
    /// When saved.
    pub created_at: DateTime<Utc>,
}

impl Checkpoint {
    /// Build a new checkpoint.
    pub fn new(
        session_id: SessionId,
        state: CheckpointState,
        message_count: u32,
        label: Option<String>,
    ) -> Self {
        Self {
            id: CheckpointId::new(),
            session_id,
            label,
            state,
            message_count,
            created_at: Utc::now(),
        }
    }
}
