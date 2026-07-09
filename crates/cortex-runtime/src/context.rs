//! Context assembly for LLM requests.

use cortex_models::{Message, ToolSpec};

/// Builds the message list and tool schemas sent to the model each turn.
#[derive(Debug, Clone)]
pub struct ContextBuilder {
    /// System prompt prepended when non-empty.
    pub system_prompt: String,
    /// Maximum number of recent messages to include (0 = unlimited).
    pub max_history_messages: usize,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self {
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            max_history_messages: 0,
        }
    }
}

/// Default system prompt for the coding-oriented agent loop.
pub const DEFAULT_SYSTEM_PROMPT: &str = r#"You are Cortex, a careful coding agent running in a local workspace.

Rules:
- Prefer specialized tools over shell when possible.
- Keep changes minimal and correct.
- When the task is complete, respond with a concise final answer and no tool calls.
- If a tool fails, diagnose and retry with a different approach.
"#;

impl ContextBuilder {
    /// Create a builder with a custom system prompt.
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            max_history_messages: 0,
        }
    }

    /// Limit history to the last N messages (system prompt still applied separately).
    pub fn with_max_history(mut self, n: usize) -> Self {
        self.max_history_messages = n;
        self
    }

    /// Assemble messages for a chat request.
    pub fn build_messages(&self, history: &[Message]) -> Vec<Message> {
        let mut out = Vec::new();
        if !self.system_prompt.trim().is_empty() {
            out.push(Message::system(&self.system_prompt));
        }
        let slice = if self.max_history_messages == 0 || history.len() <= self.max_history_messages
        {
            history
        } else {
            let start = history.len() - self.max_history_messages;
            &history[start..]
        };
        out.extend(slice.iter().cloned());
        out
    }

    /// Pass-through tool specs (hook for future skill filtering).
    pub fn build_tools(&self, tools: Vec<ToolSpec>) -> Vec<ToolSpec> {
        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_models::Message;

    #[test]
    fn prepends_system_and_truncates() {
        let builder = ContextBuilder::new("sys").with_max_history(2);
        let history = vec![
            Message::user("1"),
            Message::assistant("2"),
            Message::user("3"),
        ];
        let msgs = builder.build_messages(&history);
        assert_eq!(msgs.len(), 3); // system + last 2
        assert_eq!(msgs[0].content, "sys");
        assert_eq!(msgs[1].content, "2");
        assert_eq!(msgs[2].content, "3");
    }
}
