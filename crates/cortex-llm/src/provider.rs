//! Provider trait.

use crate::error::{ProviderError, Result};
use crate::types::{ChatRequest, ChatResponse, EmbedRequest, EmbedResponse, StreamEvent};
use async_trait::async_trait;
use futures::stream::BoxStream;

/// A boxed stream of chat stream events.
pub type ChatStream = BoxStream<'static, Result<StreamEvent>>;

/// LLM provider abstraction.
///
/// Implementations must be provider-agnostic at the trait boundary: the runtime
/// never imports concrete adapters.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Stable provider id (e.g. `"openai"`, `"anthropic"`, `"mock"`).
    fn id(&self) -> &str;

    /// Non-streaming chat completion.
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse>;

    /// Streaming chat completion.
    ///
    /// Default implementation falls back to [`Self::chat`] and emits a single
    /// [`StreamEvent::Done`].
    async fn stream(&self, req: ChatRequest) -> Result<ChatStream> {
        let response = self.chat(req).await?;
        let stream = async_stream::stream! {
            yield Ok(StreamEvent::Done { response });
        };
        Ok(Box::pin(stream))
    }

    /// Embeddings (optional).
    async fn embeddings(&self, _req: EmbedRequest) -> Result<EmbedResponse> {
        Err(ProviderError::Unsupported(format!(
            "provider `{}` does not support embeddings",
            self.id()
        )))
    }
}
