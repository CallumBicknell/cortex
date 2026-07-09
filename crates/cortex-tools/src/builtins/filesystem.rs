//! Filesystem tools.

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

fn require_path(input: &Value) -> Result<String> {
    input
        .get("path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ToolError::InvalidInput("missing string field `path`".into()))
}

/// Read a UTF-8 text file.
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a text file under the workspace."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path relative to workspace root" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let path = require_path(&input)?;
        let full = ctx.resolve_path(&path)?;
        let content = fs::read_to_string(&full)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to read {}: {e}", full.display())))?;
        Ok(content)
    }
}

/// Write (create or overwrite) a text file.
pub struct WriteFileTool;

#[derive(Deserialize)]
struct WriteInput {
    path: String,
    content: String,
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file under the workspace (creates parents as needed)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: WriteInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid write_file args: {e}")))?;
        let full = ctx.resolve_path(&args.path)?;
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&full, args.content.as_bytes()).await?;
        Ok(format!(
            "wrote {} bytes to {}",
            args.content.len(),
            args.path
        ))
    }
}

/// Replace an exact string occurrence in a file.
pub struct EditFileTool;

#[derive(Deserialize)]
struct EditInput {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing old_string with new_string. Fails if old_string is not found or is ambiguous (unless replace_all)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "old_string": { "type": "string" },
                "new_string": { "type": "string" },
                "replace_all": { "type": "boolean", "default": false }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: EditInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid edit_file args: {e}")))?;
        if args.old_string.is_empty() {
            return Err(ToolError::InvalidInput(
                "old_string must not be empty".into(),
            ));
        }
        let full = ctx.resolve_path(&args.path)?;
        let original = fs::read_to_string(&full)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to read {}: {e}", full.display())))?;
        let count = original.matches(&args.old_string).count();
        if count == 0 {
            return Err(ToolError::Execution("old_string not found in file".into()));
        }
        if count > 1 && !args.replace_all {
            return Err(ToolError::Execution(format!(
                "old_string found {count} times; set replace_all=true or provide a unique string"
            )));
        }
        let updated = if args.replace_all {
            original.replace(&args.old_string, &args.new_string)
        } else {
            original.replacen(&args.old_string, &args.new_string, 1)
        };
        fs::write(&full, updated.as_bytes()).await?;
        Ok(format!(
            "edited {} ({} replacement{})",
            args.path,
            if args.replace_all { count } else { 1 },
            if args.replace_all && count != 1 {
                "s"
            } else {
                ""
            }
        ))
    }
}

/// List directory entries.
pub struct ListDirTool;

#[async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List files and directories at a path under the workspace."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path relative to workspace (default \".\")",
                    "default": "."
                }
            }
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let full = ctx.resolve_path(path)?;
        let mut rd = fs::read_dir(&full)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to list {}: {e}", full.display())))?;
        let mut entries = Vec::new();
        while let Some(entry) = rd.next_entry().await? {
            let meta = entry.metadata().await?;
            let kind = if meta.is_dir() { "dir" } else { "file" };
            entries.push(format!("{}\t{}", kind, entry.file_name().to_string_lossy()));
        }
        entries.sort();
        Ok(entries.join("\n"))
    }
}

/// Glob for files under the workspace.
pub struct GlobFilesTool;

#[async_trait]
impl Tool for GlobFilesTool {
    fn name(&self) -> &str {
        "glob_files"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern under the workspace (e.g. \"**/*.rs\")."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern relative to workspace" }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("missing `pattern`".into()))?;

        // Run glob on a blocking thread; paths are constrained by workspace prefix filter.
        let workspace = ctx
            .workspace_root
            .canonicalize()
            .map_err(|e| ToolError::Execution(format!("workspace: {e}")))?;
        let full_pattern = workspace.join(pattern);
        let pattern_str = full_pattern.to_string_lossy().to_string();
        let workspace_clone = workspace.clone();

        let matches = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let mut out = Vec::new();
            for entry in glob::glob(&pattern_str)
                .map_err(|e| ToolError::InvalidInput(format!("invalid glob: {e}")))?
            {
                let path = entry.map_err(|e| ToolError::Execution(e.to_string()))?;
                if let Ok(canon) = path.canonicalize() {
                    if !canon.starts_with(&workspace_clone) {
                        continue;
                    }
                    let rel = canon
                        .strip_prefix(&workspace_clone)
                        .unwrap_or(Path::new(&canon))
                        .to_string_lossy()
                        .to_string();
                    out.push(rel);
                }
            }
            out.sort();
            Ok(out)
        })
        .await
        .map_err(|e| ToolError::Execution(format!("glob join: {e}")))??;

        if matches.is_empty() {
            Ok("(no matches)".into())
        } else {
            Ok(matches.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::ToolContext;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn read_write_edit_list_glob() {
        let dir = tempdir().unwrap();
        let ctx = ToolContext::for_tests(dir.path());

        WriteFileTool
            .execute(
                &ctx,
                json!({"path": "src/a.rs", "content": "fn main() {}\n"}),
            )
            .await
            .unwrap();

        let content = ReadFileTool
            .execute(&ctx, json!({"path": "src/a.rs"}))
            .await
            .unwrap();
        assert!(content.contains("fn main"));

        EditFileTool
            .execute(
                &ctx,
                json!({
                    "path": "src/a.rs",
                    "old_string": "fn main() {}",
                    "new_string": "fn main() { println!(\"hi\"); }"
                }),
            )
            .await
            .unwrap();

        let listed = ListDirTool
            .execute(&ctx, json!({"path": "src"}))
            .await
            .unwrap();
        assert!(listed.contains("a.rs"));

        let found = GlobFilesTool
            .execute(&ctx, json!({"pattern": "**/*.rs"}))
            .await
            .unwrap();
        assert!(found.contains("src/a.rs") || found.contains("a.rs"));
    }
}
