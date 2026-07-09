//! High-level MCP client session.

use crate::config::McpServerConfig;
use crate::error::{McpError, Result};
use crate::protocol::{McpToolDescriptor, ToolsCallResult, ToolsListResult};
use crate::transport::StdioTransport;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

/// An initialized MCP client session.
pub struct McpClient {
    name: String,
    transport: Arc<StdioTransport>,
}

impl McpClient {
    /// Connect to a configured stdio server and complete initialize handshake.
    pub async fn connect(config: &McpServerConfig) -> Result<Self> {
        if config.transport != "stdio" {
            return Err(McpError::Config(format!(
                "transport `{}` not supported yet (use stdio)",
                config.transport
            )));
        }
        let command = config
            .command
            .as_deref()
            .ok_or_else(|| McpError::Config(format!("server `{}` missing command", config.name)))?;
        let transport =
            StdioTransport::spawn(command, &config.args, config.cwd.as_deref(), &config.env)
                .await?;
        let transport = Arc::new(transport);

        let client = Self {
            name: config.name.clone(),
            transport,
        };
        client.initialize().await?;
        info!(server = %config.name, "MCP server connected");
        Ok(client)
    }

    /// Connect using an already-spawned transport (tests).
    pub async fn from_transport(
        name: impl Into<String>,
        transport: StdioTransport,
    ) -> Result<Self> {
        let client = Self {
            name: name.into(),
            transport: Arc::new(transport),
        };
        client.initialize().await?;
        Ok(client)
    }

    async fn initialize(&self) -> Result<()> {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "cortex",
                "version": env!("CARGO_PKG_VERSION"),
            }
        });
        let _ = self.transport.request("initialize", Some(params)).await?;
        self.transport
            .notify("notifications/initialized", Some(json!({})))
            .await?;
        Ok(())
    }

    /// Server logical name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Shared transport handle.
    pub fn transport(&self) -> Arc<StdioTransport> {
        Arc::clone(&self.transport)
    }

    /// List tools from the server.
    pub async fn list_tools(&self) -> Result<Vec<McpToolDescriptor>> {
        let result = self
            .transport
            .request("tools/list", Some(json!({})))
            .await?;
        let parsed: ToolsListResult = serde_json::from_value(result)
            .map_err(|e| McpError::Protocol(format!("tools/list parse: {e}")))?;
        Ok(parsed.tools)
    }

    /// Call a tool by server-side name.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolsCallResult> {
        let params = json!({
            "name": name,
            "arguments": arguments,
        });
        let result = self.transport.request("tools/call", Some(params)).await?;
        serde_json::from_value(result)
            .map_err(|e| McpError::Protocol(format!("tools/call parse: {e}")))
    }
}
