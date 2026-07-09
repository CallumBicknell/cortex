//! Chat / embedding request and response types.

use cortex_models::{Message, ToolCall, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Why the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Natural stop.
    Stop,
    /// Model requested tool calls.
    ToolCalls,
    /// Hit max tokens.
    Length,
    /// Content filter.
    ContentFilter,
    /// Other / unknown.
    Other,
}

/// Token usage reported by a provider (when available).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Prompt / input tokens.
    pub prompt_tokens: u32,
    /// Completion / output tokens.
    pub completion_tokens: u32,
    /// Total tokens.
    pub total_tokens: u32,
}

/// A chat completion request.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// Model id as understood by the provider (e.g. `"gpt-4.1"`, `"claude-sonnet-4-5"`).
    pub model: String,
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// Optional tool schemas.
    pub tools: Vec<ToolSpec>,
    /// Sampling temperature.
    pub temperature: Option<f32>,
    /// Max output tokens.
    pub max_tokens: Option<u32>,
    /// Optional cancellation token (defaults to never-cancel if None).
    pub cancel: Option<CancellationToken>,
    /// Per-request timeout override.
    pub timeout: Option<Duration>,
    /// Extra provider-specific fields (JSON object).
    pub extra: Value,
}

impl ChatRequest {
    /// Build a request with only model and messages.
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            cancel: None,
            timeout: None,
            extra: Value::Null,
        }
    }

    /// Attach tools.
    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }

    /// Attach cancellation.
    pub fn with_cancel(mut self, cancel: CancellationToken) -> Self {
        self.cancel = Some(cancel);
        self
    }

    /// Attach timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set max tokens.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

/// A chat completion response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Model that produced the response (as reported by the provider).
    pub model: String,
    /// Assistant message content.
    pub message: Message,
    /// Finish reason.
    pub finish_reason: FinishReason,
    /// Token usage when available.
    #[serde(default)]
    pub usage: Usage,
    /// Raw provider payload for debugging (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

impl ChatResponse {
    /// Convenience: tool calls from the assistant message.
    pub fn tool_calls(&self) -> &[ToolCall] {
        &self.message.tool_calls
    }

    /// True if the model requested tools.
    pub fn has_tool_calls(&self) -> bool {
        !self.message.tool_calls.is_empty() || self.finish_reason == FinishReason::ToolCalls
    }
}

/// Events yielded by streaming chat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Partial text delta.
    TextDelta {
        /// Incremental text.
        text: String,
    },
    /// A tool call fragment (id/name/arguments may arrive piecemeal).
    ToolCallDelta {
        /// Index among tool calls in this response.
        index: u32,
        /// Tool call id when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Tool name when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Arguments JSON fragment.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arguments_delta: Option<String>,
    },
    /// Stream finished with a complete response snapshot.
    Done {
        /// Final aggregated response.
        response: ChatResponse,
    },
    /// Non-fatal stream error from the provider.
    Error {
        /// Error message.
        message: String,
    },
}

/// Embedding request.
#[derive(Debug, Clone)]
pub struct EmbedRequest {
    /// Model id.
    pub model: String,
    /// Input texts.
    pub input: Vec<String>,
    /// Cancellation.
    pub cancel: Option<CancellationToken>,
}

/// Embedding response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbedResponse {
    /// Model used.
    pub model: String,
    /// One vector per input.
    pub embeddings: Vec<Vec<f32>>,
    /// Usage when available.
    #[serde(default)]
    pub usage: Usage,
}
