//! Runtime and agent-loop errors.

use thiserror::Error;

/// Errors from the runtime / agent loop.
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// Kernel lifecycle error.
    #[error("kernel error: {0}")]
    Kernel(String),

    /// LLM provider failure.
    #[error("provider error: {0}")]
    Provider(#[from] cortex_llm::ProviderError),

    /// Tool subsystem failure (infrastructure, not tool-level is_error).
    #[error("tool error: {0}")]
    Tool(String),

    /// Run was cancelled.
    #[error("cancelled: {0}")]
    Cancelled(String),

    /// Invalid configuration or input.
    #[error("invalid: {0}")]
    Invalid(String),

    /// Max turns exceeded without a final answer.
    #[error("max turns exceeded ({0})")]
    MaxTurns(u32),

    /// Internal error.
    #[error("internal: {0}")]
    Internal(String),
}

/// Result alias for runtime operations.
pub type Result<T> = std::result::Result<T, RuntimeError>;
