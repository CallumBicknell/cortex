//! High-level MCP client session.

use crate::config::McpServerConfig;
use crate::error::{McpError, Result};
use crate::protocol::{McpToolDescriptor, ToolsCallResult, ToolsListResult};
use crate::transport::{HttpTransport, McpTransport, StdioTransport};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

/// An initialized MCP client session.
pub struct McpClient {
    name: String,
    transport: Arc<dyn McpTransport>,
    /// Protocol version used for initialize.
    protocol_version: String,
}

impl McpClient {
    /// Connect to a configured server (stdio or HTTP/SSE) and complete initialize.
    pub async fn connect(config: &McpServerConfig) -> Result<Self> {
        let transport_kind = config.transport.to_ascii_lowercase();
        match transport_kind.as_str() {
            "stdio" | "" => Self::connect_stdio(config).await,
            "http" | "sse" | "streamable_http" | "streamable-http" => {
                Self::connect_http(config).await
            }
            other => Err(McpError::Config(format!(
                "unknown transport `{other}` (use stdio, http, or sse)"
            ))),
        }
    }

    async fn connect_stdio(config: &McpServerConfig) -> Result<Self> {
        let command = config
            .command
            .as_deref()
            .ok_or_else(|| McpError::Config(format!("server `{}` missing command", config.name)))?;
        let transport =
            StdioTransport::spawn(command, &config.args, config.cwd.as_deref(), &config.env)
                .await?;
        let client = Self {
            name: config.name.clone(),
            transport: Arc::new(transport),
            protocol_version: "2024-11-05".into(),
        };
        client.initialize().await?;
        info!(server = %config.name, transport = "stdio", "MCP server connected");
        Ok(client)
    }

    async fn connect_http(config: &McpServerConfig) -> Result<Self> {
        let url = config
            .url
            .as_deref()
            .ok_or_else(|| McpError::Config(format!("server `{}` missing url", config.name)))?;
        validate_http_url(url)?;

        let headers = config.resolved_headers();
        let timeout = config.timeout_secs.unwrap_or(60);
        let http = HttpTransport::new(url, &headers, timeout)?;
        let client = Self {
            name: config.name.clone(),
            transport: Arc::new(http),
            protocol_version: "2025-03-26".into(),
        };

        match client.initialize().await {
            Ok(()) => {
                info!(server = %config.name, transport = "http", %url, "MCP server connected");
                return Ok(client);
            }
            Err(err) => {
                warn!(
                    server = %config.name,
                    error = %err,
                    "Streamable HTTP initialize failed; trying legacy SSE endpoint discovery"
                );
            }
        }

        // Rebuild HTTP transport and switch to legacy SSE message endpoint.
        let http = HttpTransport::new(url, &headers, timeout)?;
        http.switch_to_legacy_sse_endpoint().await?;
        let client = Self {
            name: config.name.clone(),
            transport: Arc::new(http),
            protocol_version: "2024-11-05".into(),
        };
        client.initialize().await?;
        info!(
            server = %config.name,
            transport = "sse-legacy",
            %url,
            "MCP server connected"
        );
        Ok(client)
    }

    /// Connect using an already-spawned stdio transport (tests).
    pub async fn from_transport(
        name: impl Into<String>,
        transport: StdioTransport,
    ) -> Result<Self> {
        let client = Self {
            name: name.into(),
            transport: Arc::new(transport),
            protocol_version: "2024-11-05".into(),
        };
        client.initialize().await?;
        Ok(client)
    }

    /// Connect using an HTTP transport (tests).
    pub async fn from_http_transport(
        name: impl Into<String>,
        transport: HttpTransport,
        protocol_version: impl Into<String>,
    ) -> Result<Self> {
        let client = Self {
            name: name.into(),
            transport: Arc::new(transport),
            protocol_version: protocol_version.into(),
        };
        client.initialize().await?;
        Ok(client)
    }

    async fn initialize(&self) -> Result<()> {
        let params = json!({
            "protocolVersion": self.protocol_version,
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

fn validate_http_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url).map_err(|e| McpError::Config(format!("invalid url: {e}")))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(McpError::Config(format!(
                "url scheme must be http or https, got `{other}`"
            )));
        }
    }
    if parsed.host_str().is_none() {
        return Err(McpError::Config("url missing host".into()));
    }
    // Basic SSRF hygiene: block obvious local targets unless explicitly allowed via env.
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    let allow_local = std::env::var("CORTEX_MCP_ALLOW_LOCAL")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !allow_local
        && (host == "localhost"
            || host == "127.0.0.1"
            || host == "0.0.0.0"
            || host == "::1"
            || host.ends_with(".local"))
    {
        return Err(McpError::Config(format!(
            "refusing local MCP host `{host}` (set CORTEX_MCP_ALLOW_LOCAL=1 to override)"
        )));
    }
    Ok(())
}
