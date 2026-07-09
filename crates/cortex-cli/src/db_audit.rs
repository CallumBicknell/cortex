//! Audit sink that persists approval decisions to SQLite (best-effort async).

use cortex_memory::SessionStore;
use cortex_security::{AuditRecord, AuditSink};
use std::sync::Arc;
use tokio::runtime::Handle;
use tracing::warn;

/// Forwards audit records into `permissions_audit` when a runtime handle is available.
pub struct DbAuditSink {
    store: SessionStore,
}

impl DbAuditSink {
    /// Create a sink backed by an open session store.
    pub fn new(store: SessionStore) -> Self {
        Self { store }
    }
}

impl AuditSink for DbAuditSink {
    fn record(&self, entry: AuditRecord) {
        let store = self.store.clone();
        let detail = serde_json::json!({
            "id": entry.id.to_string(),
            "tool_call_id": entry.tool_call_id.map(|id| id.to_string()),
            "reason": entry.reason,
            "arguments": entry.arguments,
            "created_at": entry.created_at.to_rfc3339(),
        });
        let session_id = entry.session_id;
        let tool_name = entry.tool_name;
        let decision = entry.decision;

        // Prefer spawning on the current tokio runtime; fall back to logging.
        if let Ok(handle) = Handle::try_current() {
            handle.spawn(async move {
                if let Err(err) = store
                    .save_permission_audit(session_id, &tool_name, &decision, &detail)
                    .await
                {
                    warn!(error = %err, "failed to persist permission audit");
                }
            });
        } else {
            warn!("no tokio runtime; dropping permission audit for {tool_name}");
        }
    }
}

/// Wrap optional store into an audit sink.
pub fn audit_sink_for(store: Option<&SessionStore>) -> Arc<dyn AuditSink> {
    match store {
        Some(s) => Arc::new(DbAuditSink::new(s.clone())),
        None => Arc::new(cortex_security::NullAuditSink),
    }
}
