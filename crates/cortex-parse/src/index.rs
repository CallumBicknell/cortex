//! Workspace-wide symbol index (tree-sitter outlines).

use crate::error::Result;
use crate::outline::{outline_file, Outline, Symbol};
use serde::{Deserialize, Serialize};
use std::path::Path;
use walkdir::WalkDir;

/// Indexed symbol with file path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedSymbol {
    /// Relative or absolute path.
    pub path: String,
    /// Language.
    pub language: String,
    /// Symbol.
    #[serde(flatten)]
    pub symbol: Symbol,
}

/// Search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolHit {
    /// Indexed symbol.
    #[serde(flatten)]
    pub indexed: IndexedSymbol,
    /// Simple score (name exact > prefix > contains).
    pub score: i32,
}

/// Build a workspace symbol index for Rust/Python sources.
pub fn index_workspace(
    root: impl AsRef<Path>,
    max_files: usize,
) -> Result<(Vec<IndexedSymbol>, Vec<String>)> {
    let root = root.as_ref();
    let mut symbols = Vec::new();
    let mut errors = Vec::new();
    let mut count = 0usize;

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            // Always enter the root (may be a .tmp* path in tests).
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !(name == ".git"
                || name == ".cortex"
                || name == "target"
                || name == "node_modules"
                || name == "venv"
                || name == "__pycache__"
                || name == ".venv")
        })
        .flatten()
    {
        if count >= max_files {
            break;
        }
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "rs" && ext != "py" && ext != "pyi" {
            continue;
        }
        // Skip huge files.
        if entry.metadata().map(|m| m.len()).unwrap_or(0) > 512 * 1024 {
            continue;
        }
        count += 1;
        match outline_file(path) {
            Ok(outline) => {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                for sym in outline.symbols {
                    symbols.push(IndexedSymbol {
                        path: rel.clone(),
                        language: outline.language.clone(),
                        symbol: sym,
                    });
                }
            }
            Err(e) => {
                errors.push(format!("{}: {e}", path.display()));
            }
        }
    }
    Ok((symbols, errors))
}

/// Search indexed symbols by name query.
pub fn search_symbols(symbols: &[IndexedSymbol], query: &str, limit: usize) -> Vec<SymbolHit> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<SymbolHit> = symbols
        .iter()
        .filter_map(|s| {
            let name = s.symbol.name.to_ascii_lowercase();
            let score = if name == q {
                100
            } else if name.starts_with(&q) {
                80
            } else if name.contains(&q) {
                50
            } else if s.path.to_ascii_lowercase().contains(&q) {
                20
            } else {
                return None;
            };
            Some(SymbolHit {
                indexed: s.clone(),
                score,
            })
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.indexed.path.cmp(&b.indexed.path))
    });
    hits.truncate(limit.max(1));
    hits
}

/// Find likely definitions for a name (exact match preferred).
pub fn find_definitions(symbols: &[IndexedSymbol], name: &str, limit: usize) -> Vec<IndexedSymbol> {
    let hits = search_symbols(symbols, name, limit.saturating_mul(3));
    hits.into_iter()
        .filter(|h| h.indexed.symbol.name.eq_ignore_ascii_case(name) || h.score >= 80)
        .take(limit.max(1))
        .map(|h| h.indexed)
        .collect()
}

/// Format hits for agent output.
pub fn format_symbol_hits(hits: &[SymbolHit]) -> String {
    if hits.is_empty() {
        return "no symbols matched".into();
    }
    let mut out = String::new();
    for h in hits {
        out.push_str(&format!(
            "{}:{}  {} {}{}\n",
            h.indexed.path,
            h.indexed.symbol.start_line,
            h.indexed.symbol.kind,
            h.indexed.symbol.name,
            h.indexed
                .symbol
                .parent
                .as_ref()
                .map(|p| format!(" in {p}"))
                .unwrap_or_default()
        ));
    }
    out
}

/// Format definitions.
pub fn format_definitions(defs: &[IndexedSymbol]) -> String {
    if defs.is_empty() {
        return "no definitions found".into();
    }
    let mut out = String::new();
    for d in defs {
        out.push_str(&format!(
            "{}:{}-{}  {} {}\n",
            d.path, d.symbol.start_line, d.symbol.end_line, d.symbol.kind, d.symbol.name
        ));
    }
    out
}

/// Outline a single path for API re-export.
pub fn outline_path(path: impl AsRef<Path>) -> Result<Outline> {
    outline_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn indexes_and_finds() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("lib.rs"),
            "pub fn unique_foo_bar() {}\npub struct UniqueFoo;\n",
        )
        .unwrap();
        let (syms, errs) = index_workspace(dir.path(), 50).unwrap();
        assert!(
            !syms.is_empty(),
            "expected symbols, errs={errs:?} files in {:?}",
            std::fs::read_dir(dir.path())
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect::<Vec<_>>()
        );
        let hits = search_symbols(&syms, "unique_foo", 10);
        assert!(!hits.is_empty());
        let defs = find_definitions(&syms, "unique_foo_bar", 5);
        assert_eq!(defs[0].symbol.name, "unique_foo_bar");
    }
}
