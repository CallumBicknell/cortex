//! Import external agent skill packs (SKILL.md) into Cortex learned skills.

use crate::error::{Result, SkillError};
use crate::skill::Skill;
use crate::store::{validate_skill_id_pub, SkillDocument, SkillOrigin};
use std::fs;
use std::path::Path;

/// Default tools for imported Web3 / audit skills (safe, useful baseline).
pub const DEFAULT_IMPORT_TOOLS: &[&str] = &[
    "read_file",
    "write_file",
    "edit_file",
    "list_dir",
    "glob_files",
    "apply_patch",
    "code_outline",
    "workspace_symbols",
    "shell",
    "http_request",
    "web_search",
];

/// Result of converting a SKILL.md (or plain markdown) into a Cortex pack.
#[derive(Debug, Clone)]
pub struct ImportedSkill {
    /// Disk document to save under `.cortex/skills/`.
    pub document: SkillDocument,
    /// Prompt id relative to the prompt catalog (e.g. `skills/pashov_xray`).
    pub prompt_id: String,
    /// Markdown body for the prompt file.
    pub prompt_body: String,
    /// Source attribution string.
    pub source: String,
}

/// Parse YAML-ish frontmatter + body from SKILL.md content.
///
/// Supports:
/// ```text
/// ---
/// name: my-skill
/// description: ...
/// ---
/// # Body
/// ```
pub fn parse_skill_md(content: &str) -> Result<(String, String, String)> {
    let content = content.trim_start_matches('\u{feff}');
    let mut name = String::new();
    let mut description = String::new();
    let body: String;

    if let Some(rest) = content.strip_prefix("---") {
        let rest = rest.trim_start_matches(['\r', '\n']);
        if let Some(end) = rest.find("\n---") {
            let fm = &rest[..end];
            body = rest[end + 4..].trim_start_matches(['\r', '\n']).to_string();
            for line in fm.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once(':') {
                    let k = k.trim().to_ascii_lowercase();
                    let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
                    match k.as_str() {
                        "name" | "id" => name = v,
                        "description" | "desc" => description = v,
                        _ => {}
                    }
                }
            }
        } else {
            body = content.to_string();
        }
    } else {
        body = content.to_string();
    }

    if name.is_empty() {
        // First markdown heading
        for line in body.lines() {
            let t = line.trim();
            if let Some(h) = t.strip_prefix("# ") {
                name = h.trim().to_string();
                break;
            }
        }
    }
    if description.is_empty() {
        // First non-empty non-heading paragraph
        for line in body.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') || t.starts_with("---") {
                continue;
            }
            description = t.chars().take(240).collect();
            break;
        }
    }
    if name.is_empty() {
        name = "imported_skill".into();
    }
    if description.is_empty() {
        description = format!("Imported skill: {name}");
    }

    Ok((name, description, body))
}

/// Normalize a free-form name into a skill id.
pub fn normalize_skill_id(raw: &str) -> Result<String> {
    let mut id = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c.to_ascii_lowercase());
        } else if (c == '_' || c == '-' || c == ' ' || c == '/')
            && !id.ends_with('_')
            && !id.is_empty()
        {
            id.push('_');
        }
    }
    let id = id.trim_matches('_').to_string();
    if id.is_empty() {
        return Err(SkillError::Invalid("could not derive skill id".into()));
    }
    // Cap length
    let id: String = id.chars().take(64).collect();
    validate_skill_id_pub(&id)?;
    Ok(id)
}

/// Options for import conversion.
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Override skill id.
    pub id: Option<String>,
    /// Extra / override tools (empty = defaults).
    pub tools: Vec<String>,
    /// Extra tags.
    pub tags: Vec<String>,
    /// Source URL or path for notes.
    pub source: String,
}

