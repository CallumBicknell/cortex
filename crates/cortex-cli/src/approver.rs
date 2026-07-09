//! Interactive CLI approval for sensitive tools.

use async_trait::async_trait;
use cortex_tools::{ApprovalDecision, ApprovalRequest, Approver};
use std::io::{self, BufRead, Write};

/// Approves tool calls via stdin, or auto-allows when `yolo` is set.
pub struct CliApprover {
    yolo: bool,
}

impl CliApprover {
    /// Create an approver. When `yolo` is true, all tools are allowed.
    pub fn new(yolo: bool) -> Self {
        Self { yolo }
    }
}

#[async_trait]
impl Approver for CliApprover {
    async fn approve(&self, request: &ApprovalRequest) -> ApprovalDecision {
        if self.yolo {
            return ApprovalDecision::Allow;
        }

        let tool_name = request.tool_name.clone();
        let summary = request.summary.clone();
        let args = cortex_security::redacted_json(&request.arguments).to_string();

        tokio::task::spawn_blocking(move || {
            let mut stdout = io::stdout();
            let _ = writeln!(
                stdout,
                "\n⚠️  Approve tool `{tool_name}`?\n    {summary}\n    args: {args}\n    [y]es / [N]o: "
            );
            let _ = stdout.flush();
            let stdin = io::stdin();
            let mut line = String::new();
            if stdin.lock().read_line(&mut line).is_err() {
                return ApprovalDecision::Deny;
            }
            let answer = line.trim().to_ascii_lowercase();
            if matches!(answer.as_str(), "y" | "yes") {
                ApprovalDecision::Allow
            } else {
                ApprovalDecision::Deny
            }
        })
        .await
        .unwrap_or(ApprovalDecision::Deny)
    }
}
