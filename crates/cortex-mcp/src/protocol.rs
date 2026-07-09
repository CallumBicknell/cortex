//! Minimal MCP JSON-RPC message types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC request.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    /// Protocol version string.
    pub jsonrpc: &'static str,
    /// Request id.
    pub id: u64,
    /// Method name.
    pub method: String,
    /// Optional params.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    /// Build a request.
    pub fn new(id: u64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }
}

/// JSON-RPC response (success or error).
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    /// Response id.
    pub id: Option<Value>,
    /// Success result.
    #[serde(default)]
    pub result: Option<Value>,
    /// Error object.
    #[serde(default)]
    pub error: Option<JsonRpcErrorObject>,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcErrorObject {
    /// Error code.
    pub code: i64,
    /// Error message.
    pub message: String,
    /// Optional data payload.
    #[serde(default)]
    #[allow(dead_code)]
    pub data: Option<Value>,
}

/// Tool descriptor from `tools/list`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpToolDescriptor {
    /// Tool name on the server.
    pub name: String,
    /// Human description.
    #[serde(default)]
    pub description: Option<String>,
    /// JSON Schema for arguments.
    #[serde(default, rename = "inputSchema")]
    pub input_schema: Option<Value>,
}

/// Result of `tools/list`.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsListResult {
    /// Tools advertised by the server.
    pub tools: Vec<McpToolDescriptor>,
    /// Pagination cursor.
    #[serde(default)]
    #[allow(dead_code)]
    pub next_cursor: Option<String>,
}

/// Content item from `tools/call`.
#[derive(Debug, Clone, Deserialize)]
pub struct McpContent {
    /// Content type (usually `"text"`).
    #[serde(rename = "type")]
    pub kind: String,
    /// Text payload when present.
    #[serde(default)]
    pub text: Option<String>,
}

/// Result of `tools/call`.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsCallResult {
    /// Content blocks returned by the server.
    #[serde(default)]
    pub content: Vec<McpContent>,
    /// Whether the tool reported an error.
    #[serde(default, rename = "isError", alias = "is_error")]
    pub is_error: Option<bool>,
}

impl ToolsCallResult {
    /// Flatten content to a single string for Cortex ToolResult.
    pub fn as_text(&self) -> String {
        let parts: Vec<String> = self.content.iter().filter_map(|c| c.text.clone()).collect();
        if parts.is_empty() {
            "(empty MCP tool result)".into()
        } else {
            parts.join("\n")
        }
    }
}
