//! OpenAI-compatible Chat Completions client.
//!
//! Works with OpenAI, OpenRouter, Ollama (`/v1`), LM Studio, and similar servers.

use crate::error::{ProviderError, Result};
use crate::provider::{ChatStream, Provider};
use crate::retry::RetryPolicy;
use crate::types::{
    ChatRequest, ChatResponse, EmbedRequest, EmbedResponse, FinishReason, StreamEvent, Usage,
};
use async_trait::async_trait;
use cortex_common::ToolCallId;
use cortex_models::{Message, Role, ToolCall};
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

/// Configuration for an OpenAI-compatible endpoint.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleConfig {
    /// Provider id used in the registry (e.g. `"openai"`, `"ollama"`).
    pub id: String,
    /// Base URL without trailing slash (e.g. `https://api.openai.com/v1`).
    pub base_url: String,
    /// API key (may be empty for local servers).
    pub api_key: Option<String>,
    /// Default request timeout.
    pub timeout: Duration,
    /// Retry policy.
    pub retry: RetryPolicy,
    /// Optional extra headers (name, value).
    pub extra_headers: Vec<(String, String)>,
}

impl OpenAiCompatibleConfig {
    /// OpenAI defaults.
    pub fn openai(api_key: impl Into<String>) -> Self {
        Self {
            id: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: Some(api_key.into()),
            timeout: Duration::from_secs(120),
            retry: RetryPolicy::default(),
            extra_headers: Vec::new(),
        }
    }

    /// Ollama local defaults.
    pub fn ollama() -> Self {
        Self {
            id: "ollama".into(),
            base_url: "http://127.0.0.1:11434/v1".into(),
            api_key: None,
            timeout: Duration::from_secs(300),
            retry: RetryPolicy::default(),
            extra_headers: Vec::new(),
        }
    }

    /// LM Studio local defaults.
    pub fn lmstudio() -> Self {
        Self {
            id: "lmstudio".into(),
            base_url: "http://127.0.0.1:1234/v1".into(),
            api_key: None,
            timeout: Duration::from_secs(300),
            retry: RetryPolicy::default(),
            extra_headers: Vec::new(),
        }
    }

    /// OpenRouter defaults.
    pub fn openrouter(api_key: impl Into<String>) -> Self {
        Self {
            id: "openrouter".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            api_key: Some(api_key.into()),
            timeout: Duration::from_secs(120),
            retry: RetryPolicy::default(),
            extra_headers: Vec::new(),
        }
    }
}

/// HTTP client for OpenAI-compatible chat completions.
pub struct OpenAiCompatibleProvider {
    config: OpenAiCompatibleConfig,
    client: Client,
}