/// Convert SKILL.md content into an [`ImportedSkill`].
pub fn import_from_markdown(content: &str, opts: ImportOptions) -> Result<ImportedSkill> {
    let (name, description, body) = parse_skill_md(content)?;
    let id = if let Some(id) = opts.id {
        validate_skill_id_pub(&id)?;
        id
    } else {
        normalize_skill_id(&name)?
    };

    let tools: Vec<String> = if opts.tools.is_empty() {
        DEFAULT_IMPORT_TOOLS
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    } else {
        opts.tools
    };

    let mut tags = opts.tags;
    tags.push("imported".into());
    tags.push("web3".into());
    tags.push(id.clone());
    // Token tags from name
    for part in id.split('_') {
        if part.len() > 2 {
            tags.push(part.to_string());
        }
    }
    tags.sort();
    tags.dedup();

    let prompt_id = format!("skills/{id}");
    let prompt_body = body;
    let notes = format!(
        "Imported from {}. Use prompt id `{prompt_id}`. Review tools/tags before production use.",
        if opts.source.is_empty() {
            "markdown"
        } else {
            &opts.source
        }
    );

    let skill = Skill::new(id, description)
        .tools(tools)
        .tags(tags)
        .prompts([prompt_id.clone()]);

    Ok(ImportedSkill {
        document: SkillDocument {
            skill,
            origin: SkillOrigin::Learned,
            notes,
            score: 0,
        },
        prompt_id,
        prompt_body,
        source: opts.source,
    })
}

/// Load markdown from a local path (file or directory containing SKILL.md).
pub fn read_skill_source(path: &Path) -> Result<(String, String)> {
    let path = if path.is_dir() {
        let candidates = ["SKILL.md", "skill.md", "README.md"];
        let mut found = None;
        for c in candidates {
            let p = path.join(c);
            if p.is_file() {
                found = Some(p);
                break;
            }
        }
        found.ok_or_else(|| {
            SkillError::Invalid(format!("no SKILL.md in directory {}", path.display()))
        })?
    } else {
        path.to_path_buf()
    };
    let text = fs::read_to_string(&path).map_err(SkillError::Io)?;
    Ok((path.display().to_string(), text))
}

/// Write imported skill pack under a workspace (`.cortex/skills` + `.cortex/prompts`).
pub fn write_imported_skill(
    workspace: &Path,
    imported: &ImportedSkill,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    use crate::store::SkillStore;

    let store = SkillStore::for_workspace(workspace);
    let skill_path = store.save(&imported.document)?;

    // prompt_id is like `skills/foo` → `.cortex/prompts/skills/foo.md`
    let mut prompt_file = workspace.join(".cortex").join("prompts");
    for part in imported.prompt_id.split('/') {
        prompt_file.push(part);
    }
    prompt_file.set_extension("md");
    if let Some(parent) = prompt_file.parent() {
        fs::create_dir_all(parent).map_err(SkillError::Io)?;
    }
    fs::write(&prompt_file, &imported.prompt_body).map_err(SkillError::Io)?;
    Ok((skill_path, prompt_file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parse_frontmatter() {
        let md = r#"---
name: solidity-auditor
description: Security audit of Solidity code
---

# Smart Contract Security

You audit contracts carefully.
"#;
        let (name, desc, body) = parse_skill_md(md).unwrap();
        assert_eq!(name, "solidity-auditor");
        assert!(desc.contains("Security audit"));
        assert!(body.contains("You audit"));
    }

    #[test]
    fn import_roundtrip_disk() {
        let dir = tempdir().unwrap();
        let md = r#"---
name: my-web3-skill
description: Do web3 things
---

Body guidance here.
"#;
        let imported = import_from_markdown(
            md,
            ImportOptions {
                source: "https://example.com/SKILL.md".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(imported.document.skill.id, "my_web3_skill");
        assert!(imported
            .document
            .skill
            .prompts
            .iter()
            .any(|p| p == "skills/my_web3_skill"));

        let (sp, pp) = write_imported_skill(dir.path(), &imported).unwrap();
        assert!(sp.is_file());
        assert!(pp.is_file());
        let loaded = crate::store::SkillStore::for_workspace(dir.path())
            .load_file(&sp)
            .unwrap();
        assert_eq!(loaded.skill.id, "my_web3_skill");
    }

    #[test]
    fn normalize_id() {
        assert_eq!(normalize_skill_id("Pashov X-Ray").unwrap(), "pashov_x_ray");
    }
}
