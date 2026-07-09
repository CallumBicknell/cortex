//! Permission policy and path sandboxing.

use crate::error::{Result, ToolError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// Decision for a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Always allow without prompting.
    Allow,
    /// Always deny.
    Deny,
    /// Require interactive / approver consent.
    #[default]
    Ask,
}

/// Permission policy for tools and path/network access.
#[derive(Debug, Clone)]
pub struct PermissionPolicy {
    /// Default mode when a tool is not listed.
    pub default_mode: PermissionMode,
    /// Per-tool overrides.
    pub tools: HashMap<String, PermissionMode>,
    /// If true, all filesystem paths must resolve under the workspace root.
    pub sandbox_workspace: bool,
    /// Maximum bytes of tool output retained (stdout truncation).
    pub max_output_bytes: usize,
    /// Default shell timeout in seconds.
    pub shell_timeout_secs: u64,
    /// Hosts allowed for HTTP (empty = allow any non-blocked host).
    pub http_allow_hosts: Vec<String>,
    /// Host suffixes always blocked (SSRF basics).
    pub http_block_hosts: Vec<String>,
    /// Env var names scrubbed from shell children.
    pub scrub_env: Vec<String>,
    /// Shell command substrings that are hard-denied.
    pub shell_deny_patterns: Vec<String>,
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        let mut tools = HashMap::new();
        // Safe read tools default allow.
        for name in [
            "read_file",
            "list_dir",
            "glob_files",
            "git_status",
            "git_diff",
            "git_log",
        ] {
            tools.insert(name.to_string(), PermissionMode::Allow);
        }
        // Mutating / risky tools ask by default.
        for name in [
            "write_file",
            "edit_file",
            "shell",
            "git_add",
            "git_commit",
            "http_request",
        ] {
            tools.insert(name.to_string(), PermissionMode::Ask);
        }

        Self {
            default_mode: PermissionMode::Ask,
            tools,
            sandbox_workspace: true,
            max_output_bytes: 256 * 1024,
            shell_timeout_secs: 60,
            http_allow_hosts: Vec::new(),
            http_block_hosts: vec![
                "localhost".into(),
                "127.0.0.1".into(),
                "0.0.0.0".into(),
                "::1".into(),
                "metadata.google.internal".into(),
                "169.254.169.254".into(),
            ],
            scrub_env: vec![
                "OPENAI_API_KEY".into(),
                "ANTHROPIC_API_KEY".into(),
                "OPENROUTER_API_KEY".into(),
                "AWS_SECRET_ACCESS_KEY".into(),
                "GITHUB_TOKEN".into(),
                "GH_TOKEN".into(),
                "PRIVATE_KEY".into(),
            ],
            shell_deny_patterns: vec![
                "rm -rf /".into(),
                "mkfs".into(),
                ":(){ :|:& };:".into(),
                "dd if=/dev/zero".into(),
            ],
        }
    }
}

impl PermissionPolicy {
    /// Mode for a tool name.
    pub fn mode_for(&self, tool_name: &str) -> PermissionMode {
        self.tools
            .get(tool_name)
            .copied()
            .unwrap_or(self.default_mode)
    }

    /// Force all tools to allow (useful for tests / `--yolo`).
    pub fn allow_all(mut self) -> Self {
        self.default_mode = PermissionMode::Allow;
        for mode in self.tools.values_mut() {
            *mode = PermissionMode::Allow;
        }
        self
    }

    /// Resolve `path` relative to `workspace` and ensure it stays inside the sandbox.
    pub fn resolve_path(&self, workspace: &Path, path: &str) -> Result<PathBuf> {
        let workspace = workspace
            .canonicalize()
            .map_err(|e| ToolError::Execution(format!("workspace root invalid: {e}")))?;

        let candidate = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            workspace.join(path)
        };

        // Normalize without requiring the path to exist yet.
        let normalized = normalize_path(&candidate);

