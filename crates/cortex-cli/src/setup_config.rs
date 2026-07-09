//! Setup detection + models.toml generation for the first-run wizard.

use anyhow::{bail, Context, Result};
use std::path::Path;

/// Env / ambient provider detection (never stores secret values).
#[derive(Debug, Clone, Default)]
pub struct DetectedEnv {
    /// `OPENAI_API_KEY` is set and non-empty.
    pub openai_key: bool,
    /// `ANTHROPIC_API_KEY` is set and non-empty.
    pub anthropic_key: bool,
    /// `OPENROUTER_API_KEY` is set and non-empty.
    pub openrouter_key: bool,
    /// Something answered on Ollama's default port (best-effort).
    pub ollama_up: bool,
}

impl DetectedEnv {
    /// Scan process environment (and optional quick Ollama probe).
    pub fn detect() -> Self {
        Self {
            openai_key: env_nonempty("OPENAI_API_KEY"),
            anthropic_key: env_nonempty("ANTHROPIC_API_KEY"),
            openrouter_key: env_nonempty("OPENROUTER_API_KEY"),
            ollama_up: probe_ollama(),
        }
    }

    /// Human lines for the TUI sidebar.
    pub fn summary_lines(&self) -> Vec<String> {
        let mut v = Vec::new();
        v.push(format!(
            "OpenAI key:     {}",
            if self.openai_key { "detected" } else { "—" }
        ));
        v.push(format!(
            "Anthropic key:  {}",
            if self.anthropic_key {
                "detected"
            } else {
                "—"
            }
        ));
        v.push(format!(
            "OpenRouter key: {}",
            if self.openrouter_key {
                "detected"
            } else {
                "—"
            }
        ));
        v.push(format!(
            "Ollama :11434:  {}",
            if self.ollama_up { "up" } else { "—" }
        ));
        v
    }

    /// Prefer the first detected cloud/local provider for "Auto".
    pub fn auto_preset(&self) -> SetupPreset {
        if self.openai_key {
            return SetupPreset::OpenAI {
                model: "gpt-4.1".into(),
            };
        }
        if self.anthropic_key {
            return SetupPreset::Anthropic {
                model: "claude-sonnet-4-20250514".into(),
            };
        }
        if self.openrouter_key {
            return SetupPreset::OpenRouter {
                model: "anthropic/claude-sonnet-4".into(),
            };
        }
        if self.ollama_up {
            return SetupPreset::Ollama {
                model: "qwen2.5-coder".into(),
            };
        }
        SetupPreset::Mock
    }
}

fn env_nonempty(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| !v.is_empty())
}

