//! Tool executor with permissions, approval, and safe parallel batches.

use crate::error::{Result, ToolError};
use crate::parallel::is_parallel_safe;
use crate::permissions::PermissionMode;
use crate::registry::ToolRegistry;
use crate::tool::{run_tool, ApprovalDecision, ApprovalRequest, ToolContext};
use cortex_models::{ToolCall, ToolResult};
use futures::future::join_all;
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

    /// Execute multiple tool calls with **safe parallel batches**.
    ///
    /// Consecutive [`is_parallel_safe`] tools run concurrently via `join_all`.
    /// Mutating / shell / nested-agent tools run strictly serially and never
    /// share a batch with other tools.
    pub async fn execute_all(&self, ctx: &ToolContext, calls: &[ToolCall]) -> Vec<ToolResult> {
        if calls.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<Option<ToolResult>> = (0..calls.len()).map(|_| None).collect();
        let mut i = 0;
        while i < calls.len() {
            if ctx.cancel.is_cancelled() {
                for j in i..calls.len() {
                    out[j] = Some(ToolResult::error(
                        calls[j].id,
                        &calls[j].name,
                        "cancelled before execution",
                    ));
                }
                break;
            }

            if is_parallel_safe(&calls[i].name) {
                let start = i;
                while i < calls.len() && is_parallel_safe(&calls[i].name) {
                    i += 1;
                }
                let batch: Vec<usize> = (start..i).collect();
                if batch.len() == 1 {
                    let idx = batch[0];
                    out[idx] = Some(self.execute(ctx, &calls[idx]).await);
                } else {
                    info!(count = batch.len(), "parallel tool batch");
                    let futs: Vec<_> = batch
                        .iter()
                        .map(|&idx| {
                            let call = calls[idx].clone();
                            let exec = self.clone();
                            let ctx = ctx.clone();
                            async move { (idx, exec.execute(&ctx, &call).await) }
                        })
                        .collect();
                    for (idx, result) in join_all(futs).await {
                        out[idx] = Some(result);
                    }
                }
            } else {
                out[i] = Some(self.execute(ctx, &calls[i]).await);
                i += 1;
            }
        }
        out.into_iter()
            .map(|r| r.expect("all tool slots filled"))
            .collect()
    }

    /// Force sequential execution (tests / debugging).
    pub async fn execute_all_serial(
        &self,
        ctx: &ToolContext,
        calls: &[ToolCall],
    ) -> Vec<ToolResult> {
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
    async fn parallel_batch_reads() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "A").unwrap();
        std::fs::write(dir.path().join("b.txt"), "B").unwrap();
        let mut reg = ToolRegistry::new();
        register_default_tools(&mut reg).unwrap();
        let exec = ToolExecutor::new(Arc::new(reg));
        let ctx = ToolContext::for_tests(dir.path());
        let calls = vec![
            ToolCall::new("read_file", json!({"path": "a.txt"})),
            ToolCall::new("read_file", json!({"path": "b.txt"})),
        ];
        let results = exec.execute_all(&ctx, &calls).await;
        assert_eq!(results.len(), 2);
        assert!(!results[0].is_error);
        assert!(!results[1].is_error);
        assert_eq!(results[0].output, "A");
        assert_eq!(results[1].output, "B");
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
