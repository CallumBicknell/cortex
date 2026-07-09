//! Workspace errors.

use thiserror::Error;

/// Errors from workspace inspection.
#[derive(Debug, Error)]
pub enum WorkspaceError {
    /// I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid path or root.
    #[error("invalid workspace: {0}")]
    Invalid(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, WorkspaceError>;