        if self.sandbox_workspace {
            let workspace_str = workspace.to_string_lossy();
            let normalized_str = normalized.to_string_lossy();
            // For non-existent paths, canonicalize parent and re-join.
            let checked = if normalized.exists() {
                normalized
                    .canonicalize()
                    .map_err(|e| ToolError::Execution(format!("path resolve failed: {e}")))?
            } else {
                let parent = normalized.parent().unwrap_or(Path::new("."));
                let file = normalized.file_name().ok_or_else(|| {
                    ToolError::InvalidInput(format!("invalid path: {normalized_str}"))
                })?;
                if parent.as_os_str().is_empty() {
                    workspace.join(file)
                } else {
                    let parent_canon = if parent.exists() {
                        parent.canonicalize().map_err(|e| {
                            ToolError::Execution(format!("parent path resolve failed: {e}"))
                        })?
                    } else {
                        // Walk up to an existing ancestor under workspace.
                        resolve_under_workspace(&workspace, parent)?
                    };
                    parent_canon.join(file)
                }
            };

            if !checked.starts_with(&workspace) {
                return Err(ToolError::PermissionDenied(format!(
                    "path `{}` escapes workspace `{}`",
                    checked.display(),
                    workspace_str
                )));
            }
            return Ok(checked);
        }

        Ok(normalized)
    }

    /// Hard-deny shell command if it matches a dangerous pattern.
    pub fn shell_command_allowed(&self, command: &str) -> bool {
        let lower = command.to_ascii_lowercase();
        !self
            .shell_deny_patterns
            .iter()
            .any(|p| lower.contains(&p.to_ascii_lowercase()))
    }

    /// Whether an HTTP host is allowed.
    pub fn http_host_allowed(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        for blocked in &self.http_block_hosts {
            if host == blocked.to_ascii_lowercase() || host.ends_with(&format!(".{blocked}")) {
                return false;
            }
        }
        // Block obvious private IP literals.
        if is_blocked_ip_literal(&host) {
            return false;
        }
        if self.http_allow_hosts.is_empty() {
            return true;
        }
        self.http_allow_hosts.iter().any(|a| {
            host == a.to_ascii_lowercase()
                || host.ends_with(&format!(".{}", a.to_ascii_lowercase()))
        })
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(c) => out.push(c),
        }
    }
    out
}

fn resolve_under_workspace(workspace: &Path, path: &Path) -> Result<PathBuf> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    };
    let normalized = normalize_path(&joined);
    if !normalized.starts_with(workspace) {
        // Best-effort string prefix check before existence.
        let ws = workspace.to_string_lossy();
        let n = normalized.to_string_lossy();
        if !n.starts_with(ws.as_ref()) {
            return Err(ToolError::PermissionDenied(format!(
                "path `{}` escapes workspace `{}`",
                normalized.display(),
                workspace.display()
            )));
        }
    }
    // Create intermediate path check by finding existing prefix.
    let mut current = PathBuf::new();
    for comp in normalized.components() {
        current.push(comp.as_os_str());
        if current.exists() {
            continue;
        }
        // Parent of first missing component must exist and be under workspace.
        break;
    }
    Ok(normalized)
}

fn is_blocked_ip_literal(host: &str) -> bool {
    if host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("169.254.")
        || host == "0"
        || host.starts_with("[")
    {
        return true;
    }
    if let Some(rest) = host.strip_prefix("172.") {
        if let Some(second) = rest.split('.').next() {
            if let Ok(n) = second.parse::<u8>() {
                if (16..=31).contains(&n) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn sandbox_blocks_escape() {
        let dir = tempdir().unwrap();
        let policy = PermissionPolicy::default();
        let err = policy
            .resolve_path(dir.path(), "../outside.txt")
            .unwrap_err();
        assert!(matches!(err, ToolError::PermissionDenied(_)));
    }

    #[test]
    fn sandbox_allows_inside() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let policy = PermissionPolicy::default();
        let p = policy.resolve_path(dir.path(), "a.txt").unwrap();
        assert!(p.ends_with("a.txt"));
        assert!(p.starts_with(dir.path().canonicalize().unwrap()));
    }

    #[test]
    fn blocks_metadata_host() {
        let policy = PermissionPolicy::default();
        assert!(!policy.http_host_allowed("169.254.169.254"));
        assert!(!policy.http_host_allowed("localhost"));
        assert!(policy.http_host_allowed("example.com"));
    }
}
