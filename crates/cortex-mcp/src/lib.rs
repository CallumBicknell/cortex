//! MCP (Model Context Protocol) client for Cortex.
//!
//! Connects to **stdio** or **HTTP/Streamable HTTP** (with legacy SSE fallback)
//! MCP servers, lists tools, and exposes them as [`cortex_tools::Tool`]
//! implementations.

#![deny(missing_docs)]

mod adapter;
mod client;
mod config;
mod error;
mod protocol;
mod transport;

pub use adapter::{register_mcp_server_tools, McpToolAdapter};
pub use client::McpClient;
pub use config::{McpConfig, McpServerConfig};
pub use error::{McpError, Result};
pub use protocol::{McpToolDescriptor, ToolsCallResult};
pub use transport::{HttpTransport, McpTransport, StdioTransport};

use cortex_tools::ToolRegistry;
use tracing::{info, warn};

/// Load MCP config, connect enabled servers, and register tools.
///
/// Failures to connect individual servers are logged and skipped so the agent
/// can still start.
pub async fn load_and_register_mcp(config: &McpConfig, registry: &mut ToolRegistry) -> usize {
    let mut total = 0;
    for server in config.enabled_servers() {
        match McpClient::connect(server).await {
            Ok(client) => {
                let client = std::sync::Arc::new(client);
                let prefix = server.resolved_prefix();
                match register_mcp_server_tools(client, &prefix, registry).await {
                    Ok(n) => {
                        info!(server = %server.name, tools = n, "registered MCP tools");
                        total += n;
                    }
                    Err(err) => {
                        warn!(server = %server.name, error = %err, "failed to list MCP tools");
                    }
                }
            }
            Err(err) => {
                warn!(server = %server.name, error = %err, "failed to connect MCP server");
            }
        }
    }
    total
}
