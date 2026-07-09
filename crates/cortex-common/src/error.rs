//! Shared error types for Cortex crates.

use thiserror::Error;

/// Primary error type shared across Cortex crates.
///
/// Domain crates may define their own error enums and convert into this type
/// at boundaries (CLI, API, runtime orchestration).
#[derive(Debug, Error)]
pub enum CortexError {
    /// Configuration loading or validation failed.
    #[error("config error: {0}")]
    Config(String),

    /// An operation was cancelled.
    #[error("cancelled: {0}")]
    Cancelled(String),

    /// Invalid user or API input.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// A requested resource was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Permission or policy denied the operation.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// External provider (LLM, HTTP, …) failed.
    #[error("provider error: {0}")]
    Provider(String),

    /// Tool execution failed.
    #[error("tool error: {0}")]
    Tool(String),

    /// Persistence / storage failure.
    #[error("storage error: {0}")]
    Storage(String),

    /// Serialization or deserialization failure.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Internal invariant violated or unexpected state.
    #[error("internal error: {0}")]
    Internal(String),

    /// Other I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenient result alias using [`CortexError`].
pub type Result<T> = std::result::Result<T, CortexError>;

impl CortexError {
    /// Construct a config error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Construct a cancelled error.
    pub fn cancelled(msg: impl Into<String>) -> Self {
        Self::Cancelled(msg.into())
    }

    /// Construct an invalid-input error.
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Construct a not-found error.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Construct an internal error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Returns true if this error represents cancellation.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_cancelled() {
        let err = CortexError::cancelled("user abort");
        assert!(err.is_cancelled());
        assert!(err.to_string().contains("user abort"));
    }
}
