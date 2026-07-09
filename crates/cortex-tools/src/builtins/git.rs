//! Git tools.

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::process::Command;

async fn run_git(ctx: &ToolContext, args: &[&str]) -> Result<String> {
    ctx.check_cancelled()?;
    let output = Command::new("git")
        .args(args)
        .current_dir(&ctx.workspace_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| ToolError::Execution(format!("failed to run git: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        let mut msg = stderr;
        if msg.is_empty() {
            msg = stdout;
        }
        return Err(ToolError::Execution(format!(
            "git {} failed: {}",
            args.join(" "),
            msg.trim()
        )));
    }
    let mut text = stdout;
    if text.is_empty() && !stderr.is_empty() {
        text = stderr;
    }
    if text.is_empty() {
        text = "(no output)".into();
    }
    Ok(ctx.truncate_output(text))
}

/// `git status --short`
pub struct GitStatusTool;

#[async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Show git working tree status (short format)."
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, ctx: &ToolContext, _input: Value) -> Result<String> {
        run_git(ctx, &["status", "--short"]).await
    }
}

/// `git diff` (optional path)
pub struct GitDiffTool;

#[async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }

    fn description(&self) -> &str {
        "Show git diff. Optional path and staged flag."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "staged": { "type": "boolean", "default": false }
            }
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        let staged = input
            .get("staged")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let mut args = vec!["diff"];
        if staged {
            args.push("--cached");
        }
        let path = input.get("path").and_then(|v| v.as_str());
        if let Some(p) = path {
            // Ensure path stays in workspace.
            let _ = ctx.resolve_path(p)?;
            args.push("--");
            args.push(p);
        }
        run_git(ctx, &args).await
    }
}

/// `git log` (limited)
pub struct GitLogTool;

#[async_trait]
impl Tool for GitLogTool {
    fn name(&self) -> &str {
        "git_log"
    }

    fn description(&self) -> &str {
        "Show recent git commits (oneline)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": { "type": "integer", "default": 10, "minimum": 1, "maximum": 100 }
            }
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .clamp(1, 100);
        let n = format!("-{limit}");
        run_git(ctx, &["log", &n, "--oneline", "--decorate"]).await
    }
}

/// `git add`
pub struct GitAddTool;

#[derive(Deserialize)]
struct GitAddInput {
    paths: Vec<String>,
}

#[async_trait]
impl Tool for GitAddTool {
    fn name(&self) -> &str {
        "git_add"
    }

    fn description(&self) -> &str {
        "Stage files with git add."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Paths to stage (use [\".\"] for all)"
                }
            },
            "required": ["paths"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        let args: GitAddInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid git_add args: {e}")))?;
        if args.paths.is_empty() {
            return Err(ToolError::InvalidInput("paths must not be empty".into()));
        }
        for p in &args.paths {
            if p != "." {
                let _ = ctx.resolve_path(p)?;
            }
        }
        let mut cmd_args = vec!["add".to_string()];
        cmd_args.extend(args.paths);
        let refs: Vec<&str> = cmd_args.iter().map(|s| s.as_str()).collect();
        run_git(ctx, &refs).await
    }
}

/// `git commit`
pub struct GitCommitTool;

#[derive(Deserialize)]
struct GitCommitInput {
    message: String,
}

#[async_trait]
impl Tool for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }

    fn description(&self) -> &str {
        "Create a git commit with the given message (requires staged changes)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        let args: GitCommitInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid git_commit args: {e}")))?;
        if args.message.trim().is_empty() {
            return Err(ToolError::InvalidInput("message must not be empty".into()));
        }
        run_git(ctx, &["commit", "-m", &args.message]).await
    }
}
