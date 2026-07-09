//! Repository map for prompt context.

use crate::error::Result;
use crate::ignore_rules::list_files;
use crate::project::ProjectInfo;
use crate::root::detect_root;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A compact map of the workspace for the agent context window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMap {
    /// Absolute workspace root.
    pub root: PathBuf,
    /// Project detection summary.
    pub project: ProjectInfo,
    /// Relative paths of important files (configs, readme, …).
    pub important_files: Vec<String>,
    /// Indented tree of top-level entries (and one level of children for dirs).
    pub tree: String,
    /// Total files considered (may be capped).
    pub file_count: usize,
}

impl RepoMap {
    /// Build a repo map for `start` (detects git root).
    pub fn build(start: impl AsRef<Path>) -> Result<Self> {
        Self::build_with_limits(start, 400, 80)
    }

    /// Build with explicit caps.
    pub fn build_with_limits(
        start: impl AsRef<Path>,
        max_files: usize,
        max_tree_lines: usize,
    ) -> Result<Self> {
        let root = detect_root(start)?;
        let project = ProjectInfo::detect(&root);
        let files = list_files(&root, max_files)?;
        let file_count = files.len();

        let mut important = project.key_files.clone();
        // Prefer shallow source-ish files for "important" if few key files.
        for f in files.iter().take(30) {
            let s = f.to_string_lossy().to_string();
            let interesting = s.ends_with(".rs")
                || s.ends_with(".toml")
                || s.ends_with(".md")
                || s.ends_with(".py")
                || s.ends_with(".ts")
                || s.ends_with(".sol");
            if interesting && s.matches('/').count() <= 2 && !important.contains(&s) {
                important.push(s);
            }
        }
        important.sort();
        important.dedup();
        important.truncate(40);

        let tree = render_tree(&files, max_tree_lines);

        Ok(Self {
            root,
            project,
            important_files: important,
            tree,
            file_count,
        })
    }

    /// Format for injection into an LLM system/context message.
    pub fn to_prompt_section(&self) -> String {
        let mut out = String::new();
        out.push_str("## Workspace\n");
        out.push_str(&format!("root: {}\n", self.root.display()));
        out.push_str(&format!("files_indexed: {}\n", self.file_count));
        out.push('\n');
        out.push_str("### Project\n");
        out.push_str(&self.project.summary());
        out.push_str("\n\n");
        if !self.important_files.is_empty() {
            out.push_str("### Important files\n");
            for f in &self.important_files {
                out.push_str("- ");
                out.push_str(f);
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str("### Tree\n");
        out.push_str("```\n");
        out.push_str(&self.tree);
        if !self.tree.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("```\n");
        out
    }
}

fn render_tree(files: &[PathBuf], max_lines: usize) -> String {
    // Group by top-level component.
    let mut top: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in files {
        let mut comps = f.components();
        let Some(first) = comps.next() else { continue };
        let key = first.as_os_str().to_string_lossy().to_string();
        let rest = f
            .components()
            .skip(1)
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/");
        top.entry(key).or_default().push(rest);
    }

    let mut lines = Vec::new();
    for (name, children) in top {
        if children.len() == 1 && children[0].is_empty() {
            lines.push(name);
            continue;
        }
        // Directory-like
        if children.iter().any(|c| !c.is_empty()) {
            lines.push(format!("{name}/"));
            let mut shown = 0;
            let mut child_names: Vec<_> = children
                .iter()
                .filter(|c| !c.is_empty())
                .map(|c| c.split('/').next().unwrap_or(c).to_string())
                .collect();
            child_names.sort();
            child_names.dedup();
            for child in child_names.iter().take(12) {
                lines.push(format!("  {child}"));
                shown += 1;
            }
            if child_names.len() > shown {
                lines.push(format!("  … +{} more", child_names.len() - shown));
            }
        } else {
            lines.push(name);
        }
        if lines.len() >= max_lines {
            lines.push("…".into());
            break;
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn builds_map() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"demo\"\n").unwrap();
        fs::write(dir.path().join("README.md"), "# demo\n").unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "pub fn x() {}\n").unwrap();

        let map = RepoMap::build(dir.path()).unwrap();
        assert!(map.file_count >= 3);
        let section = map.to_prompt_section();
        assert!(section.contains("Workspace"));
        assert!(section.contains("rust") || section.contains("Cargo.toml"));
    }
}
