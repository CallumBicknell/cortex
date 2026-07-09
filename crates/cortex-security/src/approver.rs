//! Approvers with policy + audit.

use crate::audit::{AuditRecord, AuditSink};
use crate::policy::SecurityPolicy;
use crate::redact::redacted_json;
use async_trait::async_trait;
use cortex_tools::{AlwaysAllow, AlwaysDeny, ApprovalDecision, ApprovalRequest, Approver};
use std::sync::Arc;

/// Wraps an inner approver, applies policy-level hard denials, redacts args for audit.
pub struct PolicyApprover {
    policy: Arc<SecurityPolicy>,
    inner: Arc<dyn Approver>,
    audit: Arc<dyn AuditSink>,
    session_id: Option<cortex_common::SessionId>,
}

impl PolicyApprover {
    /// Create a policy-aware approver.
    pub fn new(
        policy: Arc<SecurityPolicy>,
        inner: Arc<dyn Approver>,
        audit: Arc<dyn AuditSink>,
        session_id: Option<cortex_common::SessionId>,
    ) -> Self {
        Self {
            policy,
            inner,
            audit,
            session_id,
        }
    }

    /// YOLO approver (always allow) with audit.
    pub fn yolo(policy: Arc<SecurityPolicy>, audit: Arc<dyn AuditSink>) -> Self {
        Self::new(policy, Arc::new(AlwaysAllow), audit, None)
    }

    /// Always deny with audit.
    pub fn deny_all(policy: Arc<SecurityPolicy>, audit: Arc<dyn AuditSink>) -> Self {
        Self::new(policy, Arc::new(AlwaysDeny), audit, None)
    }

    fn emit(
        &self,
        request: &ApprovalRequest,
        decision: ApprovalDecision,
        reason: impl Into<String>,
    ) {
        let args = redacted_json(&request.arguments);
        let rec = AuditRecord::new(
            self.session_id,
            &request.tool_name,
            Some(request.tool_call_id),
            decision,
            reason,
            Some(args),
        );
        self.audit.record(rec);
    }
}

#[async_trait]
impl Approver for PolicyApprover {
    async fn approve(&self, request: &ApprovalRequest) -> ApprovalDecision {
        // Hard-deny dangerous shell patterns even under yolo? Under yolo we still
        // block catastrophic patterns for safety.
        if request.tool_name == "shell" {
            if let Some(cmd) = request.arguments.get("command").and_then(|v| v.as_str()) {
                if !self.policy.shell_command_allowed(cmd) {
                    self.emit(request, ApprovalDecision::Deny, "shell_deny_pattern");
                    return ApprovalDecision::Deny;
                }
            }
        }

        if self.policy.yolo {
            self.emit(request, ApprovalDecision::Allow, "yolo");
            return ApprovalDecision::Allow;
        }

        let decision = self.inner.approve(request).await;
        let reason = match decision {
            ApprovalDecision::Allow => "approver_allow",
            ApprovalDecision::Deny => "approver_deny",
        };
        self.emit(request, decision, reason);
        decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::MemoryAuditSink;
    use cortex_common::ToolCallId;
    use serde_json::json;

    #[tokio::test]
    async fn blocks_rm_rf_root_even_yolo() {
        let policy = Arc::new(SecurityPolicy::default().with_yolo(true));
        let sink = Arc::new(MemoryAuditSink::default());
        let approver = PolicyApprover::yolo(policy, sink.clone());
        let req = ApprovalRequest {
            tool_name: "shell".into(),
            tool_call_id: ToolCallId::new(),
            arguments: json!({"command": "rm -rf /"}),
            summary: "shell".into(),
        };
        assert_eq!(approver.approve(&req).await, ApprovalDecision::Deny);
        let records = sink.snapshot();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].decision, "deny");
    }
}
