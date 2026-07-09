//! Anthropic Messages API adapter.

use crate::error::{ProviderError, Result};
use crate::provider::Provider;
use crate::retry::RetryPolicy;
use crate::types::{ChatRequest, ChatResponse, FinishReason, Usage};
use async_trait::async_trait;
use cortex_common::ToolCallId;
use cortex_models::{Message, Role, ToolCall};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

/// Anthropic provider configuration.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    /// Provider id.
    pub id: String,
    /// API base URL.
    pub base_url: String,
    /// API key.
    pub api_key: String,
    /// Anthropic API version header.
    pub api_version: String,
    /// Default timeout.
    pub timeout: Duration,
    /// Retry policy.
    pub retry: RetryPolicy,
}

impl AnthropicConfig {
    /// Default Anthropic cloud config.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            id: "anthropic".into(),
            base_url: "https://api.anthropic.com".into(),
            api_key: api_key.into(),
            api_version: "2023-06-01".into(),
            timeout: Duration::from_secs(120),
            retry: RetryPolicy::default(),
        }
    }
}

/// Anthropic Messages API provider.
pub struct AnthropicProvider {
    config: AnthropicConfig,
    client: Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    pub fn new(config: AnthropicConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&config.api_key)
                .map_err(|e| ProviderError::Config(format!("invalid api key: {e}")))?,
        );
        headers.insert(
            "anthropic-version",
            HeaderValue::from_str(&config.api_version)
                .map_err(|e| ProviderError::Config(format!("invalid api version: {e}")))?,
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(config.timeout)
            .build()
            .map_err(|e| ProviderError::Transport(e.to_string()))?;

        Ok(Self { config, client })
    }

    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'))
    }

    fn build_body(req: &ChatRequest) -> Result<Value> {
        let mut system = String::new();
        let mut messages = Vec::new();

        for msg in &req.messages {
            match msg.role {
                Role::System => {
                    if !system.is_empty() {
                        system.push('\n');
                    }
                    system.push_str(&msg.content);
                }
                Role::User => {
                    messages.push(json!({
                        "role": "user",
                        "content": msg.content,
                    }));
                }
                Role::Assistant => {
                    let mut content_blocks = Vec::new();
                    if !msg.content.is_empty() {
                        content_blocks.push(json!({
                            "type": "text",
                            "text": msg.content,
                        }));
                    }
                    for tc in &msg.tool_calls {
                        content_blocks.push(json!({
                            "type": "tool_use",
                            "id": tc.id.to_string(),
                            "name": tc.name,
                            "input": tc.arguments,
                        }));
                    }
                    if content_blocks.is_empty() {
                        content_blocks.push(json!({"type": "text", "text": ""}));
                    }
                    messages.push(json!({
                        "role": "assistant",
                        "content": content_blocks,
                    }));
                }
                Role::Tool => {
                    let tool_use_id =
                        msg.tool_call_id.map(|id| id.to_string()).ok_or_else(|| {
                            ProviderError::InvalidRequest(
                                "tool message missing tool_call_id".into(),
                            )
                        })?;
                    // Anthropic expects tool results as user messages with tool_result blocks.
                    messages.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": msg.content,
                        }],
                    }));
                }
            }
        }

        if messages.is_empty() {
            return Err(ProviderError::InvalidRequest(
                "anthropic requires at least one non-system message".into(),
            ));
        }

        let mut body = json!({
            "model": req.model,
            "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(4096),
        });
        if !system.is_empty() {
            body["system"] = json!(system);
        }
        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(
                req.tools
                    .iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description,
                            "input_schema": t.parameters,
                        })
                    })
                    .collect(),
            );
        }
        Ok(body)
    }

    async fn chat_once(&self, req: &ChatRequest) -> Result<ChatResponse> {
        if let Some(c) = &req.cancel {
            if c.is_cancelled() {
                return Err(ProviderError::cancelled("chat aborted"));
            }
        }
        let body = Self::build_body(req)?;
        let timeout = req.timeout.unwrap_or(self.config.timeout);
        let send = self
            .client
            .post(self.messages_url())
            .timeout(timeout)
            .json(&body)
            .send();

        let response = if let Some(c) = &req.cancel {
            tokio::select! {
                _ = c.cancelled() => return Err(ProviderError::cancelled("chat aborted")),
                res = send => res,
            }
        } else {
            send.await
        }
        .map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout(timeout)
            } else {
                ProviderError::Transport(e.to_string())
            }
        })?;

        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        if !status.is_success() {
            let text = String::from_utf8_lossy(&bytes).to_string();
            return match status.as_u16() {
                401 | 403 => Err(ProviderError::Auth(text)),
                429 => Err(ProviderError::RateLimited(text)),
                code => Err(ProviderError::Api {
                    status: code,
                    message: text,
                }),
            };
        }

        let raw: Value =
            serde_json::from_slice(&bytes).map_err(|e| ProviderError::Parse(e.to_string()))?;
        parse_anthropic_response(&raw)
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn id(&self) -> &str {
        &self.config.id
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        let cancel = req.cancel.clone();
        self.config
            .retry
            .run(cancel.as_ref(), || self.chat_once(&req))
            .await
    }
}

fn parse_anthropic_response(raw: &Value) -> Result<ChatResponse> {
    let model = raw
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown")
        .to_string();

    let mut text = String::new();
    let mut tool_calls = Vec::new();
    if let Some(blocks) = raw.get("content").and_then(|c| c.as_array()) {
        for block in blocks {
            let ty = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match ty {
                "text" => {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        text.push_str(t);
                    }
                }
                "tool_use" => {
                    let id_str = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                    let id = Uuid::parse_str(id_str)
                        .map(ToolCallId::from_uuid)
                        .unwrap_or_else(|_| ToolCallId::new());
                    let name = block
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = block.get("input").cloned().unwrap_or(json!({}));
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments: input,
                    });
                }
                _ => {}
            }
        }
    }

    let stop = raw
        .get("stop_reason")
        .and_then(|s| s.as_str())
        .unwrap_or("end_turn");
    let finish_reason = match stop {
        "end_turn" | "stop_sequence" => {
            if tool_calls.is_empty() {
                FinishReason::Stop
            } else {
                FinishReason::ToolCalls
            }
        }
        "tool_use" => FinishReason::ToolCalls,
        "max_tokens" => FinishReason::Length,
        _ => FinishReason::Other,
    };

    let message = if tool_calls.is_empty() {
        Message::assistant(text)
    } else {
        Message::assistant_with_tools(text, tool_calls)
    };

    let usage = Usage {
        prompt_tokens: raw
            .pointer("/usage/input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        completion_tokens: raw
            .pointer("/usage/output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        total_tokens: 0,
    };
    let usage = Usage {
        total_tokens: usage.prompt_tokens + usage.completion_tokens,
        ..usage
    };

    Ok(ChatResponse {
        model,
        message,
        finish_reason,
        usage,
        raw: Some(raw.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ChatRequest;
    use cortex_models::Message;

    #[test]
    fn build_body_extracts_system() {
        let req = ChatRequest::new(
            "claude-sonnet-4-5",
            vec![Message::system("be helpful"), Message::user("hi")],
        );
        let body = AnthropicProvider::build_body(&req).unwrap();
        assert_eq!(body["system"], json!("be helpful"));
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    }
}
