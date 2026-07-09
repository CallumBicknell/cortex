//! Docker run tool with basic resource limits.

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Run a command inside a Docker container (requires `docker` on PATH).
pub struct DockerRunTool;

#[derive(Deserialize)]
struct DockerInput {
    image: String,
    #[serde(default)]
    command: Vec<String>,
    #[serde(default)]
    workdir: Option<String>,
    #[serde(default = "default_network")]
    network: String,
    #[serde(default = "default_memory")]
    memory: String,
    #[serde(default = "default_cpus")]
    cpus: String,
    #[serde(default)]
    timeout_secs: Option<u64>,
    /// Mount workspace read-only at /workspace (default true).
    #[serde(default = "default_true")]
    mount_workspace: bool,
}

fn default_network() -> String {
    "none".into()
}
fn default_memory() -> String {
    "512m".into()
}
fn default_cpus() -> String {
    "1".into()
}
fn default_true() -> bool {
    true
}

#[async_trait]
impl Tool for DockerRunTool {
    fn name(&self) -> &str {
        "docker_run"
    }

    fn description(&self) -> &str {
        "Run a command in a Docker container with network=none by default, memory/cpu limits, optional workspace mount at /workspace."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "image": { "type": "string" },
                "command": { "type": "array", "items": { "type": "string" } },
                "workdir": { "type": "string" },
                "network": { "type": "string", "default": "none" },
                "memory": { "type": "string", "default": "512m" },
                "cpus": { "type": "string", "default": "1" },
                "timeout_secs": { "type": "integer" },
                "mount_workspace": { "type": "boolean", "default": true }
            },
            "required": ["image"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: DockerInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid docker_run args: {e}")))?;
        if args.image.trim().is_empty() {
            return Err(ToolError::InvalidInput("image must not be empty".into()));
        }

        // Basic image name safety: no spaces / shell metacharacters.
        if args
            .image
            .chars()
            .any(|c| c.is_whitespace() || ";&|$`".contains(c))
        {
            return Err(ToolError::InvalidInput("invalid image name".into()));
        }

        let mut cmd = Command::new("docker");
        cmd.arg("run")
            .arg("--rm")
            .arg("--network")
            .arg(&args.network)
            .arg("--memory")
            .arg(&args.memory)
            .arg("--cpus")
            .arg(&args.cpus)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if args.mount_workspace {
            let ws = ctx
                .workspace_root
                .canonicalize()
                .map_err(|e| ToolError::Execution(e.to_string()))?;
            cmd.arg("-v").arg(format!("{}:/workspace:ro", ws.display()));
            cmd.arg("-w")
                .arg(args.workdir.as_deref().unwrap_or("/workspace"));
        } else if let Some(wd) = &args.workdir {
            cmd.arg("-w").arg(wd);
        }

        for key in &ctx.permissions.scrub_env {
            cmd.env_remove(key);
        }

        cmd.arg(&args.image);
        for c in &args.command {
            cmd.arg(c);
        }

        let timeout_dur = Duration::from_secs(
            args.timeout_secs
                .unwrap_or(ctx.permissions.shell_timeout_secs)
                .max(1),
        );

        let output = tokio::select! {
            _ = ctx.cancel.cancelled() => {
                return Err(ToolError::Cancelled("docker_run cancelled".into()));
            }
            res = timeout(timeout_dur, cmd.output()) => {
                match res {
                    Ok(Ok(o)) => o,
                    Ok(Err(e)) => return Err(ToolError::Execution(format!("docker spawn failed: {e}"))),
                    Err(_) => return Err(ToolError::Timeout(format!(
                        "docker_run timed out after {}s",
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
                text = format!("docker failed with exit code {code}");
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
