//! MCP client errors.

use thiserror::Error;

/// Errors from MCP transport or protocol.
#[derive(Debug, Error)]
pub enum McpError {
    /// I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Protocol / JSON-RPC failure.
    #[error("protocol error: {0}")]
    Protocol(String),
    /// Server returned an error object.
    #[error("mcp server error: {0}")]
    Server(String),
    /// Configuration problem.
    #[error("config error: {0}")]
    Config(String),
    /// Tool not found on server.
    #[error("tool not found: {0}")]
    NotFound(String),
    /// Timeout.
    #[error("timeout: {0}")]
    Timeout(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, McpError>;
