//! Lightweight project / stack detection.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Detected project characteristics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// Primary languages (heuristic).
    pub languages: Vec<String>,
    /// Package managers / build tools detected.
    pub package_managers: Vec<String>,
    /// Suggested test command, if known.
    pub test_command: Option<String>,
    /// Suggested format/lint command.
    pub lint_command: Option<String>,
    /// Important root config files found.
    pub key_files: Vec<String>,
}

impl ProjectInfo {
    /// Inspect `root` for known markers.
    pub fn detect(root: &Path) -> Self {
        let mut languages = Vec::new();
        let mut package_managers = Vec::new();
        let mut test_command = None;
        let mut lint_command = None;
        let mut key_files = Vec::new();

        let markers: &[(&str, &str)] = &[
            ("Cargo.toml", "rust"),
            ("package.json", "javascript"),
            ("pyproject.toml", "python"),
            ("requirements.txt", "python"),
            ("go.mod", "go"),
            ("pom.xml", "java"),
            ("build.gradle", "java"),
            ("Gemfile", "ruby"),
            ("composer.json", "php"),
            ("CMakeLists.txt", "cpp"),
            ("foundry.toml", "solidity"),
            ("hardhat.config.js", "solidity"),
            ("hardhat.config.ts", "solidity"),
            ("truffle-config.js", "solidity"),
        ];

        for (file, lang) in markers {
            if root.join(file).is_file() {
                key_files.push((*file).to_string());
                if !languages.iter().any(|l| l == lang) {
                    languages.push((*lang).to_string());
                }
            }
        }

        if root.join("Cargo.toml").is_file() {
            package_managers.push("cargo".into());
            test_command = Some("cargo test".into());
            lint_command = Some("cargo clippy --workspace -- -D warnings".into());
        }
        if root.join("package.json").is_file() {
            if root.join("pnpm-lock.yaml").is_file() {
                package_managers.push("pnpm".into());
                test_command = test_command.or(Some("pnpm test".into()));
            } else if root.join("yarn.lock").is_file() {
                package_managers.push("yarn".into());
                test_command = test_command.or(Some("yarn test".into()));
            } else {
                package_managers.push("npm".into());
                test_command = test_command.or(Some("npm test".into()));
            }
        }
        if root.join("pyproject.toml").is_file() || root.join("requirements.txt").is_file() {
            package_managers.push("pip/uv".into());
            test_command = test_command.or(Some("pytest".into()));
            lint_command = lint_command.or(Some("ruff check .".into()));
        }
        if root.join("go.mod").is_file() {
            package_managers.push("go".into());
            test_command = test_command.or(Some("go test ./...".into()));
        }
        if root.join("foundry.toml").is_file() {
            package_managers.push("forge".into());
            test_command = test_command.or(Some("forge test".into()));
        }

        for name in [
            "README.md",
            "README",
            "LICENSE",
            "AGENTS.md",
            "Makefile",
            "justfile",
        ] {
            if root.join(name).is_file() && !key_files.iter().any(|k| k == name) {
                key_files.push(name.to_string());
            }
        }

        languages.sort();
        languages.dedup();
        package_managers.sort();
        package_managers.dedup();
        key_files.sort();
        key_files.dedup();

        Self {
            languages,
            package_managers,
            test_command,
            lint_command,
            key_files,
        }
    }

    /// Short multi-line summary for prompts.
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        if !self.languages.is_empty() {
            lines.push(format!("languages: {}", self.languages.join(", ")));
        }
        if !self.package_managers.is_empty() {
            lines.push(format!("tooling: {}", self.package_managers.join(", ")));
        }
        if let Some(t) = &self.test_command {
            lines.push(format!("test: {t}"));
        }
        if let Some(l) = &self.lint_command {
            lines.push(format!("lint: {l}"));
        }
        if !self.key_files.is_empty() {
            lines.push(format!("key_files: {}", self.key_files.join(", ")));
        }
        if lines.is_empty() {
            "project: (unknown)".into()
        } else {
            lines.join("\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detects_rust() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        let info = ProjectInfo::detect(dir.path());
        assert!(info.languages.contains(&"rust".into()));
        assert_eq!(info.test_command.as_deref(), Some("cargo test"));
    }
}
