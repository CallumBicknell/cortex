//! Parse `/skill` slash tokens and `@path` attachments in chat prompts.

use crate::complete::META_COMMANDS;
use std::fs;
use std::path::{Path, PathBuf};

/// Max bytes to inline per attached file.
const MAX_FILE_BYTES: u64 = 80_000;
/// Max total attachment section size.
const MAX_ATTACH_TOTAL: usize = 200_000;
/// Max directory entries listed.
const MAX_DIR_ENTRIES: usize = 80;

/// Built-in meta slash commands handled by the TUI (not sent to the model).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaCommand {
    /// Show help.
    Help,
    /// Exit.
    Quit,
    /// New session.
    New,
    /// Sessions drawer.
    Sessions,
    /// Toggle yolo.
    Yolo,
    /// List skills.
    Skills,
    /// Export transcript as markdown.
    Export,
}

/// Result of parsing a user composer line.
#[derive(Debug, Clone)]
pub struct ParsedPrompt {
    /// Original text (for the transcript).
    pub display: String,
    /// Prompt text sent to the agent (attachments expanded, slash skills stripped).
    pub agent_prompt: String,
    /// Skill ids requested via `/skill-id`.
    pub skills: Vec<String>,
    /// Relative paths from `@path` tokens.
    pub attachments: Vec<String>,
    /// Meta command when the whole line is a UI command.
    pub meta: Option<MetaCommand>,
}

/// Parse composer text.
///
/// `known_skills` is the set of skill ids (lowercase match allowed).
pub fn parse_prompt(raw: &str, known_skills: &[String]) -> ParsedPrompt {
    let display = raw.to_string();
    let trimmed = raw.trim();

    if let Some(meta) = parse_meta_line(trimmed) {
        return ParsedPrompt {
            display,
            agent_prompt: String::new(),
            skills: Vec::new(),
            attachments: Vec::new(),
            meta: Some(meta),
        };
    }

    let known_l: Vec<String> = known_skills
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let mut skills = Vec::new();
    let mut attachments = Vec::new();
    let mut cleaned_parts: Vec<String> = Vec::new();

    for token in tokenize_preserving(raw) {
        if let Some(rest) = token.strip_prefix('/') {
            // Only pure skill/meta id tokens: `/git` not `/git/foo` or URLs.
            if is_skill_token(rest) {
                let id_l = rest.to_ascii_lowercase();
                if known_l.iter().any(|k| k == &id_l) {
                    // Preserve canonical casing from known list.
                    if let Some(canon) = known_skills.iter().find(|k| k.eq_ignore_ascii_case(rest))
                    {
                        if !skills.iter().any(|s: &String| s == canon) {
                            skills.push(canon.clone());
                        }
                    }
                    continue; // strip from agent prompt
                }
                if META_COMMANDS
                    .iter()
                    .any(|(n, _)| n.eq_ignore_ascii_case(rest))
                {
                    // Lone meta mid-line is ignored for agent; user should use full-line meta.
                    continue;
                }
            }
        }
        if let Some(path) = token.strip_prefix('@') {
            if !path.is_empty() && !path.contains(char::is_whitespace) {
                let path = path.trim_end_matches(['.', ',', ';', ':', ')', ']']);
                if !path.is_empty() {
                    let norm = path.trim_start_matches("./").to_string();
                    if !attachments.iter().any(|a| a == &norm) {
                        attachments.push(norm);
                    }
                }
            }
        }
        cleaned_parts.push(token);
    }

    let cleaned = rejoin_tokens(&cleaned_parts);
    ParsedPrompt {
        display,
        agent_prompt: cleaned,
        skills,
        attachments,
        meta: None,
    }
}

