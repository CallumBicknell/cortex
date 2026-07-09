//! MCP server configuration (TOML).

use crate::error::{McpError, Result};
use serde::Deserialize;
use std::path::Path;

/// Top-level MCP config file.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct McpConfig {
    /// Configured servers.
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

/// One MCP server process (stdio by default).
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    /// Logical name (used in tool prefixes / logs).
    pub name: String,
    /// Whether to connect at startup.
    #[serde(default)]
    pub enabled: bool,
    /// Transport: `stdio` (default) or `sse` (reserved).
    #[serde(default = "default_transport")]
    pub transport: String,
    /// Command to spawn for stdio transport.
    #[serde(default)]
    pub command: Option<String>,
    /// Arguments for the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory for the server process.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Extra environment variables.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Optional URL for SSE transport.
    #[serde(default)]
    pub url: Option<String>,
    /// Prefix for tool names in the Cortex registry (`mcp_<name>_` if empty uses name).
    #[serde(default)]
    pub tool_prefix: Option<String>,
}

fn default_transport() -> String {
    "stdio".into()
}

impl McpConfig {
    /// Load from a TOML file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path.as_ref()).map_err(McpError::Io)?;
        Self::from_toml(&text)
    }

    /// Parse TOML.
    pub fn from_toml(text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|e| McpError::Config(e.to_string()))
    }

    /// Enabled servers only.
    pub fn enabled_servers(&self) -> impl Iterator<Item = &McpServerConfig> {
        self.servers.iter().filter(|s| s.enabled)
    }
}

impl McpServerConfig {
    /// Tool name prefix including trailing underscore when non-empty.
    pub fn resolved_prefix(&self) -> String {
        let raw = self
            .tool_prefix
            .clone()
            .unwrap_or_else(|| format!("mcp_{}", self.name));
        if raw.is_empty() {
            String::new()
        } else if raw.ends_with('_') {
            raw
        } else {
            format!("{raw}_")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_servers() {
        let cfg = McpConfig::from_toml(
            r#"
            [[servers]]
            name = "fs"
            enabled = true
            command = "npx"
            args = ["-y", "pkg"]
            "#,
        )
        .unwrap();
        assert_eq!(cfg.servers.len(), 1);
        assert!(cfg.servers[0].enabled);
        assert_eq!(cfg.servers[0].resolved_prefix(), "mcp_fs_");
    }
}
