//! Budgeted context builder for LLM requests.

use crate::history::compress_history;
use crate::token::estimate_tokens;
use cortex_models::{Message, Role, ToolSpec};
use cortex_workspace::RepoMap;

/// Default system prompt for the coding-oriented agent loop.
pub const DEFAULT_SYSTEM_PROMPT: &str = r#"You are Cortex, a careful coding agent running in a local workspace.

Rules:
- Prefer specialized tools over shell when possible.
- Keep changes minimal and correct.
- Use the workspace map to navigate the repository efficiently.
- When the task is complete, respond with a concise final answer and no tool calls.
- If a tool fails, diagnose and retry with a different approach.
"#;

/// Builds the message list and tool schemas sent to the model each turn.
#[derive(Debug, Clone)]
pub struct ContextBuilder {
    /// System prompt prepended when non-empty.
    pub system_prompt: String,
    /// Maximum number of recent history messages before compression (0 = no hard cap by count).
    pub max_history_messages: usize,
    /// Soft token budget for history + workspace sections (0 = unlimited).
    pub max_context_tokens: usize,
    /// How many recent messages to always retain when compressing.
    pub keep_recent_messages: usize,
    /// Optional prebuilt repo map section.
    pub repo_map_section: Option<String>,
    /// Whether to inject repo map into context.
    pub include_repo_map: bool,
    /// Optional skill prompt section (active skill guidance).
    pub skill_prompt_section: Option<String>,
    /// Rolling conversation summary (injected as system context).
    pub rolling_summary: Option<String>,
    /// Retrieved memory / RAG hits (injected as system context).
    pub retrieval_section: Option<String>,
    /// If set, only expose these tool names to the model.
    pub allowed_tools: Option<Vec<String>>,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self {
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            max_history_messages: 80,
            max_context_tokens: 12_000,
            keep_recent_messages: 24,
            repo_map_section: None,
            include_repo_map: true,
            skill_prompt_section: None,
            rolling_summary: None,
            retrieval_section: None,
            allowed_tools: None,
        }
    }
}

