//! Security crate errors.

use thiserror::Error;

/// Security policy / redaction errors.
#[derive(Debug, Error)]
pub enum SecurityError {
    /// I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// TOML parse failure.
    #[error("parse error: {0}")]
    Parse(String),
    /// Invalid policy.
    #[error("invalid policy: {0}")]
    Invalid(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, SecurityError>;
