//! Memory / storage errors.

use thiserror::Error;

/// Errors from durable storage operations.
#[derive(Debug, Error)]
pub enum MemoryError {
    /// Database / SQL failure.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Serialization failure.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Entity not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Invalid input.
    #[error("invalid: {0}")]
    Invalid(String),

    /// I/O error (paths, directories).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, MemoryError>;
