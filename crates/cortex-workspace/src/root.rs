//! Workspace root detection.

use crate::error::{Result, WorkspaceError};
use std::path::{Path, PathBuf};

/// Resolve a workspace root from a starting path.
///
/// Walks upward looking for a `.git` directory. If none is found, returns the
/// canonicalized starting directory.
pub fn detect_root(start: impl AsRef<Path>) -> Result<PathBuf> {
    let start = start.as_ref().canonicalize().map_err(|e| {
        WorkspaceError::Invalid(format!("cannot resolve {}: {e}", start.as_ref().display()))
    })?;

    let mut current = start.as_path();
    loop {
        if current.join(".git").exists() {
            return Ok(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => return Ok(start),
        }
    }
}

/// True if `path` is inside `root` (after normalization best-effort).
pub fn is_under(root: &Path, path: &Path) -> bool {
    path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detects_git_root() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        let nested = dir.path().join("a/b");
        fs::create_dir_all(&nested).unwrap();
        let root = detect_root(&nested).unwrap();
        assert_eq!(root, dir.path().canonicalize().unwrap());
    }

    #[test]
    fn falls_back_to_start() {
        let dir = tempdir().unwrap();
        let root = detect_root(dir.path()).unwrap();
        assert_eq!(root, dir.path().canonicalize().unwrap());
    }
}
