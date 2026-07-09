//! Skill store errors.

use thiserror::Error;

/// Skill errors.
#[derive(Debug, Error)]
pub enum SkillError {
    /// IO failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Parse failure.
    #[error("parse: {0}")]
    Parse(String),
    /// Invalid input.
    #[error("invalid: {0}")]
    Invalid(String),
    /// Missing skill.
    #[error("skill not found: {0}")]
    NotFound(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, SkillError>;
