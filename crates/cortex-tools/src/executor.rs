//! Tool executor with permissions and approval.

use crate::error::{Result, ToolError};
use crate::permissions::PermissionMode;
use crate::registry::ToolRegistry;
use crate::tool::{run_tool, ApprovalDecision, ApprovalRequest, ToolContext};
use cortex_models::{ToolCall, ToolResult};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

/// Executes tool calls against a registry under policy.
#[derive(Clone)]
pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
}

impl ToolExecutor {
    /// Create an executor.
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    /// Access the registry.
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Execute a single tool call.
    pub async fn execute(&self, ctx: &ToolContext, call: &ToolCall) -> ToolResult {
        let start = Instant::now();
        if let Err(err) = ctx.check_cancelled() {
            return ToolResult::error(call.id, &call.name, err.to_output())
                .with_duration(start.elapsed());
        }

        let tool = match self.registry.get(&call.name) {
            Ok(t) => t,
            Err(err) => {
                return ToolResult::error(call.id, &call.name, err.to_output())
                    .with_duration(start.elapsed());
            }
        };

        match self.authorize(ctx, call).await {
            Ok(()) => {}
            Err(err) => {
                warn!(tool = %call.name, error = %err, "tool denied");
                return ToolResult::error(call.id, &call.name, err.to_output())
                    .with_duration(start.elapsed());
            }
        }

        info!(tool = %call.name, id = %call.id, "executing tool");
        run_tool(tool.as_ref(), ctx, call).await
    }

    /// Execute multiple tool calls sequentially.
    pub async fn execute_all(&self, ctx: &ToolContext, calls: &[ToolCall]) -> Vec<ToolResult> {
        let mut out = Vec::with_capacity(calls.len());
        for call in calls {
            if ctx.cancel.is_cancelled() {
                out.push(ToolResult::error(
                    call.id,
                    &call.name,
                    "cancelled before execution",
                ));
                continue;
            }
            out.push(self.execute(ctx, call).await);
        }
        out
    }

    async fn authorize(&self, ctx: &ToolContext, call: &ToolCall) -> Result<()> {
        match ctx.permissions.mode_for(&call.name) {
            PermissionMode::Allow => Ok(()),
            PermissionMode::Deny => Err(ToolError::PermissionDenied(format!(
                "tool `{}` is denied by policy",
                call.name
            ))),
            PermissionMode::Ask => {
                let request = ApprovalRequest {
                    tool_name: call.name.clone(),
                    tool_call_id: call.id,
                    arguments: call.arguments.clone(),
                    summary: format!("Run tool `{}`", call.name),
                };
                match ctx.approver.approve(&request).await {
                    ApprovalDecision::Allow => Ok(()),
                    ApprovalDecision::Deny => Err(ToolError::ApprovalDenied(format!(
                        "tool `{}` was not approved",
                        call.name
                    ))),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::register_default_tools;
    use crate::permissions::PermissionPolicy;
    use crate::tool::{AlwaysDeny, ToolContext};
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn executes_read_file() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("hi.txt"), "hello").unwrap();
        let mut reg = ToolRegistry::new();
        register_default_tools(&mut reg).unwrap();
        let exec = ToolExecutor::new(Arc::new(reg));
        let ctx = ToolContext::for_tests(dir.path());
        let call = ToolCall::new("read_file", json!({"path": "hi.txt"}));
        let result = exec.execute(&ctx, &call).await;
        assert!(!result.is_error);
        assert_eq!(result.output, "hello");
    }

    #[tokio::test]
    async fn deny_mode_blocks() {
        let dir = tempdir().unwrap();
        let mut reg = ToolRegistry::new();
        register_default_tools(&mut reg).unwrap();
        let exec = ToolExecutor::new(Arc::new(reg));
        let mut policy = PermissionPolicy::default();
        policy
            .tools
            .insert("read_file".into(), crate::permissions::PermissionMode::Deny);
        let ctx = ToolContext {
            workspace_root: dir.path().to_path_buf(),
            session_id: None,
            cancel: tokio_util::sync::CancellationToken::new(),
            permissions: Arc::new(policy),
            approver: Arc::new(AlwaysDeny),
            default_timeout: std::time::Duration::from_secs(5),
        };
        let call = ToolCall::new("read_file", json!({"path": "x"}));
        let result = exec.execute(&ctx, &call).await;
        assert!(result.is_error);
        assert!(result.output.contains("denied"));
    }
}
