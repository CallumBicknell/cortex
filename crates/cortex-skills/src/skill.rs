//! Skill definition.

use serde::{Deserialize, Serialize};

/// A capability pack the planner can activate (not a hard-coded "mode").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    /// Stable id (e.g. `"coding"`, `"solidity"`).
    pub id: String,
    /// Human description for catalogs and optional LLM selection.
    pub description: String,
    /// Tool names this skill uses.
    pub tools: Vec<String>,
    /// Prompt ids from the prompt catalog to inject when active.
    pub prompts: Vec<String>,
    /// Tags used for heuristic matching (languages, domains).
    pub tags: Vec<String>,
    /// Always included in the active set.
    pub always_on: bool,
}

impl Skill {
    /// Builder helper.
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            tools: Vec::new(),
            prompts: Vec::new(),
            tags: Vec::new(),
            always_on: false,
        }
    }

    /// Set tools.
    pub fn tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tools = tools.into_iter().map(Into::into).collect();
        self
    }

    /// Set prompts.
    pub fn prompts(mut self, prompts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.prompts = prompts.into_iter().map(Into::into).collect();
        self
    }

    /// Set tags.
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Mark always-on.
    pub fn always_on(mut self) -> Self {
        self.always_on = true;
        self
    }
}