/// Expand `@` attachments into a markdown section for the agent prompt.
pub fn expand_attachments(workspace: &Path, paths: &[String], base_prompt: &str) -> String {
    if paths.is_empty() {
        return base_prompt.to_string();
    }

    let mut section = String::from("\n\n## Attached paths\n");
    let mut total = 0usize;

    for rel in paths {
        if total >= MAX_ATTACH_TOTAL {
            section.push_str("\n… further attachments truncated.\n");
            break;
        }
        let path = resolve_workspace_path(workspace, rel);
        section.push_str(&format!("\n### `{rel}`\n"));
        match load_attachment(&path, rel) {
            Ok(body) => {
                let take = body.len().min(MAX_ATTACH_TOTAL.saturating_sub(total));
                section.push_str(&body[..take]);
                if take < body.len() {
                    section.push_str("\n… truncated …\n");
                }
                total += take;
            }
            Err(e) => {
                section.push_str(&format!("_(could not read: {e})_\n"));
            }
        }
    }

    let mut out = base_prompt.trim_end().to_string();
    // Keep human @refs visible in the natural language part.
    if out.is_empty() {
        out = format!(
            "Please review the attached path(s): {}",
            paths
                .iter()
                .map(|p| format!("`{p}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    out.push_str(&section);
    out
}

fn parse_meta_line(s: &str) -> Option<MetaCommand> {
    let s = s.trim();
    if !s.starts_with('/') {
        return None;
    }
    // Only pure meta lines (optional trailing whitespace already trimmed).
    let cmd = s.trim_start_matches('/');
    if cmd.contains(char::is_whitespace) {
        return None;
    }
    match cmd.to_ascii_lowercase().as_str() {
        "help" => Some(MetaCommand::Help),
        "quit" | "exit" | "q" => Some(MetaCommand::Quit),
        "new" | "clear" => Some(MetaCommand::New),
        "sessions" => Some(MetaCommand::Sessions),
        "yolo" => Some(MetaCommand::Yolo),
        "skills" => Some(MetaCommand::Skills),
        "export" => Some(MetaCommand::Export),
        _ => None,
    }
}

fn is_skill_token(rest: &str) -> bool {
    !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Tokenize on whitespace but keep the whitespace structure loosely via separate tokens.
fn tokenize_preserving(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            // Collapse runs of whitespace to single space for agent prompt cleanliness.
            if out.last().map(|t| t != " ").unwrap_or(true) {
                // don't push pure whitespace tokens — rejoin with spaces
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn rejoin_tokens(parts: &[String]) -> String {
    parts.join(" ")
}

fn resolve_workspace_path(workspace: &Path, rel: &str) -> PathBuf {
    let p = PathBuf::from(rel);
    if p.is_absolute() {
        // Refuse absolute escapes outside workspace for safety — still try join denial.
        return workspace.join(rel.trim_start_matches('/'));
    }
    let mut clean = PathBuf::new();
    for comp in p.components() {
        use std::path::Component;
        match comp {
            Component::ParentDir => {
                clean.pop();
            }
            Component::CurDir => {}
            Component::Normal(s) => clean.push(s),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    workspace.join(clean)
}

fn load_attachment(path: &Path, rel: &str) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        return list_dir(path, rel);
    }
    if !meta.is_file() {
        return Err("not a file or directory".into());
    }
    if meta.len() > MAX_FILE_BYTES {
        // Read only the head.
        let data = fs::read(path).map_err(|e| e.to_string())?;
        if data.contains(&0) {
            return Err("binary file".into());
        }
        let take = MAX_FILE_BYTES as usize;
        let head = String::from_utf8_lossy(&data[..take.min(data.len())]);
        return Ok(format!(
            "```\n{head}\n```\n_(truncated; file is {} bytes)_\n",
            meta.len()
        ));
    }
    let data = fs::read(path).map_err(|e| e.to_string())?;
    if data.contains(&0) {
        return Err("binary file".into());
    }
    let text = String::from_utf8_lossy(&data);
    let fence = if rel.ends_with(".md") { "" } else { "```\n" };
    let fence_end = if rel.ends_with(".md") { "" } else { "\n```\n" };
    if rel.ends_with(".md") {
        Ok(format!("{text}\n"))
    } else {
        Ok(format!("{fence}{text}{fence_end}"))
    }
}

fn list_dir(path: &Path, rel: &str) -> Result<String, String> {
    let mut names: Vec<String> = fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() {
                format!("{n}/")
            } else {
                n
            }
        })
        .collect();
    names.sort();
    let total = names.len();
    names.truncate(MAX_DIR_ENTRIES);
    let mut out = format!("Directory listing for `{rel}` ({total} entries):\n");
    for n in names {
        out.push_str(&format!("- {n}\n"));
    }
    if total > MAX_DIR_ENTRIES {
        out.push_str(&format!("- … {} more\n", total - MAX_DIR_ENTRIES));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn meta_help() {
        let p = parse_prompt("/help", &[]);
        assert_eq!(p.meta, Some(MetaCommand::Help));
    }

    #[test]
    fn skill_slash_stripped_and_collected() {
        let known = vec!["git".into(), "web".into()];
        let p = parse_prompt("/git please commit", &known);
        assert_eq!(p.skills, vec!["git".to_string()]);
        assert!(p.agent_prompt.contains("please commit"));
        assert!(!p.agent_prompt.contains("/git"));
        assert!(p.meta.is_none());
    }

    /// Real chat line: `/browser visit …` must select the browser skill pack.
    #[test]
    fn browser_slash_like_chat() {
        let known = vec!["browser".into(), "git".into(), "web".into()];
        let p = parse_prompt(
            "/browser visit m.example.com for me and give a summary of the site",
            &known,
        );
        assert_eq!(p.skills, vec!["browser".to_string()]);
        assert!(p.agent_prompt.contains("visit m.example.com"));
        assert!(!p.agent_prompt.contains("/browser"));
        assert!(p.meta.is_none());
    }

    #[test]
    fn at_path_collected() {
        let p = parse_prompt("fix @src/main.rs and @README.md", &[]);
        assert_eq!(
            p.attachments,
            vec!["src/main.rs".to_string(), "README.md".to_string()]
        );
        assert!(p.agent_prompt.contains("@src/main.rs"));
    }

    #[test]
    fn expand_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
        let out = expand_attachments(dir.path(), &["a.rs".into()], "look at this");
        assert!(out.contains("fn a() {}"));
        assert!(out.contains("## Attached paths"));
    }

    #[test]
    fn path_traversal_stays_in_workspace() {
        let dir = TempDir::new().unwrap();
        let p = resolve_workspace_path(dir.path(), "../etc/passwd");
        assert!(p.starts_with(dir.path()));
    }
}
