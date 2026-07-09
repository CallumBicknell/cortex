//! Memory search tool (local vector store).

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use cortex_memory::{format_retrieval_section, VectorStore};
use serde::Deserialize;
use serde_json::{json, Value};

/// Shared handle for memory tools.
#[derive(Clone)]
pub struct MemoryHandle {
    store: VectorStore,
    /// Default collection name (usually workspace path).
    collection: String,
}

impl MemoryHandle {
    /// Create a handle.
    pub fn new(store: VectorStore, collection: impl Into<String>) -> Self {
        Self {
            store,
            collection: collection.into(),
        }
    }

    /// Underlying store.
    pub fn store(&self) -> &VectorStore {
        &self.store
    }

    /// Collection id.
    pub fn collection(&self) -> &str {
        &self.collection
    }
}

/// Semantic search over indexed workspace / notes.
pub struct MemorySearchTool {
    handle: MemoryHandle,
}

impl MemorySearchTool {
    /// Create tool.
    pub fn new(handle: MemoryHandle) -> Self {
        Self { handle }
    }
}

#[derive(Deserialize)]
struct SearchInput {
    query: String,
    #[serde(default = "default_k")]
    top_k: usize,
}

fn default_k() -> usize {
    5
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }

    fn description(&self) -> &str {
        "Semantic search over the workspace memory index (local embeddings). \
         Index with `cortex memory index` first."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "top_k": { "type": "integer", "default": 5, "minimum": 1, "maximum": 20 }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: SearchInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid memory_search args: {e}")))?;
        if args.query.trim().is_empty() {
            return Err(ToolError::InvalidInput("query must not be empty".into()));
        }
        let top_k = args.top_k.clamp(1, 20);
        let hits = self
            .handle
            .store
            .search_text_local(self.handle.collection(), args.query.trim(), top_k)
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        if hits.is_empty() {
            return Ok("no hits (index empty or no match). Run: cortex memory index".into());
        }
        let section = format_retrieval_section(&hits, 12_000);
        Ok(ctx.truncate_output(section))
    }
}
