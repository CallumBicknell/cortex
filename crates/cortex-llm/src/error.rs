//! Provider error types.

use thiserror::Error;

/// Errors from LLM providers and the registry.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// Request was cancelled.
    #[error("cancelled: {0}")]
    Cancelled(String),

    /// Invalid request parameters.
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// Authentication / API key failure.
    #[error("authentication failed: {0}")]
    Auth(String),

    /// Rate limited by the provider.
    #[error("rate limited: {0}")]
    RateLimited(String),

    /// Provider returned an error response.
    #[error("provider error ({status}): {message}")]
    Api {
        /// HTTP status if known.
        status: u16,
        /// Error message body.
        message: String,
    },

    /// Network / transport failure.
    #[error("transport error: {0}")]
    Transport(String),

    /// Response could not be parsed.
    #[error("parse error: {0}")]
    Parse(String),

    /// Feature not supported by this provider.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// Timed out waiting for the provider.
    #[error("timeout after {0:?}")]
    Timeout(std::time::Duration),

    /// Named provider/model not found in the registry.
    #[error("not found: {0}")]
    NotFound(String),

    /// Configuration error.
    #[error("config error: {0}")]
    Config(String),
}

impl ProviderError {
    /// Whether this error is likely transient and worth retrying.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited(_) | Self::Timeout(_) | Self::Transport(_) => true,
            Self::Api { status, .. } => {
                matches!(status, 408 | 409 | 425 | 429 | 500 | 502 | 503 | 504)
            }
            _ => false,
        }
    }

    /// Construct a cancelled error.
    pub fn cancelled(msg: impl Into<String>) -> Self {
        Self::Cancelled(msg.into())
    }
}

/// Result alias for provider operations.
pub type Result<T> = std::result::Result<T, ProviderError>;