impl ContextBuilder {
    /// Create a builder with a custom system prompt.
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            ..Default::default()
        }
    }

    /// Limit history by message count (applied before token compression).
    pub fn with_max_history(mut self, n: usize) -> Self {
        self.max_history_messages = n;
        self
    }

    /// Set approximate token budget for non-system context.
    pub fn with_max_tokens(mut self, n: usize) -> Self {
        self.max_context_tokens = n;
        self
    }

    /// Attach a repo map (from [`RepoMap::to_prompt_section`]).
    pub fn with_repo_map(mut self, map: &RepoMap) -> Self {
        self.repo_map_section = Some(map.to_prompt_section());
        self
    }

    /// Attach a raw workspace section string.
    pub fn with_repo_map_section(mut self, section: impl Into<String>) -> Self {
        self.repo_map_section = Some(section.into());
        self
    }

    /// Disable repo map injection.
    pub fn without_repo_map(mut self) -> Self {
        self.include_repo_map = false;
        self.repo_map_section = None;
        self
    }

    /// Attach skill guidance text (markdown sections joined by caller).
    pub fn with_skill_prompts(mut self, section: impl Into<String>) -> Self {
        self.skill_prompt_section = Some(section.into());
        self
    }

    /// Attach a rolling conversation summary.
    pub fn with_rolling_summary(mut self, summary: impl Into<String>) -> Self {
        self.rolling_summary = Some(summary.into());
        self
    }

    /// Attach retrieval / memory search hits for the current turn.
    pub fn with_retrieval(mut self, section: impl Into<String>) -> Self {
        self.retrieval_section = Some(section.into());
        self
    }

    /// Restrict tools exposed to the model.
    pub fn with_allowed_tools(
        mut self,
        tools: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.allowed_tools = Some(tools.into_iter().map(Into::into).collect());
        self
    }

    /// Assemble messages for a chat request.
    pub fn build_messages(&self, history: &[Message]) -> Vec<Message> {
        let mut out = Vec::new();

        // System prompt
        if !self.system_prompt.trim().is_empty() {
            out.push(Message::system(&self.system_prompt));
        }

        // Skill guidance
        if let Some(section) = &self.skill_prompt_section {
            if !section.trim().is_empty() {
                out.push(Message::system(section));
            }
        }

        // Rolling summary of earlier turns
        if let Some(summary) = &self.rolling_summary {
            if !summary.trim().is_empty() {
                out.push(Message::system(format!(
                    "[session summary]\n{}",
                    summary.trim()
                )));
            }
        }

        // Retrieved memory snippets
        if let Some(section) = &self.retrieval_section {
            if !section.trim().is_empty() {
                out.push(Message::system(section));
            }
        }

        // Workspace / repo map as an additional system message
        if self.include_repo_map {
            if let Some(section) = &self.repo_map_section {
                let mut map_msg = section.clone();
                if self.max_context_tokens > 0 {
                    // Cap repo map to ~25% of budget.
                    let map_budget = (self.max_context_tokens / 4).max(500);
                    while estimate_tokens(&map_msg) > map_budget && map_msg.len() > 200 {
                        let keep = map_msg.len() * 3 / 4;
                        map_msg.truncate(keep);
                        map_msg.push_str("\n…[repo map truncated]");
                    }
                }
                out.push(Message::system(map_msg));
            }
        }

        // History window by count
        let history_slice: &[Message] =
            if self.max_history_messages == 0 || history.len() <= self.max_history_messages {
                history
            } else {
                let start = history.len() - self.max_history_messages;
                &history[start..]
            };

        // Token compression for history
        let history_budget = if self.max_context_tokens == 0 {
            0
        } else {
            let used: usize = out.iter().map(|m| estimate_tokens(&m.content)).sum();
            self.max_context_tokens.saturating_sub(used)
        };

        let compressed = if history_budget == 0 {
            history_slice.to_vec()
        } else {
            compress_history(history_slice, self.keep_recent_messages, history_budget)
        };

        // Avoid duplicating system role notes at the start if history already has system.
        for msg in compressed {
            if msg.role == Role::System && out.iter().any(|m| m.role == Role::System) {
                // Allow compression notes; skip empty duplicate system shells.
                if msg.content.starts_with("[context]") {
                    out.push(msg);
                }
                continue;
            }
            out.push(msg);
        }

        out
    }

    /// Filter tool specs when [`Self::allowed_tools`] is set.
    pub fn build_tools(&self, tools: Vec<ToolSpec>) -> Vec<ToolSpec> {
        match &self.allowed_tools {
            None => tools,
            Some(allow) => tools
                .into_iter()
                .filter(|t| allow.iter().any(|a| a == &t.name))
                .collect(),
        }
    }

    /// Estimated tokens for a built message list.
    pub fn estimate_built_tokens(&self, history: &[Message]) -> usize {
        self.build_messages(history)
            .iter()
            .map(|m| estimate_tokens(&m.content))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_models::Message;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn prepends_system_and_truncates_count() {
        let builder = ContextBuilder::new("sys")
            .with_max_history(2)
            .without_repo_map()
            .with_max_tokens(0);
        let history = vec![
            Message::user("1"),
            Message::assistant("2"),
            Message::user("3"),
        ];
        let msgs = builder.build_messages(&history);
        assert_eq!(msgs[0].content, "sys");
        assert!(msgs.iter().any(|m| m.content == "3"));
        assert!(!msgs.iter().any(|m| m.content == "1"));
    }

    #[test]
    fn injects_repo_map() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        let map = cortex_workspace::RepoMap::build(dir.path()).unwrap();
        let builder = ContextBuilder::new("sys").with_repo_map(&map);
        let msgs = builder.build_messages(&[Message::user("hi")]);
        assert!(msgs.len() >= 3);
        assert!(msgs[1].content.contains("Workspace"));
        assert_eq!(msgs.last().unwrap().content, "hi");
    }

    #[test]
    fn compresses_long_history() {
        let mut history = Vec::new();
        for i in 0..50 {
            history.push(Message::user(format!(
                "message number {i} {}",
                "pad ".repeat(40)
            )));
        }
        let builder = ContextBuilder::new("sys")
            .without_repo_map()
            .with_max_tokens(200)
            .with_max_history(50);
        let msgs = builder.build_messages(&history);
        let tokens: usize = msgs.iter().map(|m| estimate_tokens(&m.content)).sum();
        // Soft budget — should be in the ballpark (note may add a little).
        assert!(tokens < 800, "tokens={tokens}");
        assert!(msgs.iter().any(|m| m.content.contains("message number 49")));
    }
}
