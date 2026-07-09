//! Deterministic mock provider for tests and offline demos.

use crate::error::{ProviderError, Result};
use crate::provider::{ChatStream, Provider};
use crate::types::{
    ChatRequest, ChatResponse, EmbedRequest, EmbedResponse, FinishReason, StreamEvent, Usage,
};
use async_trait::async_trait;
use cortex_models::Message;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// A scripted response the mock will return.
#[derive(Debug, Clone)]
pub enum MockResponse {
    /// Return a fixed chat response.
    Chat(ChatResponse),
    /// Fail with an error.
    Error(String),
}

impl MockResponse {
    /// Simple assistant text reply.
    pub fn text(model: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Chat(ChatResponse {
            model: model.into(),
            message: Message::assistant(content),
            finish_reason: FinishReason::Stop,
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            raw: None,
        })
    }

    /// Assistant message with tool calls.
    pub fn with_tools(model: impl Into<String>, message: Message) -> Self {
        Self::Chat(ChatResponse {
            model: model.into(),
            message,
            finish_reason: FinishReason::ToolCalls,
            usage: Usage::default(),
            raw: None,
        })
    }
}

/// Mock provider that walks through a queue of scripted responses.
pub struct MockProvider {
    id: String,
    scripts: Mutex<Vec<MockResponse>>,
    /// Used when the script queue is empty (reusable; good for CLI demos).
    fallback: Option<MockResponse>,
    call_count: AtomicUsize,
    /// If true, stream emits text deltas character-by-character for text responses.
    stream_deltas: bool,
}

impl MockProvider {
    /// Create a mock with the given scripted responses (consumed in order).
    pub fn new(scripts: Vec<MockResponse>) -> Self {
        Self {
            id: "mock".into(),
            scripts: Mutex::new(scripts),
            fallback: None,
            call_count: AtomicUsize::new(0),
            stream_deltas: false,
        }
    }

    /// Empty mock (errors when scripts are exhausted, unless a fallback is set).
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Mock that always answers with the same text when scripts are empty.
    pub fn with_fallback(mut self, response: MockResponse) -> Self {
        self.fallback = Some(response);
        self
    }

    /// Offline-friendly mock that always returns a short assistant message.
    pub fn echo(message: impl Into<String>) -> Self {
        Self::empty().with_fallback(MockResponse::text("mock-default", message))
    }

    /// Enable character-level stream deltas for text responses.
    pub fn with_stream_deltas(mut self, enabled: bool) -> Self {
        self.stream_deltas = enabled;
        self
    }

    /// Append a scripted response.
    pub fn push(&self, response: MockResponse) {
        self.scripts.lock().expect("mock lock").push(response);
    }

    /// How many `chat`/`stream` calls have been made.
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    fn materialize(resp: MockResponse, model: &str) -> Result<ChatResponse> {
        match resp {
            MockResponse::Chat(mut r) => {
                if r.model.is_empty() {
                    r.model = model.to_string();
                }
                Ok(r)
            }
            MockResponse::Error(msg) => Err(ProviderError::Api {
                status: 500,
                message: msg,
            }),
        }
    }

    fn next_response(&self, req: &ChatRequest) -> Result<ChatResponse> {
        if let Some(c) = &req.cancel {
            if c.is_cancelled() {
                return Err(ProviderError::cancelled("mock chat aborted"));
            }
        }
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let mut scripts = self.scripts.lock().expect("mock lock");
        if !scripts.is_empty() {
            return Self::materialize(scripts.remove(0), &req.model);
        }
        if let Some(fallback) = &self.fallback {
            return Self::materialize(fallback.clone(), &req.model);
        }
        Err(ProviderError::InvalidRequest(
            "mock provider has no more scripted responses".into(),
        ))
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn id(&self) -> &str {
        &self.id
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        self.next_response(&req)
    }

    async fn stream(&self, req: ChatRequest) -> Result<ChatStream> {
        let response = self.next_response(&req)?;
        let stream_deltas = self.stream_deltas;
        let stream = async_stream::stream! {
            if stream_deltas {
                for ch in response.message.content.chars() {
                    yield Ok(StreamEvent::TextDelta {
                        text: ch.to_string(),
                    });
                }
            }
            yield Ok(StreamEvent::Done { response });
        };
        Ok(Box::pin(stream))
    }

    async fn embeddings(&self, req: EmbedRequest) -> Result<EmbedResponse> {
        if let Some(c) = &req.cancel {
            if c.is_cancelled() {
                return Err(ProviderError::cancelled("mock embeddings aborted"));
            }
        }
        // Deterministic pseudo-embedding from content hash (64-d).
        let embeddings = req.input.iter().map(|t| mock_embed(t)).collect::<Vec<_>>();
        Ok(EmbedResponse {
            model: req.model,
            embeddings,
            usage: Usage {
                prompt_tokens: req.input.iter().map(|t| t.len() as u32 / 4).sum(),
                completion_tokens: 0,
                total_tokens: req.input.iter().map(|t| t.len() as u32 / 4).sum(),
            },
        })
    }
}

fn mock_embed(text: &str) -> Vec<f32> {
    const DIMS: usize = 64;
    let mut v = vec![0.0f32; DIMS];
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
        let idx = (h as usize) % DIMS;
        v[idx] += 1.0;
    }
    let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        for x in &mut v {
            *x /= n;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_models::{Message, ToolCall};
    use futures::StreamExt;
    use serde_json::json;

    #[tokio::test]
    async fn scripted_sequence() {
        let mock = MockProvider::new(vec![
            MockResponse::text("mock-model", "first"),
            MockResponse::with_tools(
                "mock-model",
                Message::assistant_with_tools(
                    "",
                    vec![ToolCall::new("read_file", json!({"path": "a.rs"}))],
                ),
            ),
        ]);

        let r1 = mock
            .chat(ChatRequest::new("mock-model", vec![Message::user("hi")]))
            .await
            .unwrap();
        assert_eq!(r1.message.content, "first");
        assert!(!r1.has_tool_calls());

        let r2 = mock
            .chat(ChatRequest::new("mock-model", vec![Message::user("next")]))
            .await
            .unwrap();
        assert!(r2.has_tool_calls());
        assert_eq!(r2.tool_calls()[0].name, "read_file");
        assert_eq!(mock.call_count(), 2);
    }

    #[tokio::test]
    async fn stream_done() {
        let mock =
            MockProvider::new(vec![MockResponse::text("m", "hello")]).with_stream_deltas(true);
        let mut stream = mock
            .stream(ChatRequest::new("m", vec![Message::user("x")]))
            .await
            .unwrap();
        let mut text = String::new();
        let mut done = false;
        while let Some(ev) = stream.next().await {
            match ev.unwrap() {
                StreamEvent::TextDelta { text: t } => text.push_str(&t),
                StreamEvent::Done { response } => {
                    assert_eq!(response.message.content, "hello");
                    done = true;
                }
                _ => {}
            }
        }
        assert_eq!(text, "hello");
        assert!(done);
    }
}
