//! Tree-sitter based code outlines for Cortex.
//!
//! Supported languages: Rust, Python, Solidity. Full LSP is intentionally deferred.

#![deny(missing_docs)]

mod error;
mod index;
mod language;
mod outline;

pub use error::{ParseError, Result};
pub use index::{
    find_definitions, format_definitions, format_symbol_hits, index_workspace, outline_path,
    search_symbols, IndexedSymbol, SymbolHit,
};
pub use language::SourceLanguage;
pub use outline::{format_outline, outline_file, outline_source, Outline, Symbol};
