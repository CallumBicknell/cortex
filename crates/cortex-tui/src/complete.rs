//! Composer autocomplete for `/skills` and `@paths` (Claude Code–style).

use std::fs;
use std::path::{Path, PathBuf};

/// Maximum completion candidates shown.
const MAX_ITEMS: usize = 12;

/// Built-in slash commands (not skills).
pub const META_COMMANDS: &[(&str, &str)] = &[
    ("help", "Show commands and keys"),
    ("skills", "List available skills"),
    ("copy", "Copy last assistant reply to clipboard"),
    ("new", "Start a fresh session"),
    ("clear", "Alias for /new"),
    ("sessions", "Open sessions list"),
    ("yolo", "Toggle auto-approve tools"),
    ("quit", "Exit chat"),
    ("exit", "Exit chat"),
    ("q", "Exit chat"),
];

/// Kind of active completion popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompleteKind {
    /// `/command` or `/skill-id`
    Slash,
    /// `@relative/path`
    Path,
}

/// One row in the completion popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// Primary label (command/skill id or path).
    pub label: String,
    /// Full replacement for the active token (includes `/` or `@`).
    pub insert: String,
    /// Secondary detail line.
    pub detail: String,
}

/// Live completion state while the user types.
#[derive(Debug, Clone)]
pub struct CompletionState {
    /// Slash vs path.
    pub kind: CompleteKind,
    /// Filtered candidates.
    pub items: Vec<CompletionItem>,
    /// Selected index.
    pub selected: usize,
    /// Byte range `[start, end)` in the input buffer to replace.
    pub range: (usize, usize),
}

impl CompletionState {
    /// Move selection up.
    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.items.len() - 1
        } else {
            self.selected - 1
        };
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.items.len();
    }

    /// Currently selected item.
    pub fn current(&self) -> Option<&CompletionItem> {
        self.items.get(self.selected)
    }
}

/// Detect and build completion for the token at the end of `input`.
///
/// `skill_ids` should already be sorted; `workspace` is the project root for `@` paths.
pub fn refresh_completion(
    input: &str,
    skill_ids: &[String],
    skill_details: &[(String, String)],
    workspace: &Path,
) -> Option<CompletionState> {
    let (start, token) = current_token(input)?;
    if token.is_empty() {
        return None;
    }

    if let Some(prefix) = token.strip_prefix('/') {
        // No spaces inside slash tokens we complete.
        if prefix.contains(char::is_whitespace) {
            return None;
        }
        let items = slash_candidates(prefix, skill_ids, skill_details);
        if items.is_empty() {
            return None;
        }
        return Some(CompletionState {
            kind: CompleteKind::Slash,
            items,
            selected: 0,
            range: (start, input.len()),
        });
    }

    if let Some(partial) = token.strip_prefix('@') {
        // Paths may include `/` `.` `-` `_` but not whitespace.
        if partial.contains(char::is_whitespace) {
            return None;
        }
        let items = path_candidates(workspace, partial);
        if items.is_empty() {
            return None;
        }
        return Some(CompletionState {
            kind: CompleteKind::Path,
            items,
            selected: 0,
            range: (start, input.len()),
        });
    }

    None
}

/// Apply the selected completion into `input`, returning the new buffer.
pub fn apply_completion(input: &str, state: &CompletionState) -> String {
    let Some(item) = state.current() else {
        return input.to_string();
    };
    let (start, end) = state.range;
    let start = start.min(input.len());
    let end = end.min(input.len()).max(start);
    let mut out = String::with_capacity(input.len() + item.insert.len());
    out.push_str(&input[..start]);
    out.push_str(&item.insert);
    // Trailing space after slash skills/commands so user can keep typing.
    // Directories keep trailing `/` and get no extra space so nesting continues.
    let needs_space = match state.kind {
        CompleteKind::Slash => true,
        CompleteKind::Path => !item.insert.ends_with('/'),
    };
    if needs_space && !out.ends_with(' ') {
        out.push(' ');
    }
    if end < input.len() {
        out.push_str(&input[end..]);
    }
    out
}

fn current_token(input: &str) -> Option<(usize, &str)> {
    if input.is_empty() {
        return None;
    }
    // Complete against the last line only (multi-line composer).
    let line_start = input.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = &input[line_start..];
    if line.is_empty() {
        return None;
    }
    let mut token_start = 0;
    for (idx, ch) in line.char_indices() {
        if ch.is_whitespace() {
            token_start = idx + ch.len_utf8();
        }
    }
    let token = &line[token_start..];
    if token.is_empty() {
        return None;
    }
    Some((line_start + token_start, token))
}

fn slash_candidates(
    prefix: &str,
    skill_ids: &[String],
    skill_details: &[(String, String)],
) -> Vec<CompletionItem> {
    let prefix_l = prefix.to_ascii_lowercase();
    let mut items = Vec::new();

    for (name, detail) in META_COMMANDS {
        if name.starts_with(&prefix_l) || prefix_l.is_empty() {
            items.push(CompletionItem {
                label: format!("/{name}"),
                insert: format!("/{name}"),
                detail: (*detail).into(),
            });
        }
    }

    for id in skill_ids {
        let id_l = id.to_ascii_lowercase();
        if id_l.starts_with(&prefix_l) || prefix_l.is_empty() {
            let detail = skill_details
                .iter()
                .find(|(k, _)| k == id)
                .map(|(_, d)| d.clone())
                .unwrap_or_else(|| "skill".into());
            // Avoid duplicating meta names if a skill ever collides.
            if META_COMMANDS.iter().any(|(n, _)| *n == id.as_str()) {
                continue;
            }
            items.push(CompletionItem {
                label: format!("/{id}"),
                insert: format!("/{id}"),
                detail,
            });
        }
    }

    items.truncate(MAX_ITEMS);
    items
}

