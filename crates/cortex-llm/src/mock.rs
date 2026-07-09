//! Deterministic mock provider for tests and offline demos.

use crate::error::{ProviderError, Result};
use crate::provider::{ChatStream, Provider};
use crate::types::{ChatRequest, ChatResponse, FinishReason, StreamEvent, Usage};
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
            call_count: AtomicUsize::new(0),
            stream_deltas: false,
        }
    }

    /// Empty mock (always errors until scripts are pushed).
    pub fn empty() -> Self {
        Self::new(Vec::new())
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

    fn next_response(&self, req: &ChatRequest) -> Result<ChatResponse> {
        if let Some(c) = &req.cancel {
            if c.is_cancelled() {
                return Err(ProviderError::cancelled("mock chat aborted"));
            }
        }
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let mut scripts = self.scripts.lock().expect("mock lock");
        if scripts.is_empty() {
            return Err(ProviderError::InvalidRequest(
                "mock provider has no more scripted responses".into(),
            ));
        }
        match scripts.remove(0) {
            MockResponse::Chat(mut resp) => {
                if resp.model.is_empty() {
                    resp.model = req.model.clone();
                }
                Ok(resp)
            }
            MockResponse::Error(msg) => Err(ProviderError::Api {
                status: 500,
                message: msg,
            }),
        }
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
