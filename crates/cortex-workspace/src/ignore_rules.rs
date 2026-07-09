//! Ignore rules via gitignore-style matching (`.gitignore` + `.cortexignore`).

use crate::error::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// Default directory names always skipped in maps (even if not gitignored).
const ALWAYS_SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".cortex",
    "dist",
    "build",
    "__pycache__",
    ".venv",
    "venv",
];

/// Walk files under `root`, honoring `.gitignore` and `.cortexignore`.
///
/// Returns paths relative to `root`.
pub fn list_files(root: &Path, max_files: usize) -> Result<Vec<PathBuf>> {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .require_git(false)
        .max_depth(Some(12));

    // Add .cortexignore if present.
    let cortexignore = root.join(".cortexignore");
    if cortexignore.is_file() {
        let _ = builder.add_ignore(&cortexignore);
    }

    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        if should_skip(root, path) {
            continue;
        }
        if let Ok(rel) = path.strip_prefix(root) {
            files.push(rel.to_path_buf());
            if files.len() >= max_files {
                break;
            }
        }
    }
    files.sort();
    Ok(files)
}

fn should_skip(root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return true;
    };
    for comp in rel.components() {
        if let std::path::Component::Normal(name) = comp {
            let s = name.to_string_lossy();
            if ALWAYS_SKIP_DIRS.iter().any(|d| *d == s) {
                return true;
            }
        }
    }
    // Skip very large / binary-ish extensions in maps.
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some(
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "pdf"
                | "zip"
                | "gz"
                | "so"
                | "dylib"
                | "o"
                | "a"
                | "wasm"
                | "lock"
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn respects_gitignore() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "secret.txt\n").unwrap();
        fs::write(dir.path().join("visible.txt"), "ok").unwrap();
        fs::write(dir.path().join("secret.txt"), "nope").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target/x.rs"), "x").unwrap();

        let files = list_files(dir.path(), 100).unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "visible.txt"));
        assert!(!names.iter().any(|n| n.contains("secret")));
        assert!(!names.iter().any(|n| n.contains("target")));
    }
}
