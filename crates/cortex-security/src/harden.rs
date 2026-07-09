//! Extra hardening helpers (path checks, env scrubbing, bubblewrap detection).

use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// Return true if `path` has no `..` components after normalization attempt.
pub fn path_has_parent_escape(path: &Path) -> bool {
    path.components().any(|c| matches!(c, Component::ParentDir))
}

/// Reject absolute paths when sandboxing to workspace-relative only.
pub fn reject_absolute_path(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if p.is_absolute() {
        return Err(format!("absolute paths not allowed in sandbox: {path}"));
    }
    if path_has_parent_escape(p) {
        return Err(format!("path escapes workspace via ..: {path}"));
    }
    Ok(())
}

/// Whether `bwrap` (bubblewrap) is available on PATH.
pub fn bubblewrap_available() -> bool {
    Command::new("bwrap")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Build a bubblewrap argv prefix that binds `workspace` read-write and drops net.
///
/// Returns `None` if bubblewrap is not installed. Caller still runs the command
/// without isolation in that case (document as soft hardening).
pub fn bubblewrap_shell_prefix(workspace: &Path) -> Option<Vec<String>> {
    if !bubblewrap_available() {
        return None;
    }
    let ws = workspace.to_string_lossy().to_string();
    Some(vec![
        "bwrap".into(),
        "--ro-bind".into(),
        "/usr".into(),
        "/usr".into(),
        "--ro-bind".into(),
        "/lib".into(),
        "/lib".into(),
        "--ro-bind-try".into(),
        "/lib64".into(),
        "/lib64".into(),
        "--ro-bind-try".into(),
        "/bin".into(),
        "/bin".into(),
        "--ro-bind-try".into(),
        "/sbin".into(),
        "/sbin".into(),
        "--dev".into(),
        "/dev".into(),
        "--proc".into(),
        "/proc".into(),
        "--tmpfs".into(),
        "/tmp".into(),
        "--bind".into(),
        ws.clone(),
        ws.clone(),
        "--chdir".into(),
        ws,
        "--unshare-net".into(),
        "--die-with-parent".into(),
        "--".into(),
    ])
}

/// Normalize a relative path under workspace without requiring existence.
pub fn safe_join(workspace: &Path, rel: &str) -> Result<PathBuf, String> {
    reject_absolute_path(rel)?;
    let joined = workspace.join(rel);
    let workspace_c = workspace
        .canonicalize()
        .map_err(|e| format!("workspace: {e}"))?;
    // Canonicalize parent chain if file missing.
    let mut cur = workspace_c.clone();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(s) => cur.push(s),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!("path escapes workspace: {rel}"));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("invalid path component in {rel}"));
            }
        }
    }
    if !cur.starts_with(&workspace_c) {
        return Err(format!("resolved path outside workspace: {rel}"));
    }
    let _ = joined;
    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn rejects_parent_escape() {
        assert!(reject_absolute_path("../etc/passwd").is_err());
        assert!(reject_absolute_path("/etc/passwd").is_err());
        assert!(reject_absolute_path("src/main.rs").is_ok());
    }

    #[test]
    fn safe_join_ok() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        let p = safe_join(dir.path(), "src/lib.rs").unwrap();
        assert!(p.ends_with("src/lib.rs"));
    }
}
