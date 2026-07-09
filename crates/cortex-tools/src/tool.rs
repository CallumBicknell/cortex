//! Tool trait and execution context.

use crate::error::{Result, ToolError};
use crate::permissions::PermissionPolicy;
use async_trait::async_trait;
use cortex_common::{SessionId, ToolCallId};
use cortex_models::{ToolCall, ToolResult, ToolSpec};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Request for human/policy approval before a tool runs.
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    /// Tool name.
    pub tool_name: String,
    /// Tool call id.
    pub tool_call_id: ToolCallId,
    /// JSON arguments.
    pub arguments: Value,
    /// Short human summary.
    pub summary: String,
}

/// Approval outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Proceed.
    Allow,
    /// Reject.
    Deny,
}

/// Approves or denies sensitive tool invocations.
#[async_trait]
pub trait Approver: Send + Sync {
    /// Decide whether a tool may run.
    async fn approve(&self, request: &ApprovalRequest) -> ApprovalDecision;
}

/// Approver that always allows (tests / trusted mode).
#[derive(Debug, Default, Clone, Copy)]
pub struct AlwaysAllow;

#[async_trait]
impl Approver for AlwaysAllow {
    async fn approve(&self, _request: &ApprovalRequest) -> ApprovalDecision {
        ApprovalDecision::Allow
    }
}

/// Approver that always denies.
#[derive(Debug, Default, Clone, Copy)]
pub struct AlwaysDeny;

#[async_trait]
impl Approver for AlwaysDeny {
    async fn approve(&self, _request: &ApprovalRequest) -> ApprovalDecision {
        ApprovalDecision::Deny
    }
}

/// Context passed to every tool invocation.
#[derive(Clone)]
pub struct ToolContext {
    /// Workspace root (absolute preferred).
    pub workspace_root: PathBuf,
    /// Optional session id.
    pub session_id: Option<SessionId>,
    /// Cancellation token for the run.
    pub cancel: CancellationToken,
    /// Permission policy.
    pub permissions: Arc<PermissionPolicy>,
    /// Approval hook.
    pub approver: Arc<dyn Approver>,
    /// Default timeout for tools that support it.
    pub default_timeout: Duration,
}

impl ToolContext {
    /// Create a context with allow-all permissions (handy for unit tests).
    pub fn for_tests(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            session_id: None,
            cancel: CancellationToken::new(),
            permissions: Arc::new(PermissionPolicy::default().allow_all()),
            approver: Arc::new(AlwaysAllow),
            default_timeout: Duration::from_secs(30),
        }
    }

    /// Resolve a path under the workspace sandbox.
    pub fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        self.permissions.resolve_path(&self.workspace_root, path)
    }

    /// Check cancellation.
    pub fn check_cancelled(&self) -> Result<()> {
        if self.cancel.is_cancelled() {
            Err(ToolError::Cancelled("tool context cancelled".into()))
        } else {
            Ok(())
        }
    }

    /// Truncate output to policy max.
    pub fn truncate_output(&self, mut output: String) -> String {
        let max = self.permissions.max_output_bytes;
        if output.len() > max {
            output.truncate(max);
            output.push_str("\n...[truncated]");
        }
        output
    }
}

/// A capability the agent can invoke.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name (stable identifier for the model).
    fn name(&self) -> &str;

    /// Human description for the model.
    fn description(&self) -> &str;

    /// JSON Schema for parameters.
    fn parameters_schema(&self) -> Value;

    /// Full tool spec for LLM tool-calling.
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(self.name(), self.description()).with_parameters(self.parameters_schema())
    }

    /// Execute the tool.
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String>;
}

/// Helper: run a tool and wrap into a [`ToolResult`].
pub async fn run_tool(tool: &dyn Tool, ctx: &ToolContext, call: &ToolCall) -> ToolResult {
    let start = std::time::Instant::now();
    match tool.execute(ctx, call.arguments.clone()).await {
        Ok(output) => {
            let output = ctx.truncate_output(output);
            ToolResult::success(call.id, tool.name(), output).with_duration(start.elapsed())
        }
        Err(err) => {
            ToolResult::error(call.id, tool.name(), err.to_output()).with_duration(start.elapsed())
        }
    }
}
