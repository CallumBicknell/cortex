//! Chat messages exchanged with models and stored in sessions.

use crate::tool::ToolCall;
use chrono::{DateTime, Utc};
use cortex_common::{MessageId, ToolCallId};
use serde::{Deserialize, Serialize};

/// Role of a message author.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// System instructions.
    System,
    /// End-user input.
    User,
    /// Model assistant output.
    Assistant,
    /// Result of a tool invocation (paired with a tool_call_id).
    Tool,
}

/// A single message in a conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Stable message id.
    pub id: MessageId,
    /// Author role.
    pub role: Role,
    /// Primary text content (may be empty when only tool_calls are present).
    pub content: String,
    /// Tool calls requested by the assistant (assistant messages only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// For tool-role messages: which tool call this result answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<ToolCallId>,
    /// Optional tool name for tool-role messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl Message {
    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::System,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
            created_at: Utc::now(),
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
            created_at: Utc::now(),
        }
    }

    /// Create an assistant text message (no tool calls).
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::Assistant,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
            created_at: Utc::now(),
        }
    }

    /// Create an assistant message that requests tool calls.
    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::Assistant,
            content: content.into(),
            tool_calls,
            tool_call_id: None,
            name: None,
            created_at: Utc::now(),
        }
    }

    /// Create a tool-result message.
    pub fn tool_result(
        tool_call_id: ToolCallId,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id),
            name: Some(name.into()),
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::ToolCall;
    use serde_json::json;

    #[test]
    fn message_serde_roundtrip() {
        let msg = Message::assistant_with_tools(
            "calling tools",
            vec![ToolCall {
                id: ToolCallId::new(),
                name: "read_file".into(),
                arguments: json!({"path": "README.md"}),
            }],
        );
        let raw = serde_json::to_string_pretty(&msg).unwrap();
        let back: Message = serde_json::from_str(&raw).unwrap();
        assert_eq!(msg, back);
        assert_eq!(back.role, Role::Assistant);
        assert_eq!(back.tool_calls.len(), 1);
    }
}
