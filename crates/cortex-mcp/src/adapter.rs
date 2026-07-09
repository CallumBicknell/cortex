//! Map MCP tools onto the Cortex [`Tool`] trait.

use crate::client::McpClient;
use crate::error::Result;
use crate::protocol::McpToolDescriptor;
use async_trait::async_trait;
use cortex_tools::{Tool, ToolContext, ToolError, ToolRegistry};
use serde_json::{json, Value};
use std::sync::Arc;

/// Cortex tool that proxies to an MCP `tools/call`.
pub struct McpToolAdapter {
    /// Name as registered in Cortex (may include prefix).
    pub cortex_name: String,
    /// Name on the MCP server.
    pub remote_name: String,
    description: String,
    parameters: Value,
    client: Arc<McpClient>,
}

impl McpToolAdapter {
    /// Build from a server descriptor + prefix.
    pub fn new(client: Arc<McpClient>, prefix: &str, desc: McpToolDescriptor) -> Self {
        let cortex_name = format!("{prefix}{}", desc.name);
        let description = desc
            .description
            .unwrap_or_else(|| format!("MCP tool `{}` from {}", desc.name, client.name()));
        let parameters = desc.input_schema.unwrap_or_else(|| {
            json!({
                "type": "object",
                "properties": {}
            })
        });
        Self {
            cortex_name,
            remote_name: desc.name,
            description,
            parameters,
            client,
        }
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.cortex_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        self.parameters.clone()
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> cortex_tools::Result<String> {
        ctx.check_cancelled()?;
        let result = self
            .client
            .call_tool(&self.remote_name, input)
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        let text = result.as_text();
        if result.is_error.unwrap_or(false) {
            Err(ToolError::Execution(text))
        } else {
            Ok(ctx.truncate_output(text))
        }
    }
}

/// Connect to an MCP server and register all of its tools into `registry`.
pub async fn register_mcp_server_tools(
    client: Arc<McpClient>,
    prefix: &str,
    registry: &mut ToolRegistry,
) -> Result<usize> {
    let tools = client.list_tools().await?;
    let mut count = 0;
    for desc in tools {
        let adapter = McpToolAdapter::new(Arc::clone(&client), prefix, desc);
        registry.register_or_replace(Arc::new(adapter));
        count += 1;
    }
    Ok(count)
}
