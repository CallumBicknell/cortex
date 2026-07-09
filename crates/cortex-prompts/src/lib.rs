//! Prompt catalog: load markdown prompts and render `{{var}}` templates.
//!
//! Builtin prompts are embedded from the repo `prompts/` tree. Additional
//! prompts can be loaded from disk at runtime.

#![deny(missing_docs)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Prompt load/render errors.
#[derive(Debug, Error)]
pub enum PromptError {
    /// Missing prompt id.
    #[error("prompt not found: {0}")]
    NotFound(String),
    /// I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, PromptError>;

/// A named prompt body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// Stable id (e.g. `"system"`, `"skills/rust"`).
    pub id: String,
    /// Markdown body.
    pub body: String,
}

impl Prompt {
    /// Render `{{key}}` placeholders from `vars`. Unknown keys are left as-is.
    pub fn render(&self, vars: &HashMap<String, String>) -> String {
        render_template(&self.body, vars)
    }
}

/// In-memory prompt catalog.
#[derive(Debug, Clone, Default)]
pub struct PromptCatalog {
    prompts: HashMap<String, Prompt>,
}

impl PromptCatalog {
    /// Empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builtin prompts embedded at compile time.
    pub fn with_builtins() -> Self {
        let mut cat = Self::new();
        for (id, body) in BUILTIN_PROMPTS {
            cat.insert(Prompt {
                id: (*id).to_string(),
                body: (*body).to_string(),
            });
        }
        cat
    }

    /// Insert or replace a prompt.
    pub fn insert(&mut self, prompt: Prompt) {
        self.prompts.insert(prompt.id.clone(), prompt);
    }

    /// Get a prompt by id.
    pub fn get(&self, id: &str) -> Result<&Prompt> {
        self.prompts
            .get(id)
            .ok_or_else(|| PromptError::NotFound(id.to_string()))
    }

    /// Render a prompt by id.
    pub fn render(&self, id: &str, vars: &HashMap<String, String>) -> Result<String> {
        Ok(self.get(id)?.render(vars))
    }

    /// List prompt ids (sorted).
    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<_> = self.prompts.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Load all `*.md` files under `dir` (id = relative path without `.md`).
    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> Result<usize> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut count = 0;
        load_dir_rec(dir, dir, self, &mut count)?;
        Ok(count)
    }
}

fn load_dir_rec(
    root: &Path,
    current: &Path,
    cat: &mut PromptCatalog,
    count: &mut usize,
) -> Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            load_dir_rec(root, &path, cat, count)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(&path).with_extension("");
        let id = path_to_id(&rel);
        let body = std::fs::read_to_string(&path)?;
        cat.insert(Prompt { id, body });
        *count += 1;
    }
    Ok(())
}

fn path_to_id(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Replace `{{key}}` with values from `vars`.
pub fn render_template(template: &str, vars: &HashMap<String, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        let needle = format!("{{{{{k}}}}}");
        out = out.replace(&needle, v);
    }
    out
}

/// Embedded builtin prompts (synced with `prompts/` in the repo).
const BUILTIN_PROMPTS: &[(&str, &str)] = &[
    ("system", include_str!("../../../prompts/system.md")),
    ("planner", include_str!("../../../prompts/planner.md")),
    ("coding", include_str!("../../../prompts/coding.md")),
    ("review", include_str!("../../../prompts/review.md")),
    ("security", include_str!("../../../prompts/security.md")),
    ("skills/git", include_str!("../../../prompts/skills/git.md")),
    ("skills/web", include_str!("../../../prompts/skills/web.md")),
    (
        "skills/testing",
        include_str!("../../../prompts/skills/testing.md"),
    ),
    (
        "skills/rust",
        include_str!("../../../prompts/skills/rust.md"),
    ),
    (
        "skills/python",
        include_str!("../../../prompts/skills/python.md"),
    ),
    (
        "skills/javascript",
        include_str!("../../../prompts/skills/javascript.md"),
    ),
    (
        "skills/solidity",
        include_str!("../../../prompts/skills/solidity.md"),
    ),
    (
        "skills/sc_security",
        include_str!("../../../prompts/skills/sc_security.md"),
    ),
    (
        "skills/skill_creator",
        include_str!("../../../prompts/skills/skill_creator.md"),
    ),
    (
        "skills/frontend_design",
        include_str!("../../../prompts/skills/frontend_design.md"),
    ),
];

/// Default prompts directory relative to a workspace root.
pub fn default_prompts_dir(workspace: &Path) -> PathBuf {
    workspace.join("prompts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_load() {
        let cat = PromptCatalog::with_builtins();
        assert!(cat.get("system").is_ok());
        assert!(cat.get("skills/rust").is_ok());
        assert!(cat.get("skills/sc_security").is_ok());
        assert!(cat.get("skills/solidity").is_ok());
        assert!(cat.ids().len() >= 10);
    }

    #[test]
    fn render_vars() {
        let mut vars = HashMap::new();
        vars.insert("name".into(), "Cortex".into());
        let out = render_template("Hello {{name}}!", &vars);
        assert_eq!(out, "Hello Cortex!");
    }

    #[test]
    fn load_dir_works() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("custom.md"), "hi {{x}}").unwrap();
        let mut cat = PromptCatalog::new();
        assert_eq!(cat.load_dir(dir.path()).unwrap(), 1);
        let mut vars = HashMap::new();
        vars.insert("x".into(), "there".into());
        assert_eq!(cat.render("custom", &vars).unwrap(), "hi there");
    }
}
