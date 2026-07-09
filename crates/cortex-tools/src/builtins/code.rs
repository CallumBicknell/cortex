//! Code intelligence tools (tree-sitter outlines + workspace symbols).

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use cortex_parse::{
    find_definitions, format_definitions, format_outline, format_symbol_hits, index_workspace,
    outline_file, search_symbols,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

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

/// Cached workspace symbol index (per process, invalidated after 60s).
struct SymbolCache {
    root: String,
    built_at: Instant,
    symbols: Vec<cortex_parse::IndexedSymbol>,
}

static CACHE: OnceLock<Mutex<Option<SymbolCache>>> = OnceLock::new();

fn get_index(
    workspace: &std::path::Path,
    max_files: usize,
) -> Result<Vec<cortex_parse::IndexedSymbol>> {
    let key = workspace.to_string_lossy().to_string();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache
        .lock()
        .map_err(|_| ToolError::Execution("symbol cache lock".into()))?;
    if let Some(c) = guard.as_ref() {
        if c.root == key && c.built_at.elapsed().as_secs() < 60 {
            return Ok(c.symbols.clone());
        }
    }
    let (symbols, _errs) =
        index_workspace(workspace, max_files).map_err(|e| ToolError::Execution(e.to_string()))?;
    *guard = Some(SymbolCache {
        root: key,
        built_at: Instant::now(),
        symbols: symbols.clone(),
    });
    Ok(symbols)
}

/// Search symbols across the workspace.
pub struct WorkspaceSymbolsTool;

#[derive(Deserialize)]
struct SymbolsInput {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_max_files")]
    max_files: usize,
}

fn default_limit() -> usize {
    20
}
fn default_max_files() -> usize {
    400
}

#[async_trait]
impl Tool for WorkspaceSymbolsTool {
    fn name(&self) -> &str {
        "workspace_symbols"
    }

    fn description(&self) -> &str {
        "Search functions/types/classes across the workspace (Rust/Python) by name. Lightweight LSP-style workspace/symbol."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer", "default": 20 },
                "max_files": { "type": "integer", "default": 400 }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: SymbolsInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid workspace_symbols args: {e}")))?;
        if args.query.trim().is_empty() {
            return Err(ToolError::InvalidInput("query must not be empty".into()));
        }
        let symbols = get_index(&ctx.workspace_root, args.max_files.clamp(10, 2000))?;
        let hits = search_symbols(&symbols, &args.query, args.limit.clamp(1, 100));
        Ok(ctx.truncate_output(format_symbol_hits(&hits)))
    }
}

/// Find definitions for a symbol name.
pub struct CodeDefinitionTool;

#[derive(Deserialize)]
struct DefInput {
    name: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[async_trait]
impl Tool for CodeDefinitionTool {
    fn name(&self) -> &str {
        "code_definition"
    }

    fn description(&self) -> &str {
        "Find likely definitions of a symbol name across the workspace (exact/prefix match). Lightweight go-to-definition."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "limit": { "type": "integer", "default": 20 }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: DefInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid code_definition args: {e}")))?;
        if args.name.trim().is_empty() {
            return Err(ToolError::InvalidInput("name must not be empty".into()));
        }
        let symbols = get_index(&ctx.workspace_root, 400)?;
        let defs = find_definitions(&symbols, args.name.trim(), args.limit.clamp(1, 50));
        Ok(ctx.truncate_output(format_definitions(&defs)))
    }
}
