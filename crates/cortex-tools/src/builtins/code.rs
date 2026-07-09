//! Code intelligence tools (tree-sitter outlines).

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use cortex_parse::{format_outline, outline_file};
use serde::Deserialize;
use serde_json::{json, Value};

/// Return a symbol outline for a source file (Rust / Python).
pub struct CodeOutlineTool;

#[derive(Deserialize)]
struct OutlineInput {
    path: String,
}

#[async_trait]
impl Tool for CodeOutlineTool {
    fn name(&self) -> &str {
        "code_outline"
    }

    fn description(&self) -> &str {
        "Extract a structural outline (functions, types, classes) from a Rust or Python source file using tree-sitter."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path relative to the workspace"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: OutlineInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid code_outline args: {e}")))?;
        let path = ctx.resolve_path(&args.path)?;
        let outline = outline_file(&path).map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(ctx.truncate_output(format_outline(&outline)))
    }
}