fn path_candidates(workspace: &Path, partial: &str) -> Vec<CompletionItem> {
    let partial = partial.trim_start_matches("./");
    let (dir_rel, file_prefix) = split_path_partial(partial);
    let base = if dir_rel.is_empty() {
        workspace.to_path_buf()
    } else {
        workspace.join(&dir_rel)
    };

    if !base.is_dir() {
        // Parent missing — try fuzzy basename search from workspace root (shallow).
        return fuzzy_basename(workspace, partial);
    }

    let prefix_l = file_prefix.to_ascii_lowercase();
    let mut entries: Vec<(String, bool)> = Vec::new();
    let Ok(read) = fs::read_dir(&base) else {
        return Vec::new();
    };
    for ent in read.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') && !prefix_l.starts_with('.') {
            continue;
        }
        let name_l = name.to_ascii_lowercase();
        if !prefix_l.is_empty() && !name_l.starts_with(&prefix_l) {
            continue;
        }
        let is_dir = ent.file_type().map(|t| t.is_dir()).unwrap_or(false);
        entries.push((name, is_dir));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut items = Vec::new();
    for (name, is_dir) in entries.into_iter().take(MAX_ITEMS) {
        let rel = if dir_rel.is_empty() {
            name.clone()
        } else {
            format!("{dir_rel}/{name}")
        };
        let insert = if is_dir {
            format!("@{rel}/")
        } else {
            format!("@{rel}")
        };
        items.push(CompletionItem {
            label: insert.clone(),
            insert,
            detail: if is_dir {
                "directory".into()
            } else {
                "file".into()
            },
        });
    }
    items
}

/// Split `src/app` → (`src`, `app`); `src/` → (`src`, ``); `app` → (``, `app`).
fn split_path_partial(partial: &str) -> (String, String) {
    if partial.is_empty() {
        return (String::new(), String::new());
    }
    if let Some(i) = partial.rfind('/') {
        let dir = partial[..i].to_string();
        let file = partial[i + 1..].to_string();
        (dir, file)
    } else {
        (String::new(), partial.to_string())
    }
}

fn fuzzy_basename(workspace: &Path, needle: &str) -> Vec<CompletionItem> {
    if needle.is_empty() || needle.contains('/') {
        return Vec::new();
    }
    let needle_l = needle.to_ascii_lowercase();
    let mut hits: Vec<PathBuf> = Vec::new();
    shallow_find(workspace, workspace, &needle_l, 0, 3, &mut hits);
    hits.sort();
    hits.truncate(MAX_ITEMS);
    hits.into_iter()
        .filter_map(|p| {
            let rel = p
                .strip_prefix(workspace)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            let is_dir = p.is_dir();
            let insert = if is_dir {
                format!("@{rel}/")
            } else {
                format!("@{rel}")
            };
            Some(CompletionItem {
                label: insert.clone(),
                insert,
                detail: if is_dir {
                    "directory".into()
                } else {
                    "file".into()
                },
            })
        })
        .collect()
}

fn shallow_find(
    workspace: &Path,
    dir: &Path,
    needle_l: &str,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<PathBuf>,
) {
    if out.len() >= MAX_ITEMS || depth > max_depth {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for ent in read.flatten() {
        if out.len() >= MAX_ITEMS {
            break;
        }
        let name = ent.file_name().to_string_lossy().into_owned();
        if name == ".git" || name == "target" || name == "node_modules" {
            continue;
        }
        let path = ent.path();
        if name.to_ascii_lowercase().contains(needle_l) {
            if let Ok(rel) = path.strip_prefix(workspace) {
                // Prefer matches under workspace.
                let _ = rel;
                out.push(path.clone());
            }
        }
        if depth < max_depth && path.is_dir() {
            shallow_find(workspace, &path, needle_l, depth + 1, max_depth, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn current_token_last_word() {
        let (s, t) = current_token("hello /gi").unwrap();
        assert_eq!(t, "/gi");
        assert_eq!(s, 6);
    }

    #[test]
    fn slash_filters_skills_and_meta() {
        let skills = vec!["git".into(), "web".into(), "solidity".into()];
        let details = vec![("git".into(), "git tools".into())];
        let items = slash_candidates("gi", &skills, &details);
        assert!(items.iter().any(|i| i.label == "/git"));
        assert!(!items.iter().any(|i| i.label == "/web"));
        let help = slash_candidates("he", &skills, &details);
        assert!(help.iter().any(|i| i.label == "/help"));
    }

    #[test]
    fn path_lists_dir() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("README.md"), "hi").unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src").join("main.rs"), "fn main(){}").unwrap();

        let top = path_candidates(dir.path(), "");
        assert!(top.iter().any(|i| i.label == "@README.md"));
        assert!(top.iter().any(|i| i.label == "@src/"));

        let nested = path_candidates(dir.path(), "src/");
        assert!(nested.iter().any(|i| i.label == "@src/main.rs"));

        let filtered = path_candidates(dir.path(), "src/ma");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].insert, "@src/main.rs");
    }

    #[test]
    fn apply_replaces_token() {
        let state = CompletionState {
            kind: CompleteKind::Slash,
            items: vec![CompletionItem {
                label: "/git".into(),
                insert: "/git".into(),
                detail: "d".into(),
            }],
            selected: 0,
            range: (6, 9),
        };
        let out = apply_completion("hello /gi", &state);
        assert_eq!(out, "hello /git ");
    }

    #[test]
    fn refresh_detects_at() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.rs"), "").unwrap();
        let c = refresh_completion("@a", &[], &[], dir.path()).unwrap();
        assert_eq!(c.kind, CompleteKind::Path);
        assert!(c.items.iter().any(|i| i.insert == "@a.rs"));
    }
}
