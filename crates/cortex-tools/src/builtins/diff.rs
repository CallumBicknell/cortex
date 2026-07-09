//! Apply a unified-ish patch / search-replace multi-hunk file update.

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::fs;

/// Apply sequential search/replace hunks to a file (safer than free-form patch).
pub struct ApplyPatchTool;

#[derive(Deserialize)]
struct Hunk {
    old_string: String,
    new_string: String,
}

#[derive(Deserialize)]
struct PatchInput {
    path: String,
    hunks: Vec<Hunk>,
}

#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply one or more exact search/replace hunks to a file under the workspace."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "hunks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_string": { "type": "string" },
                            "new_string": { "type": "string" }
                        },
                        "required": ["old_string", "new_string"]
                    }
                }
            },
            "required": ["path", "hunks"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: PatchInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid apply_patch args: {e}")))?;
        if args.hunks.is_empty() {
            return Err(ToolError::InvalidInput("hunks must not be empty".into()));
        }
        let full = ctx.resolve_path(&args.path)?;
        let mut content = fs::read_to_string(&full)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to read {}: {e}", full.display())))?;
        let mut applied = 0usize;
        for (i, hunk) in args.hunks.iter().enumerate() {
            if hunk.old_string.is_empty() {
                return Err(ToolError::InvalidInput(format!(
                    "hunk {i}: old_string must not be empty"
                )));
            }
            let count = content.matches(&hunk.old_string).count();
            if count == 0 {
                return Err(ToolError::Execution(format!(
                    "hunk {i}: old_string not found"
                )));
            }
            if count > 1 {
                return Err(ToolError::Execution(format!(
                    "hunk {i}: old_string found {count} times; make it unique"
                )));
            }
            content = content.replacen(&hunk.old_string, &hunk.new_string, 1);
            applied += 1;
        }
        fs::write(&full, content.as_bytes()).await?;
        Ok(format!("applied {applied} hunk(s) to {}", args.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::ToolContext;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn applies_two_hunks() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\nthree\n").unwrap();
        let ctx = ToolContext::for_tests(dir.path());
        ApplyPatchTool
            .execute(
                &ctx,
                json!({
                    "path": "a.txt",
                    "hunks": [
                        {"old_string": "one", "new_string": "1"},
                        {"old_string": "three", "new_string": "3"}
                    ]
                }),
            )
            .await
            .unwrap();
        let out = std::fs::read_to_string(dir.path().join("a.txt")).unwrap();
        assert_eq!(out, "1\ntwo\n3\n");
    }
}
