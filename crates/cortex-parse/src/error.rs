//! Parse errors.

use thiserror::Error;

/// Errors from parsing / outlining.
#[derive(Debug, Error)]
pub enum ParseError {
    /// Language not supported for the given path/extension.
    #[error("unsupported language for `{0}`")]
    Unsupported(String),
    /// Tree-sitter failed to parse.
    #[error("parse failed: {0}")]
    Parse(String),
    /// IO failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Query compile/run error.
    #[error("query: {0}")]
    Query(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, ParseError>;
