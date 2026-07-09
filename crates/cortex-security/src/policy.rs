//! Serializable security policy.

use crate::error::{Result, SecurityError};
use cortex_tools::{PermissionMode, PermissionPolicy};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// On-disk / in-memory security policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    /// Default mode for unlisted tools.
    #[serde(default = "default_ask")]
    pub default_mode: PermissionMode,
    /// Per-tool modes.
    #[serde(default)]
    pub tools: HashMap<String, PermissionMode>,
    /// Path sandbox under workspace.
    #[serde(default = "default_true")]
    pub sandbox_workspace: bool,
    /// Tool output byte cap.
    #[serde(default = "default_max_output")]
    pub max_output_bytes: usize,
    /// Shell timeout seconds.
    #[serde(default = "default_shell_timeout")]
    pub shell_timeout_secs: u64,
    /// YOLO / approve-all (prefer CLI flag; config override allowed).
    #[serde(default)]
    pub yolo: bool,
    /// HTTP allow hosts (empty = unrestricted except blocks).
    #[serde(default)]
    pub http_allow_hosts: Vec<String>,
    /// HTTP block hosts.
    #[serde(default)]
    pub http_block_hosts: Vec<String>,
    /// Shell command substrings that are hard-denied.
    #[serde(default)]
    pub shell_deny_patterns: Vec<String>,
    /// Env var names scrubbed from shell children.
    #[serde(default)]
    pub scrub_env: Vec<String>,
    /// Prefer bubblewrap for shell when available.
    #[serde(default = "default_true")]
    pub shell_use_bubblewrap: bool,
}

fn default_ask() -> PermissionMode {
    PermissionMode::Ask
}
fn default_true() -> bool {
    true
}
fn default_max_output() -> usize {
    256 * 1024
}
fn default_shell_timeout() -> u64 {
    60
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        let tools_policy = PermissionPolicy::default();
        Self {
            default_mode: tools_policy.default_mode,
            tools: tools_policy.tools,
            sandbox_workspace: true,
            max_output_bytes: 256 * 1024,
            shell_timeout_secs: 60,
            yolo: false,
            http_allow_hosts: Vec::new(),
            http_block_hosts: tools_policy.http_block_hosts,
            shell_deny_patterns: default_shell_deny_patterns(),
            scrub_env: default_scrub_env(),
            shell_use_bubblewrap: true,
        }
    }
}

impl SecurityPolicy {
    /// Load from TOML file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())?;
        Self::from_toml(&text)
    }

    /// Parse TOML.
    pub fn from_toml(text: &str) -> Result<Self> {
        let mut policy: SecurityPolicy =
            toml::from_str(text).map_err(|e| SecurityError::Parse(e.to_string()))?;
        if policy.http_block_hosts.is_empty() {
            policy.http_block_hosts = PermissionPolicy::default().http_block_hosts;
        }
        if policy.scrub_env.is_empty() {
            policy.scrub_env = default_scrub_env();
        }
        if policy.shell_deny_patterns.is_empty() {
            policy.shell_deny_patterns = default_shell_deny_patterns();
        }
        Ok(policy)
    }

    /// Apply yolo override.
    pub fn with_yolo(mut self, yolo: bool) -> Self {
        self.yolo = yolo || self.yolo;
        self
    }

    /// Convert to the runtime permission policy used by tools.
    pub fn to_permission_policy(&self) -> PermissionPolicy {
        if self.yolo {
            return PermissionPolicy::default().allow_all();
        }
        PermissionPolicy {
            default_mode: self.default_mode,
            tools: self.tools.clone(),
            sandbox_workspace: self.sandbox_workspace,
            max_output_bytes: self.max_output_bytes.max(1024),
            shell_timeout_secs: self.shell_timeout_secs.max(1),
            http_allow_hosts: self.http_allow_hosts.clone(),
            http_block_hosts: self.http_block_hosts.clone(),
            scrub_env: self.scrub_env.clone(),
            shell_deny_patterns: self.shell_deny_patterns.clone(),
            shell_use_bubblewrap: self.shell_use_bubblewrap,
        }
    }

    /// Hard-deny shell command if it matches a dangerous pattern.
    pub fn shell_command_allowed(&self, command: &str) -> bool {
        let lower = command.to_ascii_lowercase();
        !self
            .shell_deny_patterns
            .iter()
            .any(|p| lower.contains(&p.to_ascii_lowercase()))
    }

    /// Env keys to scrub for shell.
    pub fn scrub_env_keys(&self) -> &[String] {
        &self.scrub_env
    }
}

/// Default env vars stripped from shell subprocesses.
pub fn default_scrub_env() -> Vec<String> {
    [
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENROUTER_API_KEY",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        "AWS_ACCESS_KEY_ID",
        "GITHUB_TOKEN",
        "GH_TOKEN",
        "NPM_TOKEN",
        "HF_TOKEN",
        "PRIVATE_KEY",
        "SECRET_KEY",
        "DATABASE_URL",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn default_shell_deny_patterns() -> Vec<String> {
    [
        "rm -rf /",
        "mkfs",
        ":(){ :|:& };:",
        "dd if=/dev/zero",
        "shutdown",
        "reboot",
        "> /dev/sda",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sample_and_denies_rm_root() {
        let policy = SecurityPolicy::from_toml(
            r#"
            default_mode = "ask"
            shell_deny_patterns = ["rm -rf /"]
            [tools]
            read_file = "allow"
            shell = "ask"
            "#,
        )
        .unwrap();
        assert!(!policy.shell_command_allowed("sudo rm -rf /"));
        assert!(policy.shell_command_allowed("cargo test"));
        let pp = policy.to_permission_policy();
        assert_eq!(pp.mode_for("read_file"), PermissionMode::Allow);
    }

    #[test]
    fn yolo_allows_all() {
        let p = SecurityPolicy::default().with_yolo(true);
        assert_eq!(
            p.to_permission_policy().mode_for("shell"),
            PermissionMode::Allow
        );
    }
}