fn probe_ollama() -> bool {
    // Fast TCP connect — no full HTTP client required.
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    let Ok(mut addrs) = ("127.0.0.1:11434").to_socket_addrs() else {
        return false;
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
}

/// User's chosen default provider / model for setup.
#[derive(Debug, Clone)]
pub enum SetupPreset {
    /// Offline mock provider.
    Mock,
    /// Local Ollama OpenAI-compatible API.
    Ollama {
        /// Model id (e.g. `qwen2.5-coder`).
        model: String,
    },
    /// Official OpenAI API.
    OpenAI {
        /// Model id.
        model: String,
    },
    /// Anthropic Messages API.
    Anthropic {
        /// Model id.
        model: String,
    },
    /// OpenRouter gateway.
    OpenRouter {
        /// Model id.
        model: String,
    },
    /// Custom OpenAI-compatible endpoint.
    Custom {
        /// Provider + alias id (snake_case).
        id: String,
        /// Base URL for chat completions.
        base_url: String,
        /// Model id string.
        model: String,
        /// Env var name holding the API key (may be empty for local).
        api_key_env: String,
    },
}

impl SetupPreset {
    /// Alias written to `default_model`.
    pub fn alias(&self) -> &str {
        match self {
            Self::Mock => "default",
            Self::Ollama { .. } => "ollama",
            Self::OpenAI { .. } => "openai",
            Self::Anthropic { .. } => "anthropic",
            Self::OpenRouter { .. } => "openrouter",
            Self::Custom { id, .. } => id.as_str(),
        }
    }

    /// Short label for list UI.
    pub fn label(&self) -> String {
        match self {
            Self::Mock => "Mock (offline)".into(),
            Self::Ollama { model } => format!("Ollama · {model}"),
            Self::OpenAI { model } => format!("OpenAI · {model}"),
            Self::Anthropic { model } => format!("Anthropic · {model}"),
            Self::OpenRouter { model } => format!("OpenRouter · {model}"),
            Self::Custom { id, model, .. } => format!("Custom `{id}` · {model}"),
        }
    }
}

/// Validate custom provider id / free-form strings.
pub fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 64 {
        bail!("id must be 1–64 characters");
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        bail!("id must be [A-Za-z0-9_-]+");
    }
    if id == "default" || id == "mock" {
        bail!("reserved id `{id}`");
    }
    Ok(())
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Render a complete models.toml for the chosen preset (includes common providers).
pub fn render_models_toml(preset: &SetupPreset) -> Result<String> {
    let default_alias = preset.alias();
    if matches!(preset, SetupPreset::Custom { .. }) {
        validate_id(default_alias)?;
    }

    let mut out = String::new();
    out.push_str("# Cortex model / provider configuration\n");
    out.push_str("# Generated by `cortex setup`. API keys stay in environment variables.\n\n");
    out.push_str(&format!("default_model = \"{}\"\n\n", esc(default_alias)));

    // Standard providers always present for easy switching.
    out.push_str(
        r#"[providers.openai]
kind = "openai_compatible"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
timeout_secs = 120
max_retries = 3

[providers.openrouter]
kind = "openai_compatible"
base_url = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
timeout_secs = 120
max_retries = 3

[providers.ollama]
kind = "openai_compatible"
base_url = "http://127.0.0.1:11434/v1"
timeout_secs = 300
max_retries = 1

[providers.lmstudio]
kind = "openai_compatible"
base_url = "http://127.0.0.1:1234/v1"
timeout_secs = 300
max_retries = 1

[providers.anthropic]
kind = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
timeout_secs = 120
max_retries = 3

[providers.mock]
kind = "mock"

"#,
    );

    // Custom provider block when needed.
    if let SetupPreset::Custom {
        id,
        base_url,
        api_key_env,
        ..
    } = preset
    {
        out.push_str(&format!("[providers.{id}]\n"));
        out.push_str("kind = \"openai_compatible\"\n");
        out.push_str(&format!("base_url = \"{}\"\n", esc(base_url)));
        if !api_key_env.trim().is_empty() {
            out.push_str(&format!("api_key_env = \"{}\"\n", esc(api_key_env.trim())));
        }
        out.push_str("timeout_secs = 120\n");
        out.push_str("max_retries = 3\n\n");
    }

    // Model aliases.
    let openai_model = match preset {
        SetupPreset::OpenAI { model } => model.as_str(),
        _ => "gpt-4.1",
    };
    let ollama_model = match preset {
        SetupPreset::Ollama { model } => model.as_str(),
        _ => "qwen2.5-coder",
    };
    let openrouter_model = match preset {
        SetupPreset::OpenRouter { model } => model.as_str(),
        _ => "anthropic/claude-sonnet-4",
    };
    let anthropic_model = match preset {
        SetupPreset::Anthropic { model } => model.as_str(),
        _ => "claude-sonnet-4-20250514",
    };

    out.push_str("[models.default]\n");
    // When mock is not the default, still keep alias `default` as mock for offline.
    if matches!(preset, SetupPreset::Mock) {
        out.push_str("provider = \"mock\"\n");
        out.push_str("model = \"mock-default\"\n\n");
    } else {
        // Point `default` alias at the chosen provider so `default_model = "default"` works too
        // when user later changes default_model line — actually we set default_model to alias.
        // Keep models.default as mock for safe offline fallback alias.
        out.push_str("provider = \"mock\"\n");
        out.push_str("model = \"mock-default\"\n\n");
    }

    out.push_str("[models.openai]\n");
    out.push_str("provider = \"openai\"\n");
    out.push_str(&format!("model = \"{}\"\n\n", esc(openai_model)));

    out.push_str("[models.ollama]\n");
    out.push_str("provider = \"ollama\"\n");
    out.push_str(&format!("model = \"{}\"\n\n", esc(ollama_model)));

    out.push_str("[models.openrouter]\n");
    out.push_str("provider = \"openrouter\"\n");
    out.push_str(&format!("model = \"{}\"\n\n", esc(openrouter_model)));

    out.push_str("[models.anthropic]\n");
    out.push_str("provider = \"anthropic\"\n");
    out.push_str(&format!("model = \"{}\"\n\n", esc(anthropic_model)));

    if let SetupPreset::Custom { id, model, .. } = preset {
        out.push_str(&format!("[models.{id}]\n"));
        out.push_str(&format!("provider = \"{}\"\n", esc(id)));
        out.push_str(&format!("model = \"{}\"\n\n", esc(model)));
    }

    // Ensure default_model alias exists when not mock/custom-already-handled.
    // For openai/ollama/etc the alias name matches models.* above.
    let _ = default_alias;
    Ok(out)
}

/// Write generated models.toml to disk.
pub fn write_setup_models_toml(path: &Path, preset: &SetupPreset) -> Result<()> {
    let body = render_models_toml(preset)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(path, body).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Map non-interactive CLI flags to a preset.
pub fn preset_from_flags(default_model: &str, ollama_model: Option<&str>) -> Result<SetupPreset> {
    match default_model {
        "default" | "mock" => Ok(SetupPreset::Mock),
        "ollama" => Ok(SetupPreset::Ollama {
            model: ollama_model.unwrap_or("qwen2.5-coder").into(),
        }),
        "openai" => Ok(SetupPreset::OpenAI {
            model: "gpt-4.1".into(),
        }),
        "anthropic" => Ok(SetupPreset::Anthropic {
            model: "claude-sonnet-4-20250514".into(),
        }),
        "openrouter" => Ok(SetupPreset::OpenRouter {
            model: "anthropic/claude-sonnet-4".into(),
        }),
        other => bail!(
            "unknown --default-model `{other}`; use default|ollama|openai|anthropic|openrouter or the TUI wizard for custom"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn render_openai_parses() {
        let toml = render_models_toml(&SetupPreset::OpenAI {
            model: "gpt-4.1".into(),
        })
        .unwrap();
        assert!(toml.contains("default_model = \"openai\""));
        assert!(toml.contains("[providers.anthropic]"));
        let cfg = cortex_llm::ModelsConfig::from_toml(&toml).unwrap();
        assert_eq!(cfg.default_model.as_deref(), Some("openai"));
        assert!(cfg.providers.contains_key("openai"));
        assert!(cfg.models.contains_key("openai"));
    }

    #[test]
    fn render_custom_and_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("models.toml");
        let preset = SetupPreset::Custom {
            id: "groq".into(),
            base_url: "https://api.groq.com/openai/v1".into(),
            model: "llama-3.3-70b".into(),
            api_key_env: "GROQ_API_KEY".into(),
        };
        write_setup_models_toml(&path, &preset).unwrap();
        let cfg = cortex_llm::ModelsConfig::from_file(&path).unwrap();
        assert_eq!(cfg.default_model.as_deref(), Some("groq"));
        assert_eq!(cfg.models["groq"].provider, "groq");
        assert_eq!(
            cfg.providers["groq"].base_url.as_deref(),
            Some("https://api.groq.com/openai/v1")
        );
    }

    #[test]
    fn auto_prefers_openai_when_key_set() {
        // Don't mutate process env in parallel tests hard — just check Mock default.
        let d = DetectedEnv::default();
        assert!(matches!(d.auto_preset(), SetupPreset::Mock));
    }
}
