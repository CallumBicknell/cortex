//! Plugin errors.

use thiserror::Error;

/// Errors from plugin loading and lifecycle.
#[derive(Debug, Error)]
pub enum PluginError {
    /// Configuration could not be parsed or validated.
    #[error("plugin config: {0}")]
    Config(String),
    /// Unknown plugin id (not in builtin factory and not discoverable).
    #[error("unknown plugin id: {0}")]
    Unknown(String),
    /// Plugin failed during init/start/stop.
    #[error("plugin `{id}`: {message}")]
    Lifecycle {
        /// Plugin id.
        id: String,
        /// Error detail.
        message: String,
    },
    /// IO failure (config files, etc.).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, PluginError>;
