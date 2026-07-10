//! Interactive TUI approval for sensitive tools.
//!
//! Sends an [`ApprovalRequest`] to the TUI event loop and waits for the user's
//! Allow / Deny response via a oneshot channel.

use async_trait::async_trait;
use cortex_tools::{ApprovalDecision, ApprovalRequest, Approver};
use tokio::sync::{mpsc, oneshot};

/// A request forwarded from the [`TuiApprover`] to the TUI event loop.
pub struct TuiApprovalRequest {
    /// The original tool approval request.
    pub request: ApprovalRequest,
    /// Channel to send the user's decision back.
    pub respond: oneshot::Sender<ApprovalDecision>,
}

/// Approver that delegates to the TUI modal overlay.
///
/// When a tool needs approval, the [`TuiApprover`] sends a [`TuiApprovalRequest`]
/// to the TUI event loop, which renders an Allow / Deny modal. The user's
/// response is sent back through the oneshot channel.
pub struct TuiApprover {
    tx: mpsc::UnboundedSender<TuiApprovalRequest>,
}

impl TuiApprover {
    /// Create a new approver bound to the given channel sender.
    pub fn new(tx: mpsc::UnboundedSender<TuiApprovalRequest>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl Approver for TuiApprover {
    async fn approve(&self, request: &ApprovalRequest) -> ApprovalDecision {
        let (respond, rx) = oneshot::channel();
        let req = TuiApprovalRequest {
            request: request.clone(),
            respond,
        };
        if self.tx.send(req).is_err() {
            return ApprovalDecision::Deny;
        }
        rx.await.unwrap_or(ApprovalDecision::Deny)
    }
}
