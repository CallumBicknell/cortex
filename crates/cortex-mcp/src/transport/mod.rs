//! MCP transports: stdio and Streamable HTTP (with legacy SSE fallback).

mod http;
mod stdio;

pub use http::HttpTransport;
pub use stdio::StdioTransport;

use crate::error::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Common interface for MCP message exchange.
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// JSON-RPC request → result value (or error).
    async fn request(&self, method: &str, params: Option<Value>) -> Result<Value>;
    /// Fire-and-forget notification.
    async fn notify(&self, method: &str, params: Option<Value>) -> Result<()>;
}
