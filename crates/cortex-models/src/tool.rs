//! Tool specifications, calls, and results.

use chrono::{DateTime, Utc};
use cortex_common::ToolCallId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// JSON-schema style tool description exposed to the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Unique tool name (e.g. `"read_file"`).
    pub name: String,
    /// Human-readable description for the model.
    pub description: String,
    /// JSON Schema for parameters (`type: object` expected).
    pub parameters: Value,
}

impl ToolSpec {
    /// Create a tool spec with an empty object parameter schema.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    /// Set the parameters schema (builder style).
    pub fn with_parameters(mut self, parameters: Value) -> Self {
        self.parameters = parameters;
        self
    }
}

/// A tool invocation requested by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Provider/runtime id for this call.
    pub id: ToolCallId,
    /// Tool name.
    pub name: String,
    /// Arguments as JSON (object).
    pub arguments: Value,
}

impl ToolCall {
    /// Create a new tool call with a fresh id.
    pub fn new(name: impl Into<String>, arguments: Value) -> Self {
        Self {
            id: ToolCallId::new(),
            name: name.into(),
            arguments,
        }
    }
}

/// Outcome of executing a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    /// Matching tool call id.
    pub tool_call_id: ToolCallId,
    /// Tool name (redundant but convenient for logs).
    pub name: String,
    /// Primary textual or JSON output.
    pub output: String,
    /// Whether the tool reported failure.
    pub is_error: bool,
    /// Wall-clock duration of execution, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// When the result was produced.
    pub created_at: DateTime<Utc>,
}

impl ToolResult {
    /// Successful tool result.
    pub fn success(
        tool_call_id: ToolCallId,
        name: impl Into<String>,
        output: impl Into<String>,
    ) -> Self {
        Self {
            tool_call_id,
            name: name.into(),
            output: output.into(),
            is_error: false,
            duration_ms: None,
            created_at: Utc::now(),
        }
    }

    /// Failed tool result.
    pub fn error(
        tool_call_id: ToolCallId,
        name: impl Into<String>,
        output: impl Into<String>,
    ) -> Self {
        Self {
            tool_call_id,
            name: name.into(),
            output: output.into(),
            is_error: true,
            duration_ms: None,
            created_at: Utc::now(),
        }
    }

    /// Attach a duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = Some(duration.as_millis() as u64);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_result_roundtrip() {
        let id = ToolCallId::new();
        let result =
            ToolResult::success(id, "shell", "ok").with_duration(Duration::from_millis(12));
        let raw = serde_json::to_value(&result).unwrap();
        let back: ToolResult = serde_json::from_value(raw).unwrap();
        assert_eq!(result, back);
        assert!(!back.is_error);
        assert_eq!(back.duration_ms, Some(12));
    }

    #[test]
    fn tool_spec_default_params() {
        let spec = ToolSpec::new("noop", "does nothing");
        assert_eq!(spec.parameters["type"], json!("object"));
    }
}
