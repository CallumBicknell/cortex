//! Build a shared Solidity source snapshot for multi-lens audits.

use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Default max bytes for the combined source bundle.
pub const DEFAULT_MAX_BUNDLE_BYTES: usize = 400_000;

/// Result of collecting in-scope Solidity sources.
#[derive(Debug, Clone)]
pub struct SourceBundle {
    /// Absolute paths included.
    pub files: Vec<PathBuf>,
    /// Markdown body with path headers and fenced code.
    pub markdown: String,
    /// True if content was truncated to stay under the size budget.
    pub truncated: bool,
}

/// Whether a path should be excluded from audit scope.
pub fn is_excluded_sol_path(path: &Path) -> bool {
    let s = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let parts: Vec<&str> = s.split('/').collect();
    for p in &parts {
        if matches!(
            *p,
            "lib"
                | "node_modules"
                | "out"
                | "cache"
                | "artifacts"
                | "broadcast"
                | "dependencies"
                | "vendor"
                | "mocks"
                | "mock"
                | "test"
                | "tests"
                | "script"
                | "scripts"
        ) {
            return true;
        }
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".t.sol") || name.ends_with(".s.sol") {
        return true;
    }
    if name.contains("mock") || name.contains("test") {
        return true;
    }
    false
}

/// Collect `.sol` files under `root` / optional `scope` relative path.
pub fn collect_sol_files(workspace: &Path, scope: &str) -> std::io::Result<Vec<PathBuf>> {
    let base = if scope.is_empty() || scope == "." {
        workspace.to_path_buf()
    } else {
        let p = workspace.join(scope);
        if p.is_absolute() {
            p
        } else {
            workspace.join(scope)
        }
    };
    if !base.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(&base).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("sol") {
            continue;
        }
        if is_excluded_sol_path(path) {
            continue;
        }
        files.push(path.to_path_buf());
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Build markdown source bundle, optionally writing under `.cortex/tmp/`.
pub fn build_source_bundle(
    workspace: &Path,
    scope: &str,
    max_bytes: usize,
) -> std::io::Result<SourceBundle> {
    let files = collect_sol_files(workspace, scope)?;
    let mut markdown = String::from("# Audit source bundle\n\n");
    markdown.push_str(&format!(
        "Workspace: `{}`\nScope: `{}`\nFiles: {}\n\n",
        workspace.display(),
        scope,
        files.len()
    ));
    let mut truncated = false;
    let mut included = Vec::new();

    for path in &files {
        let rel = path
            .strip_prefix(workspace)
            .unwrap_or(path)
            .display()
            .to_string();
        let body = match fs::read_to_string(path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let chunk = format!("### {rel}\n\n```solidity\n{body}\n```\n\n");
        if markdown.len() + chunk.len() > max_bytes {
            truncated = true;
            markdown.push_str(&format!(
                "\n… truncated; remaining files omitted (budget {max_bytes} bytes).\n"
            ));
            break;
        }
        markdown.push_str(&chunk);
        included.push(path.clone());
    }

    if included.is_empty() {
        markdown.push_str(
            "_No in-scope `.sol` files found (after excluding lib/test/mocks)._ \
             Lenses should use workspace tools to locate sources.\n",
        );
    }

    Ok(SourceBundle {
        files: included,
        markdown,
        truncated,
    })
}

/// Write bundle to `.cortex/tmp/audit-<id>/source.md` and return the path.
pub fn write_source_bundle(
    workspace: &Path,
    scope: &str,
    max_bytes: usize,
) -> std::io::Result<(PathBuf, SourceBundle)> {
    let bundle = build_source_bundle(workspace, scope, max_bytes)?;
    let dir = workspace
        .join(".cortex")
        .join("tmp")
        .join(format!("audit-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir)?;
    let path = dir.join("source.md");
    fs::write(&path, &bundle.markdown)?;
    Ok((path, bundle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn excludes_lib_and_tests() {
        assert!(is_excluded_sol_path(Path::new(
            "lib/forge-std/src/Test.sol"
        )));
        assert!(is_excluded_sol_path(Path::new("test/Vault.t.sol")));
        assert!(is_excluded_sol_path(Path::new("src/MockToken.sol")));
        assert!(!is_excluded_sol_path(Path::new("src/Vault.sol")));
        assert!(!is_excluded_sol_path(Path::new("contracts/Token.sol")));
    }

    #[test]
    fn builds_bundle_from_fixture() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("lib/x")).unwrap();
        fs::write(
            dir.path().join("src/Vault.sol"),
            "contract Vault { function withdraw() external {} }\n",
        )
        .unwrap();
        fs::write(dir.path().join("lib/x/Ignore.sol"), "contract Ignore {}\n").unwrap();
        let b = build_source_bundle(dir.path(), ".", 50_000).unwrap();
        assert_eq!(b.files.len(), 1);
        assert!(b.markdown.contains("Vault.sol"));
        assert!(!b.markdown.contains("Ignore.sol"));
    }
}
