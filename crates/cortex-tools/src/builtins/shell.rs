//! Shell command tool.

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Run a shell command in the workspace.
pub struct ShellTool;

#[derive(Deserialize)]
struct ShellInput {
    command: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Run a shell command in the workspace. Prefer specialized tools when available."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Command string passed to `sh -c`" },
                "cwd": { "type": "string", "description": "Working directory relative to workspace" },
                "timeout_secs": { "type": "integer", "description": "Timeout in seconds" }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: ShellInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid shell args: {e}")))?;
        if args.command.trim().is_empty() {
            return Err(ToolError::InvalidInput("command must not be empty".into()));
        }

        if !ctx.permissions.shell_command_allowed(&args.command) {
            return Err(ToolError::PermissionDenied(
                "shell command matches a denied security pattern".into(),
            ));
        }

        let cwd = if let Some(rel) = args.cwd {
            ctx.resolve_path(&rel)?
        } else {
            ctx.workspace_root.clone()
        };

        let timeout_dur = Duration::from_secs(
            args.timeout_secs
                .unwrap_or(ctx.permissions.shell_timeout_secs)
                .max(1),
        );

        // Scrub secrets from the child environment.
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(&args.command)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for key in &ctx.permissions.scrub_env {
            cmd.env_remove(key);
        }

        let child_fut = cmd.output();
        let output = tokio::select! {
            _ = ctx.cancel.cancelled() => {
                return Err(ToolError::Cancelled("shell cancelled".into()));
            }
            res = timeout(timeout_dur, child_fut) => {
                match res {
                    Ok(Ok(o)) => o,
                    Ok(Err(e)) => return Err(ToolError::Execution(format!("failed to spawn shell: {e}"))),
                    Err(_) => return Err(ToolError::Timeout(format!(
                        "command timed out after {}s",
                        timeout_dur.as_secs()
                    ))),
                }
            }
        };

        let mut text = String::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.is_empty() {
            text.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str("--- stderr ---\n");
            text.push_str(&stderr);
        }
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            if text.is_empty() {
                text = format!("command failed with exit code {code}");
            } else {
                text.push_str(&format!("\n(exit code {code})"));
            }
            return Err(ToolError::Execution(text));
        }
        if text.is_empty() {
            text = "(no output)".into();
        }
        Ok(ctx.truncate_output(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::ToolContext;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn runs_echo() {
        let dir = tempdir().unwrap();
        let ctx = ToolContext::for_tests(dir.path());
        let out = ShellTool
            .execute(&ctx, json!({"command": "echo hello-cortex"}))
            .await
            .unwrap();
        assert!(out.contains("hello-cortex"));
    }

    #[tokio::test]
    async fn timeout_errors() {
        let dir = tempdir().unwrap();
        let ctx = ToolContext::for_tests(dir.path());
        let err = ShellTool
            .execute(&ctx, json!({"command": "sleep 5", "timeout_secs": 1}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Timeout(_)));
    }

    #[tokio::test]
    async fn denies_dangerous_pattern() {
        let dir = tempdir().unwrap();
        let ctx = ToolContext::for_tests(dir.path());
        let err = ShellTool
            .execute(&ctx, json!({"command": "rm -rf /"}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::PermissionDenied(_)));
    }
}