impl OpenAiCompatibleProvider {
    /// Create a provider from config.
    pub fn new(config: OpenAiCompatibleConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(key) = &config.api_key {
            if !key.is_empty() {
                let value = format!("Bearer {key}");
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&value).map_err(|e| {
                        ProviderError::Config(format!("invalid API key header: {e}"))
                    })?,
                );
            }
        }
        for (name, value) in &config.extra_headers {
            let header_name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|e| ProviderError::Config(format!("invalid header name {name}: {e}")))?;
            let header_value = HeaderValue::from_str(value).map_err(|e| {
                ProviderError::Config(format!("invalid header value for {name}: {e}"))
            })?;
            headers.insert(header_name, header_value);
        }

        let client = Client::builder()
            .default_headers(headers)
            .timeout(config.timeout)
            .build()
            .map_err(|e| ProviderError::Transport(e.to_string()))?;

        Ok(Self { config, client })
    }

    fn chat_url(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        )
    }

    fn embeddings_url(&self) -> String {
        format!("{}/embeddings", self.config.base_url.trim_end_matches('/'))
    }

    fn build_body(req: &ChatRequest, stream: bool) -> Result<Value> {
        let messages: Vec<Value> = req
            .messages
            .iter()
            .map(message_to_openai)
            .collect::<std::result::Result<_, _>>()?;

        let mut body = json!({
            "model": req.model,
            "messages": messages,
            "stream": stream,
        });

        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(m) = req.max_tokens {
            body["max_tokens"] = json!(m);
        }
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(
                req.tools
                    .iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.parameters,
                            }
                        })
                    })
                    .collect(),
            );
        }
        if let Value::Object(extra) = &req.extra {
            if let Some(obj) = body.as_object_mut() {
                for (k, v) in extra {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        Ok(body)
    }

    async fn chat_once(&self, req: &ChatRequest) -> Result<ChatResponse> {
        if let Some(c) = &req.cancel {
            if c.is_cancelled() {
                return Err(ProviderError::cancelled("chat aborted before request"));
            }
        }

        let body = Self::build_body(req, false)?;
        let timeout = req.timeout.unwrap_or(self.config.timeout);
        let request = self
            .client
            .post(self.chat_url())
            .timeout(timeout)
            .json(&body);

        let send = request.send();
        let response = if let Some(c) = &req.cancel {
            tokio::select! {
                _ = c.cancelled() => return Err(ProviderError::cancelled("chat aborted")),
                res = send => res,
            }
        } else {
            send.await
        }
        .map_err(|e| map_reqwest_error(e, timeout))?;

        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;

        if !status.is_success() {
            return Err(map_http_error(status.as_u16(), &bytes));
        }

        let raw: Value = serde_json::from_slice(&bytes)
            .map_err(|e| ProviderError::Parse(format!("invalid JSON: {e}")))?;
        parse_chat_completion(&raw)
    }
}

#[async_trait]
impl Provider for OpenAiCompatibleProvider {
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

    async fn stream(&self, req: ChatRequest) -> Result<ChatStream> {
        if let Some(c) = &req.cancel {
            if c.is_cancelled() {
                return Err(ProviderError::cancelled("stream aborted before request"));
            }
        }

        let body = Self::build_body(&req, true)?;
        let timeout = req.timeout.unwrap_or(self.config.timeout);
        let response = self
            .client
            .post(self.chat_url())
            .timeout(timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, timeout))?;

        let status = response.status();
        if !status.is_success() {
            let bytes = response
                .bytes()
                .await
                .map_err(|e| ProviderError::Transport(e.to_string()))?;
            return Err(map_http_error(status.as_u16(), &bytes));
        }

        let cancel = req.cancel.clone();
        let model = req.model.clone();
        let byte_stream = response.bytes_stream();

        let stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut content = String::new();
            let mut tool_acc: Vec<ToolCallAcc> = Vec::new();
            let mut finish_reason = FinishReason::Stop;
            let mut usage = Usage::default();
            let mut stream = byte_stream;

            loop {
                if let Some(c) = &cancel {
                    if c.is_cancelled() {
                        yield Err(ProviderError::cancelled("stream aborted"));
                        return;
                    }
                }

                let next = stream.next().await;
                let chunk = match next {
                    Some(Ok(b)) => b,
                    Some(Err(e)) => {
                        yield Err(ProviderError::Transport(e.to_string()));
                        return;
                    }
                    None => break,
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim_end_matches('\r').to_string();
                    buffer = buffer[pos + 1..].to_string();
                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }
                    let data = match line.strip_prefix("data:") {
                        Some(d) => d.trim(),
                        None => continue,
                    };
                    if data == "[DONE]" {
                        let response = finalize_stream(&model, &content, &tool_acc, finish_reason, usage);
                        yield Ok(StreamEvent::Done { response });
                        return;
                    }
                    let parsed: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(e) => {
                            yield Err(ProviderError::Parse(format!("SSE JSON: {e}")));
                            return;
                        }
                    };
                    if let Some(u) = parsed.get("usage") {
                        usage = parse_usage(u);
                    }
                    let choices = parsed.get("choices").and_then(|c| c.as_array());
                    let Some(choices) = choices else { continue };
                    let Some(choice) = choices.first() else { continue };
                    if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                        finish_reason = map_finish_reason(fr);
                    }
                    let delta = choice.get("delta").cloned().unwrap_or(Value::Null);
                    if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
                        if !text.is_empty() {
                            content.push_str(text);
                            yield Ok(StreamEvent::TextDelta { text: text.to_string() });
                        }
                    }
                    if let Some(tcs) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                        for tc in tcs {
                            let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                            while tool_acc.len() <= index {
                                tool_acc.push(ToolCallAcc::default());
                            }
                            let acc = &mut tool_acc[index];
                            if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                                acc.id = Some(id.to_string());
                                yield Ok(StreamEvent::ToolCallDelta {
                                    index: index as u32,
                                    id: Some(id.to_string()),
                                    name: None,
                                    arguments_delta: None,
                                });
                            }
                            if let Some(name) = tc
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                            {
                                acc.name = Some(name.to_string());
                                yield Ok(StreamEvent::ToolCallDelta {
                                    index: index as u32,
                                    id: None,
                                    name: Some(name.to_string()),
                                    arguments_delta: None,
                                });
                            }
                            if let Some(args) = tc
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str())
                            {
                                acc.arguments.push_str(args);
                                yield Ok(StreamEvent::ToolCallDelta {
                                    index: index as u32,
                                    id: None,
                                    name: None,
                                    arguments_delta: Some(args.to_string()),
                                });
                            }
                        }
                    }
                }
            }

            // Stream ended without [DONE]
            let response = finalize_stream(&model, &content, &tool_acc, finish_reason, usage);
            yield Ok(StreamEvent::Done { response });
        };

        Ok(Box::pin(stream))
    }

    async fn embeddings(&self, req: EmbedRequest) -> Result<EmbedResponse> {
        if let Some(c) = &req.cancel {
            if c.is_cancelled() {
                return Err(ProviderError::cancelled("embeddings aborted"));
            }
        }
        let body = json!({
            "model": req.model,
            "input": req.input,
        });
        let response = self
            .client
            .post(self.embeddings_url())
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(map_http_error(status.as_u16(), &bytes));
        }
        let raw: Value =
            serde_json::from_slice(&bytes).map_err(|e| ProviderError::Parse(e.to_string()))?;
        parse_embeddings(&raw, &req.model)
    }
}

