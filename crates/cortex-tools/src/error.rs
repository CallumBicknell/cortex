//! Tool subsystem errors.

use thiserror::Error;

/// Errors from tool registration, permissions, or execution infrastructure.
#[derive(Debug, Error)]
pub enum ToolError {
    /// Tool name is unknown.
    #[error("tool not found: {0}")]
    NotFound(String),

    /// Tool is already registered.
    #[error("tool already registered: {0}")]
    AlreadyRegistered(String),

    /// Invalid input / arguments.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// User or policy rejected approval.
    #[error("approval denied: {0}")]
    ApprovalDenied(String),

    /// Operation cancelled.
    #[error("cancelled: {0}")]
    Cancelled(String),

    /// Timed out.
    #[error("timeout: {0}")]
    Timeout(String),

    /// Execution failed (tool-level).
    #[error("execution failed: {0}")]
    Execution(String),

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl ToolError {
    /// Convert to a user-visible tool result error string.
    pub fn to_output(&self) -> String {
        self.to_string()
    }
}

/// Result alias for tool infrastructure.
pub type Result<T> = std::result::Result<T, ToolError>;
