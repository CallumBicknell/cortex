//! Tools for listing / proposing / saving evolving skills.

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use cortex_skills::{propose_skill, SkillDocument, SkillOrigin, SkillStore};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

/// Shared skill store handle.
#[derive(Clone)]
pub struct SkillStoreHandle {
    store: Arc<SkillStore>,
}

impl SkillStoreHandle {
    /// Create handle.
    pub fn new(store: SkillStore) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// Underlying store.
    pub fn store(&self) -> &SkillStore {
        &self.store
    }
}

/// List learned + file-backed skills.
pub struct SkillListTool {
    handle: SkillStoreHandle,
}

impl SkillListTool {
    /// Create.
    pub fn new(handle: SkillStoreHandle) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl Tool for SkillListTool {
    fn name(&self) -> &str {
        "skill_list"
    }

    fn description(&self) -> &str {
        "List learned/promoted skills stored under .cortex/skills/."
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, ctx: &ToolContext, _input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let docs = self
            .handle
            .store
            .load_all()
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        if docs.is_empty() {
            return Ok("no learned skills yet (use skill_save to create one)".into());
        }
        let mut out = String::from("Learned skills:\n");
        for d in docs {
            out.push_str(&format!(
                "- {} [{}] score={} tools={:?} tags={:?}\n  {}\n",
                d.skill.id,
                format!("{:?}", d.origin).to_ascii_lowercase(),
                d.score,
                d.skill.tools,
                d.skill.tags,
                d.skill.description
            ));
        }
        Ok(out)
    }
}

/// Propose and save a new skill pack (self-evolution).
pub struct SkillSaveTool {
    handle: SkillStoreHandle,
}

impl SkillSaveTool {
    /// Create.
    pub fn new(handle: SkillStoreHandle) -> Self {
        Self { handle }
    }
}

#[derive(Deserialize)]
struct SaveInput {
    id: String,
    description: String,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    notes: String,
    /// If true, mark origin as promoted (trusted).
    #[serde(default)]
    promote: bool,
}

#[async_trait]
impl Tool for SkillSaveTool {
    fn name(&self) -> &str {
        "skill_save"
    }

    fn description(&self) -> &str {
        "Save or update a learned skill pack under .cortex/skills/ so future runs can activate it. \
         Use after discovering a repeatable workflow (tools + tags + description)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "snake-case skill id" },
                "description": { "type": "string" },
                "tools": { "type": "array", "items": { "type": "string" } },
                "tags": { "type": "array", "items": { "type": "string" } },
                "notes": { "type": "string", "description": "why this skill exists / evolution notes" },
                "promote": { "type": "boolean", "default": false }
            },
            "required": ["id", "description"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: SaveInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid skill_save args: {e}")))?;
        let mut doc = propose_skill(args.id, args.description, args.tools, args.tags, args.notes)
            .map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        if args.promote {
            doc.origin = SkillOrigin::Promoted;
            doc.score = doc.score.saturating_add(1);
        }
        // Merge score if already exists.
        if let Ok(existing) = self
            .handle
            .store
            .load_file(&self.handle.store.path_for(&doc.skill.id))
        {
            doc.score = existing.score.max(doc.score);
            if !existing.notes.is_empty() && doc.notes != existing.notes {
                doc.notes = format!("{}\n---\n{}", existing.notes, doc.notes);
            }
        }
        let path = self
            .handle
            .store
            .save(&doc)
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(format!(
            "saved skill `{}` ({:?}) → {}",
            doc.skill.id,
            doc.origin,
            path.display()
        ))
    }
}

/// Reinforce / promote an existing skill.
pub struct SkillPromoteTool {
    handle: SkillStoreHandle,
}

impl SkillPromoteTool {
    /// Create.
    pub fn new(handle: SkillStoreHandle) -> Self {
        Self { handle }
    }
}

#[derive(Deserialize)]
struct PromoteInput {
    id: String,
    #[serde(default)]
    note: String,
}

#[async_trait]
impl Tool for SkillPromoteTool {
    fn name(&self) -> &str {
        "skill_promote"
    }

    fn description(&self) -> &str {
        "Mark a learned skill as promoted (trusted) and increment its score."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "note": { "type": "string" }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: PromoteInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid skill_promote args: {e}")))?;
        let path = self.handle.store.path_for(&args.id);
        let mut doc: SkillDocument = self
            .handle
            .store
            .load_file(&path)
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        doc.origin = SkillOrigin::Promoted;
        doc.score = doc.score.saturating_add(1);
        if !args.note.trim().is_empty() {
            if !doc.notes.is_empty() {
                doc.notes.push('\n');
            }
            doc.notes.push_str(args.note.trim());
        }
        self.handle
            .store
            .save(&doc)
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(format!(
            "promoted skill `{}` (score={})",
            doc.skill.id, doc.score
        ))
    }
}

/// Register skill evolution tools.
pub fn register_skill_tools(
    registry: &mut crate::registry::ToolRegistry,
    handle: SkillStoreHandle,
) {
    registry.register_or_replace(Arc::new(SkillListTool::new(handle.clone())));
    registry.register_or_replace(Arc::new(SkillSaveTool::new(handle.clone())));
    registry.register_or_replace(Arc::new(SkillPromoteTool::new(handle)));
}
