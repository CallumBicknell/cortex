//! MCP server configuration (TOML).

use crate::error::{McpError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Top-level MCP config file.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct McpConfig {
    /// Configured servers.
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

/// One MCP server (stdio process or remote HTTP/SSE endpoint).
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    /// Logical name (used in tool prefixes / logs).
    pub name: String,
    /// Whether to connect at startup.
    #[serde(default)]
    pub enabled: bool,
    /// Transport: `stdio` (default), `http` / `streamable_http`, or `sse`.
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
    /// Extra environment variables (stdio child, and `${ENV}` expansion in headers).
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// URL for HTTP / Streamable HTTP / SSE transports.
    #[serde(default)]
    pub url: Option<String>,
    /// Extra HTTP headers (values may use `$ENV_VAR` for secret injection).
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Optional HTTP timeout in seconds (default 60).
    #[serde(default)]
    pub timeout_secs: Option<u64>,
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

    /// HTTP headers with `$VAR` / `${VAR}` expanded from the process environment.
    pub fn resolved_headers(&self) -> HashMap<String, String> {
        self.headers
            .iter()
            .map(|(k, v)| (k.clone(), expand_env(v)))
            .collect()
    }
}

fn expand_env(value: &str) -> String {
    // Support $VAR and ${VAR}.
    let mut out = value.to_string();
    // ${VAR}
    while let Some(start) = out.find("${") {
        let rest = &out[start + 2..];
        if let Some(end) = rest.find('}') {
            let key = &rest[..end];
            let repl = std::env::var(key).unwrap_or_default();
            out = format!("{}{}{}", &out[..start], repl, &rest[end + 1..]);
        } else {
            break;
        }
    }
    // $VAR (simple identifier)
    if out.contains('$') {
        let mut result = String::new();
        let chars: Vec<char> = out.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '$'
                && i + 1 < chars.len()
                && (chars[i + 1].is_ascii_alphabetic() || chars[i + 1] == '_')
            {
                i += 1;
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let key: String = chars[start..i].iter().collect();
                result.push_str(&std::env::var(&key).unwrap_or_default());
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }
        return result;
    }
    out
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

    #[test]
    fn parses_http_server() {
        let cfg = McpConfig::from_toml(
            r#"
            [[servers]]
            name = "blockscout"
            enabled = true
            transport = "http"
            url = "https://mcp.blockscout.com/mcp"
            tool_prefix = "mcp_blockscout"
            "#,
        )
        .unwrap();
        assert_eq!(cfg.servers[0].transport, "http");
        assert_eq!(
            cfg.servers[0].url.as_deref(),
            Some("https://mcp.blockscout.com/mcp")
        );
    }

    #[test]
    fn expand_env_var() {
        std::env::set_var("CORTEX_TEST_TOKEN", "secret123");
        assert_eq!(expand_env("Bearer $CORTEX_TEST_TOKEN"), "Bearer secret123");
        assert_eq!(
            expand_env("Bearer ${CORTEX_TEST_TOKEN}"),
            "Bearer secret123"
        );
        std::env::remove_var("CORTEX_TEST_TOKEN");
    }
}