#[derive(Default)]
struct ToolCallAcc {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

fn finalize_stream(
    model: &str,
    content: &str,
    tool_acc: &[ToolCallAcc],
    finish_reason: FinishReason,
    usage: Usage,
) -> ChatResponse {
    let tool_calls: Vec<ToolCall> = tool_acc
        .iter()
        .filter_map(|acc| {
            let name = acc.name.clone()?;
            let id = acc
                .id
                .as_deref()
                .and_then(|s| Uuid::parse_str(s).ok())
                .map(ToolCallId::from_uuid)
                .unwrap_or_else(ToolCallId::new);
            let arguments = serde_json::from_str(&acc.arguments)
                .unwrap_or_else(|_| json!({ "raw": acc.arguments }));
            Some(ToolCall {
                id,
                name,
                arguments,
            })
        })
        .collect();

    let message = if tool_calls.is_empty() {
        Message::assistant(content)
    } else {
        Message::assistant_with_tools(content, tool_calls)
    };

    let finish_reason = if !message.tool_calls.is_empty() {
        FinishReason::ToolCalls
    } else {
        finish_reason
    };

    ChatResponse {
        model: model.to_string(),
        message,
        finish_reason,
        usage,
        raw: None,
    }
}

fn message_to_openai(msg: &Message) -> Result<Value> {
    match msg.role {
        Role::System => Ok(json!({"role": "system", "content": msg.content})),
        Role::User => Ok(json!({"role": "user", "content": msg.content})),
        Role::Assistant => {
            let mut m = json!({"role": "assistant", "content": msg.content});
            if !msg.tool_calls.is_empty() {
                m["tool_calls"] = Value::Array(
                    msg.tool_calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "id": tc.id.to_string(),
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.arguments.to_string(),
                                }
                            })
                        })
                        .collect(),
                );
            }
            Ok(m)
        }
        Role::Tool => {
            let tool_call_id = msg.tool_call_id.map(|id| id.to_string()).ok_or_else(|| {
                ProviderError::InvalidRequest("tool message missing tool_call_id".into())
            })?;
            Ok(json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": msg.content,
            }))
        }
    }
}

