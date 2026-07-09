//! Approval audit records.

use chrono::{DateTime, Utc};
use cortex_common::{SessionId, ToolCallId};
use cortex_tools::ApprovalDecision;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// One approval / policy decision for audit storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Record id.
    pub id: Uuid,
    /// Optional session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    /// Tool name.
    pub tool_name: String,
    /// Tool call id when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<ToolCallId>,
    /// Decision taken.
    pub decision: String,
    /// Why (policy deny, user deny, user allow, yolo, …).
    pub reason: String,
    /// Redacted arguments snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
    /// Timestamp.
    pub created_at: DateTime<Utc>,
}

impl AuditRecord {
    /// Build a record.
    pub fn new(
        session_id: Option<SessionId>,
        tool_name: impl Into<String>,
        tool_call_id: Option<ToolCallId>,
        decision: ApprovalDecision,
        reason: impl Into<String>,
        arguments: Option<Value>,
    ) -> Self {
        let decision = match decision {
            ApprovalDecision::Allow => "allow",
            ApprovalDecision::Deny => "deny",
        };
        Self {
            id: Uuid::new_v4(),
            session_id,
            tool_name: tool_name.into(),
            tool_call_id,
            decision: decision.into(),
            reason: reason.into(),
            arguments,
            created_at: Utc::now(),
        }
    }
}

/// Sink for audit records (memory store, log, …).
pub trait AuditSink: Send + Sync {
    /// Persist an audit record (best-effort; errors are logged by caller).
    fn record(&self, entry: AuditRecord);
}

/// No-op sink.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullAuditSink;

impl AuditSink for NullAuditSink {
    fn record(&self, _entry: AuditRecord) {}
}

/// Collects audit records in memory (tests).
#[derive(Debug, Default)]
pub struct MemoryAuditSink {
    /// Collected records.
    pub records: std::sync::Mutex<Vec<AuditRecord>>,
}

impl AuditSink for MemoryAuditSink {
    fn record(&self, entry: AuditRecord) {
        if let Ok(mut g) = self.records.lock() {
            g.push(entry);
        }
    }
}

impl MemoryAuditSink {
    /// Snapshot of records.
    pub fn snapshot(&self) -> Vec<AuditRecord> {
        self.records.lock().map(|g| g.clone()).unwrap_or_default()
    }
}
