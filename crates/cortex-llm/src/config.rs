//! Provider / model configuration (`models.toml`).

use crate::error::{ProviderError, Result};
use crate::retry::RetryPolicy;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

/// Top-level models configuration file.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ModelsConfig {
    /// Named provider configs.
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfigEntry>,
    /// Named model aliases.
    #[serde(default)]
    pub models: HashMap<String, ModelAlias>,
    /// Default model alias name (e.g. `"default"`).
    #[serde(default)]
    pub default_model: Option<String>,
}

/// Kind of provider backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// OpenAI Chat Completions compatible HTTP API.
    OpenaiCompatible,
    /// Anthropic Messages API.
    Anthropic,
    /// In-process mock (tests).
    Mock,
}

/// One provider entry from config.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfigEntry {
    /// Backend kind.
    pub kind: ProviderKind,
    /// Base URL (required for HTTP providers).
    #[serde(default)]
    pub base_url: Option<String>,
    /// Env var name holding the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Literal API key (prefer env; only for local dev).
    #[serde(default)]
    pub api_key: Option<String>,
    /// Request timeout in seconds.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Max retry attempts.
    #[serde(default)]
    pub max_retries: Option<u32>,
    /// Anthropic API version.
    #[serde(default)]
    pub api_version: Option<String>,
    /// Extra headers.
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

/// A named model alias.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelAlias {
    /// Provider id (key in `providers`).
    pub provider: String,
    /// Model id for that provider.
    pub model: String,
}

impl ModelsConfig {
    /// Load from a TOML file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            ProviderError::Config(format!("failed to read {}: {e}", path.as_ref().display()))
        })?;
        Self::from_toml(&content)
    }

    /// Parse TOML content.
    pub fn from_toml(content: &str) -> Result<Self> {
        toml::from_str(content)
            .map_err(|e| ProviderError::Config(format!("parse models.toml: {e}")))
    }

    /// Resolve the default model alias, if configured.
    pub fn default_alias(&self) -> Option<&ModelAlias> {
        let name = self.default_model.as_deref().unwrap_or("default");
        self.models.get(name)
    }

    /// Resolve an alias by name.
    pub fn resolve_alias(&self, name: &str) -> Result<&ModelAlias> {
        self.models
            .get(name)
            .ok_or_else(|| ProviderError::NotFound(format!("model alias `{name}` not found")))
    }

    /// Resolve API key for a provider entry (env wins over literal).
    pub fn resolve_api_key(entry: &ProviderConfigEntry) -> Option<String> {
        if let Some(env_name) = &entry.api_key_env {
            if let Ok(v) = std::env::var(env_name) {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        entry.api_key.clone().filter(|s| !s.is_empty())
    }

    /// Build a retry policy from a provider entry.
    pub fn retry_policy(entry: &ProviderConfigEntry) -> RetryPolicy {
        let mut policy = RetryPolicy::default();
        if let Some(n) = entry.max_retries {
            policy.max_attempts = n.max(1);
        }
        policy
    }

    /// Timeout duration from entry.
    pub fn timeout(entry: &ProviderConfigEntry) -> Duration {
        Duration::from_secs(entry.timeout_secs.unwrap_or(120))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_toml() {
        let cfg = ModelsConfig::from_toml(
            r#"
            default_model = "default"

            [providers.openai]
            kind = "openai_compatible"
            base_url = "https://api.openai.com/v1"
            api_key_env = "OPENAI_API_KEY"
            timeout_secs = 60
            max_retries = 2

            [providers.ollama]
            kind = "openai_compatible"
            base_url = "http://127.0.0.1:11434/v1"

            [providers.anthropic]
            kind = "anthropic"
            api_key_env = "ANTHROPIC_API_KEY"

            [models.default]
            provider = "openai"
            model = "gpt-4.1"

            [models.fast]
            provider = "ollama"
            model = "qwen2.5-coder"
            "#,
        )
        .unwrap();

        assert_eq!(cfg.providers.len(), 3);
        assert_eq!(cfg.models.len(), 2);
        let def = cfg.default_alias().unwrap();
        assert_eq!(def.provider, "openai");
        assert_eq!(def.model, "gpt-4.1");
        assert_eq!(cfg.providers["openai"].kind, ProviderKind::OpenaiCompatible);
    }
}
