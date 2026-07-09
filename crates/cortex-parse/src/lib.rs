//! Tree-sitter based code outlines for Cortex.
//!
//! Supported languages: Rust, Python. Full LSP is intentionally deferred.

#![deny(missing_docs)]

mod error;
mod language;
mod outline;

pub use error::{ParseError, Result};
pub use language::SourceLanguage;
pub use outline::{format_outline, outline_file, outline_source, Outline, Symbol};