fn parse_chat_completion(raw: &Value) -> Result<ChatResponse> {
    let model = raw
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown")
        .to_string();
    let choice = raw
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| ProviderError::Parse("missing choices".into()))?;
    let message_obj = choice
        .get("message")
        .ok_or_else(|| ProviderError::Parse("missing message".into()))?;
    let content = message_obj
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let tool_calls = parse_tool_calls(message_obj.get("tool_calls"))?;
    let finish = choice
        .get("finish_reason")
        .and_then(|f| f.as_str())
        .map(map_finish_reason)
        .unwrap_or(if tool_calls.is_empty() {
            FinishReason::Stop
        } else {
            FinishReason::ToolCalls
        });
    let message = if tool_calls.is_empty() {
        Message::assistant(content)
    } else {
        Message::assistant_with_tools(content, tool_calls)
    };
    Ok(ChatResponse {
        model,
        message,
        finish_reason: finish,
        usage: raw.get("usage").map(parse_usage).unwrap_or_default(),
        raw: Some(raw.clone()),
    })
}

fn parse_tool_calls(value: Option<&Value>) -> Result<Vec<ToolCall>> {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for tc in arr {
        let id_str = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
        let id = Uuid::parse_str(id_str)
            .map(ToolCallId::from_uuid)
            .unwrap_or_else(|_| ToolCallId::new());
        let name = tc
            .get("function")
            .and_then(|f| f.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        let args_str = tc
            .get("function")
            .and_then(|f| f.get("arguments"))
            .and_then(|a| a.as_str())
            .unwrap_or("{}");
        let arguments =
            serde_json::from_str(args_str).unwrap_or_else(|_| json!({ "raw": args_str }));
        out.push(ToolCall {
            id,
            name,
            arguments,
        });
    }
    Ok(out)
}

fn parse_embeddings(raw: &Value, fallback_model: &str) -> Result<EmbedResponse> {
    let model = raw
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or(fallback_model)
        .to_string();
    let data = raw
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| ProviderError::Parse("missing embeddings data".into()))?;
    let mut embeddings = Vec::new();
    for item in data {
        let vec = item
            .get("embedding")
            .and_then(|e| e.as_array())
            .ok_or_else(|| ProviderError::Parse("missing embedding vector".into()))?
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();
        embeddings.push(vec);
    }
    Ok(EmbedResponse {
        model,
        embeddings,
        usage: raw.get("usage").map(parse_usage).unwrap_or_default(),
    })
}

fn parse_usage(v: &Value) -> Usage {
    Usage {
        prompt_tokens: v.get("prompt_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        completion_tokens: v
            .get("completion_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32,
        total_tokens: v.get("total_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
    }
}

fn map_finish_reason(s: &str) -> FinishReason {
    match s {
        "stop" => FinishReason::Stop,
        "tool_calls" | "function_call" => FinishReason::ToolCalls,
        "length" => FinishReason::Length,
        "content_filter" => FinishReason::ContentFilter,
        _ => FinishReason::Other,
    }
}

fn map_http_error(status: u16, body: &[u8]) -> ProviderError {
    let text = String::from_utf8_lossy(body).to_string();
    if status == 401 || status == 403 {
        return ProviderError::Auth(text);
    }
    if status == 429 {
        return ProviderError::RateLimited(text);
    }
    ProviderError::Api {
        status,
        message: text,
    }
}

fn map_reqwest_error(e: reqwest::Error, timeout: Duration) -> ProviderError {
    if e.is_timeout() {
        ProviderError::Timeout(timeout)
    } else {
        ProviderError::Transport(e.to_string())
    }
}
