//! Provider and model alias registry.

use crate::anthropic::{AnthropicConfig, AnthropicProvider};
use crate::config::{ModelsConfig, ProviderKind};
use crate::error::{ProviderError, Result};
use crate::mock::{MockProvider, MockResponse};
use crate::openai_compatible::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};
use crate::provider::Provider;
use crate::types::{ChatRequest, ChatResponse};
use std::collections::HashMap;
use std::sync::Arc;

/// Resolved model binding: provider handle + concrete model id.
#[derive(Clone)]
pub struct ResolvedModel {
    /// Alias name (e.g. `"default"`).
    pub alias: String,
    /// Provider id.
    pub provider_id: String,
    /// Provider instance.
    pub provider: Arc<dyn Provider>,
    /// Model id for the provider API.
    pub model: String,
}

impl ResolvedModel {
    /// Run chat with the bound model id (overwrites `req.model`).
    pub async fn chat(&self, mut req: ChatRequest) -> Result<ChatResponse> {
        req.model = self.model.clone();
        self.provider.chat(req).await
    }
}

/// Registry of providers and model aliases.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
    aliases: HashMap<String, (String, String)>, // alias -> (provider_id, model)
    default_alias: Option<String>,
}

impl ProviderRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            aliases: HashMap::new(),
            default_alias: None,
        }
    }

    /// Build a registry from `ModelsConfig`.
    pub fn from_config(config: &ModelsConfig) -> Result<Self> {
        let mut reg = Self::new();
        for (id, entry) in &config.providers {
            let provider: Arc<dyn Provider> = match entry.kind {
                ProviderKind::OpenaiCompatible => {
                    let base_url = entry.base_url.clone().ok_or_else(|| {
                        ProviderError::Config(format!(
                            "provider `{id}` missing base_url for openai_compatible"
                        ))
                    })?;
                    let mut cfg = OpenAiCompatibleConfig {
                        id: id.clone(),
                        base_url,
                        api_key: ModelsConfig::resolve_api_key(entry),
                        timeout: ModelsConfig::timeout(entry),
                        retry: ModelsConfig::retry_policy(entry),
                        extra_headers: entry
                            .headers
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    };
                    // Ensure id matches map key.
                    cfg.id = id.clone();
                    Arc::new(OpenAiCompatibleProvider::new(cfg)?)
                }
                ProviderKind::Anthropic => {
                    let api_key = ModelsConfig::resolve_api_key(entry).ok_or_else(|| {
                        ProviderError::Config(format!(
                            "provider `{id}` requires ANTHROPIC_API_KEY or api_key"
                        ))
                    })?;
                    let mut cfg = AnthropicConfig::new(api_key);
                    cfg.id = id.clone();
                    if let Some(url) = &entry.base_url {
                        cfg.base_url = url.clone();
                    }
                    if let Some(v) = &entry.api_version {
                        cfg.api_version = v.clone();
                    }
                    cfg.timeout = ModelsConfig::timeout(entry);
                    cfg.retry = ModelsConfig::retry_policy(entry);
                    Arc::new(AnthropicProvider::new(cfg)?)
                }
                ProviderKind::Mock => {
                    // Empty mock; tests can replace via register.
                    Arc::new(MockProvider::empty())
                }
            };
            reg.register_provider(id.clone(), provider);
        }

        for (alias, model) in &config.models {
            reg.register_alias(alias.clone(), model.provider.clone(), model.model.clone())?;
        }
        reg.default_alias = config.default_model.clone().or_else(|| {
            config
                .models
                .contains_key("default")
                .then(|| "default".into())
        });
        Ok(reg)
    }

    /// Register a provider instance.
    pub fn register_provider(&mut self, id: impl Into<String>, provider: Arc<dyn Provider>) {
        self.providers.insert(id.into(), provider);
    }

    /// Register a convenience mock provider under `"mock"` with scripts.
    pub fn register_mock(&mut self, scripts: Vec<MockResponse>) {
        self.register_provider("mock", Arc::new(MockProvider::new(scripts)));
    }

    /// Register a model alias.
    pub fn register_alias(
        &mut self,
        alias: impl Into<String>,
        provider_id: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<()> {
        let alias = alias.into();
        let provider_id = provider_id.into();
        if !self.providers.contains_key(&provider_id) {
            return Err(ProviderError::NotFound(format!(
                "provider `{provider_id}` not registered for alias `{alias}`"
            )));
        }
        self.aliases.insert(alias, (provider_id, model.into()));
        Ok(())
    }

    /// Set the default alias name.
    pub fn set_default_alias(&mut self, alias: impl Into<String>) {
        self.default_alias = Some(alias.into());
    }

    /// Get a provider by id.
    pub fn provider(&self, id: &str) -> Result<Arc<dyn Provider>> {
        self.providers
            .get(id)
            .cloned()
            .ok_or_else(|| ProviderError::NotFound(format!("provider `{id}` not found")))
    }

    /// List provider ids.
    pub fn provider_ids(&self) -> Vec<String> {
        let mut ids: Vec<_> = self.providers.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// List model alias names.
    pub fn alias_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.aliases.keys().cloned().collect();
        names.sort();
        names
    }

    /// Resolve a model alias (or default if `None` / empty).
    pub fn resolve(&self, alias: Option<&str>) -> Result<ResolvedModel> {
        let name = alias
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| self.default_alias.clone())
            .ok_or_else(|| {
                ProviderError::NotFound("no model alias specified and no default configured".into())
            })?;
        let (provider_id, model) =
            self.aliases.get(&name).cloned().ok_or_else(|| {
                ProviderError::NotFound(format!("model alias `{name}` not found"))
            })?;
        let provider = self.provider(&provider_id)?;
        Ok(ResolvedModel {
            alias: name,
            provider_id,
            provider,
            model,
        })
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_models::Message;

    #[tokio::test]
    async fn resolve_and_chat() {
        let mut reg = ProviderRegistry::new();
        reg.register_mock(vec![MockResponse::text("m", "pong")]);
        reg.register_alias("default", "mock", "m").unwrap();
        reg.set_default_alias("default");

        let resolved = reg.resolve(None).unwrap();
        assert_eq!(resolved.provider_id, "mock");
        let resp = resolved
            .chat(ChatRequest::new("ignored", vec![Message::user("ping")]))
            .await
            .unwrap();
        assert_eq!(resp.message.content, "pong");
        assert_eq!(resp.model, "m");
    }
}
