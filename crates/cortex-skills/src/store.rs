//! Persistent skill packs under `.cortex/skills/` (self-evolving skills).

use crate::error::{Result, SkillError};
use crate::skill::Skill;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// On-disk skill document (TOML).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDocument {
    /// Skill definition.
    pub skill: Skill,
    /// Origin of the skill.
    #[serde(default)]
    pub origin: SkillOrigin,
    /// Optional free-form notes / evolution log.
    #[serde(default)]
    pub notes: String,
    /// How many times this skill was promoted / reinforced.
    #[serde(default)]
    pub score: u32,
}

/// Where a skill came from.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillOrigin {
    /// Shipped with Cortex.
    #[default]
    Builtin,
    /// Written by the agent / user into the workspace.
    Learned,
    /// Explicitly promoted as trusted.
    Promoted,
}

/// Disk-backed skill store.
#[derive(Debug, Clone)]
pub struct SkillStore {
    root: PathBuf,
}

impl SkillStore {
    /// Skills directory (created on first write).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Default path: `{workspace}/.cortex/skills`.
    pub fn for_workspace(workspace: impl AsRef<Path>) -> Self {
        Self::new(workspace.as_ref().join(".cortex").join("skills"))
    }

    /// Root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ensure directory exists.
    pub fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.root).map_err(SkillError::Io)?;
        Ok(())
    }

    /// Path for a skill id.
    pub fn path_for(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.toml"))
    }

    /// Load all skill documents from disk.
    pub fn load_all(&self) -> Result<Vec<SkillDocument>> {
        if !self.root.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(SkillError::Io)? {
            let entry = entry.map_err(SkillError::Io)?;
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            match self.load_file(&p) {
                Ok(doc) => out.push(doc),
                Err(e) => {
                    tracing::warn!(path = %p.display(), error = %e, "skip skill file");
                }
            }
        }
        out.sort_by(|a, b| a.skill.id.cmp(&b.skill.id));
        Ok(out)
    }

    /// Load one file.
    pub fn load_file(&self, path: &Path) -> Result<SkillDocument> {
        let text = fs::read_to_string(path).map_err(SkillError::Io)?;
        toml::from_str(&text).map_err(|e| SkillError::Parse(e.to_string()))
    }

    /// Save (overwrite) a skill document.
    pub fn save(&self, doc: &SkillDocument) -> Result<PathBuf> {
        validate_skill_id(&doc.skill.id)?;
        self.ensure_dir()?;
        let path = self.path_for(&doc.skill.id);
        let text = toml::to_string_pretty(doc).map_err(|e| SkillError::Parse(e.to_string()))?;
        fs::write(&path, text).map_err(SkillError::Io)?;
        info!(id = %doc.skill.id, path = %path.display(), "skill saved");
        Ok(path)
    }

    /// Delete a learned skill file.
    pub fn delete(&self, id: &str) -> Result<bool> {
        let path = self.path_for(id);
        if path.is_file() {
            fs::remove_file(&path).map_err(SkillError::Io)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Append a note / evolution log line.
    pub fn append_note(&self, id: &str, note: &str) -> Result<()> {
        let path = self.path_for(id);
        let mut doc = if path.is_file() {
            self.load_file(&path)?
        } else {
            return Err(SkillError::NotFound(id.into()));
        };
        if !doc.notes.is_empty() {
            doc.notes.push('\n');
        }
        doc.notes
            .push_str(&format!("[{}] {}", chrono_like_now(), note.trim()));
        self.save(&doc)?;
        Ok(())
    }
}

fn chrono_like_now() -> String {
    // Avoid chrono dep in skills crate: use system time seconds.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

fn validate_skill_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(SkillError::Invalid(format!(
            "skill id must be [A-Za-z0-9_-]{{1,64}}, got `{id}`"
        )));
    }
    Ok(())
}

/// Propose a skill from free-form agent fields.
pub fn propose_skill(
    id: impl Into<String>,
    description: impl Into<String>,
    tools: Vec<String>,
    tags: Vec<String>,
    notes: impl Into<String>,
) -> Result<SkillDocument> {
    let id = id.into();
    validate_skill_id(&id)?;
    Ok(SkillDocument {
        skill: Skill::new(id, description).tools(tools).tags(tags),
        origin: SkillOrigin::Learned,
        notes: notes.into(),
        score: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_load_roundtrip() {
        let dir = tempdir().unwrap();
        let store = SkillStore::new(dir.path());
        let doc = propose_skill(
            "my_skill",
            "does things",
            vec!["read_file".into()],
            vec!["custom".into()],
            "first draft",
        )
        .unwrap();
        store.save(&doc).unwrap();
        let all = store.load_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].skill.id, "my_skill");
        assert_eq!(all[0].origin, SkillOrigin::Learned);
    }
}
