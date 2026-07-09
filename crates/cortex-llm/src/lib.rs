//! Cortex LLM providers.
//!
//! Provider-agnostic chat/embeddings traits with adapters for OpenAI-compatible
//! APIs (OpenAI, OpenRouter, Ollama, LM Studio), Anthropic, and an in-process mock.

#![deny(missing_docs)]

mod anthropic;
mod config;
mod error;
mod mock;
mod openai_compatible;
mod provider;
mod registry;
mod retry;
mod types;

pub use anthropic::{AnthropicConfig, AnthropicProvider};
pub use config::{ModelAlias, ModelsConfig, ProviderConfigEntry, ProviderKind};
pub use error::{ProviderError, Result};
pub use mock::{MockProvider, MockResponse};
pub use openai_compatible::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};
pub use provider::{ChatStream, Provider};
pub use registry::{ProviderRegistry, ResolvedModel};
pub use retry::RetryPolicy;
pub use types::{
    ChatRequest, ChatResponse, EmbedRequest, EmbedResponse, FinishReason, StreamEvent, Usage,
};
