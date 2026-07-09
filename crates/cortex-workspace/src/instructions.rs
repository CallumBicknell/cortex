//! Project instruction files (AGENTS.md / CLAUDE.md / .cortex/instructions.md).

use std::fs;
use std::path::{Path, PathBuf};

/// Max bytes to inject from a project instruction file (avoid blowing context).
pub const MAX_INSTRUCTION_BYTES: usize = 32_768;

/// Candidate paths, relative to workspace root, in precedence order.
///
/// First existing readable file wins.
pub fn instruction_candidates(workspace: &Path) -> Vec<PathBuf> {
    vec![
        workspace.join(".cortex").join("instructions.md"),
        workspace.join("AGENTS.md"),
        workspace.join("CLAUDE.md"),
        workspace.join(".cursorrules"),
        workspace.join("CORTEX.md"),
    ]
}

/// Loaded project instructions ready for context injection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInstructions {
    /// Absolute or workspace-relative path that was loaded.
    pub path: PathBuf,
    /// Display label (file name).
    pub label: String,
    /// File body (possibly truncated).
    pub body: String,
    /// True if truncated to [`MAX_INSTRUCTION_BYTES`].
    pub truncated: bool,
}

impl ProjectInstructions {
    /// Markdown section for a system message.
    pub fn to_prompt_section(&self) -> String {
        let trunc = if self.truncated {
            "\n\n…[project instructions truncated]"
        } else {
            ""
        };
        format!(
            "## Project instructions ({})\n\n{}{}",
            self.label,
            self.body.trim(),
            trunc
        )
    }
}

/// Load the first available instruction file under `workspace`.
pub fn load_project_instructions(workspace: impl AsRef<Path>) -> Option<ProjectInstructions> {
    let workspace = workspace.as_ref();
    for path in instruction_candidates(workspace) {
        if !path.is_file() {
            continue;
        }
        let raw = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if raw.is_empty() {
            continue;
        }
        let truncated = raw.len() > MAX_INSTRUCTION_BYTES;
        let slice = if truncated {
            &raw[..MAX_INSTRUCTION_BYTES]
        } else {
            &raw[..]
        };
        // Lossy UTF-8 is fine for agent context.
        let body = String::from_utf8_lossy(slice).into_owned();
        if body.trim().is_empty() {
            continue;
        }
        let label = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("instructions")
            .to_string();
        return Some(ProjectInstructions {
            path,
            label,
            body,
            truncated,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn prefers_cortex_instructions_over_agents() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".cortex")).unwrap();
        fs::write(dir.path().join("AGENTS.md"), "from agents").unwrap();
        fs::write(dir.path().join(".cortex/instructions.md"), "from cortex").unwrap();
        let loaded = load_project_instructions(dir.path()).unwrap();
        assert_eq!(loaded.label, "instructions.md");
        assert!(loaded.body.contains("from cortex"));
    }

    #[test]
    fn loads_agents_md() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Rules\n- use forge").unwrap();
        let loaded = load_project_instructions(dir.path()).unwrap();
        assert_eq!(loaded.label, "AGENTS.md");
        assert!(loaded.to_prompt_section().contains("use forge"));
    }

    #[test]
    fn missing_returns_none() {
        let dir = tempdir().unwrap();
        assert!(load_project_instructions(dir.path()).is_none());
    }
}
