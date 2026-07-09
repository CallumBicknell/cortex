//! Language detection and tree-sitter language handles.

use crate::error::{ParseError, Result};
use std::path::Path;
use tree_sitter::Language;

/// Supported outline languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLanguage {
    /// Rust.
    Rust,
    /// Python.
    Python,
}

impl SourceLanguage {
    /// Detect from file extension.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "rs" => Ok(Self::Rust),
            "py" | "pyi" => Ok(Self::Python),
            _ => Err(ParseError::Unsupported(path.display().to_string())),
        }
    }

    /// Human name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
        }
    }

    /// Tree-sitter language.
    pub fn language(self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
        }
    }
}
