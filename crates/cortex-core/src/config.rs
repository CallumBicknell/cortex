//! Kernel and runtime configuration.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration for the kernel and runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// Interval between optional heartbeat iterations in milliseconds.
    pub loop_interval_ms: u64,
    /// Log level for tracing (e.g. "info", "debug").
    pub log_level: String,
    /// Maximum number of events retained in the in-memory bus history.
    pub event_history_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            loop_interval_ms: 100,
            log_level: "info".to_string(),
            event_history_size: 1024,
        }
    }
}

impl Config {
    /// Load configuration from environment variables with fallback to defaults.
    ///
    /// Environment variables:
    /// - `CORTEX_LOOP_INTERVAL_MS`: loop interval in milliseconds
    /// - `CORTEX_LOG_LEVEL`: tracing log level
    /// - `CORTEX_EVENT_HISTORY_SIZE`: in-memory event history capacity
    pub fn from_env() -> Self {
        let defaults = Self::default();
        let loop_interval_ms = std::env::var("CORTEX_LOOP_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.loop_interval_ms);
        let log_level =
            std::env::var("CORTEX_LOG_LEVEL").unwrap_or_else(|_| defaults.log_level.clone());
        let event_history_size = std::env::var("CORTEX_EVENT_HISTORY_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.event_history_size);

        Self {
            loop_interval_ms,
            log_level,
            event_history_size,
        }
    }

    /// Load configuration from a TOML file, then overlay environment variables.
    ///
    /// Missing keys use defaults. Environment variables always win.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|source| ConfigError::Io {
            path: path.as_ref().display().to_string(),
            source,
        })?;
        Self::from_toml(&content)
    }

    /// Parse configuration from a TOML string, then overlay environment variables.
    pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
        let file: ConfigFile = toml::from_str(content).map_err(|source| ConfigError::Parse {
            source: source.to_string(),
        })?;
        let mut config = Self::default();
        if let Some(v) = file.loop_interval_ms {
            config.loop_interval_ms = v;
        }
        if let Some(v) = file.log_level {
            config.log_level = v;
        }
        if let Some(v) = file.event_history_size {
            config.event_history_size = v;
        }
        config.apply_env_overrides();
        config.validate()?;
        Ok(config)
    }

    /// Overlay environment variables onto this config (in place).
    pub fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("CORTEX_LOOP_INTERVAL_MS") {
            if let Ok(parsed) = v.parse() {
                self.loop_interval_ms = parsed;
            }
        }
        if let Ok(v) = std::env::var("CORTEX_LOG_LEVEL") {
            self.log_level = v;
        }
        if let Ok(v) = std::env::var("CORTEX_EVENT_HISTORY_SIZE") {
            if let Ok(parsed) = v.parse() {
                self.event_history_size = parsed;
            }
        }
    }

    /// Validate configuration values.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.loop_interval_ms == 0 {
            return Err(ConfigError::Invalid(
                "loop_interval_ms must be greater than 0".into(),
            ));
        }
        if self.event_history_size == 0 {
            return Err(ConfigError::Invalid(
                "event_history_size must be greater than 0".into(),
            ));
        }
        let allowed = ["trace", "debug", "info", "warn", "error", "off"];
        if !allowed.contains(&self.log_level.to_ascii_lowercase().as_str()) {
            return Err(ConfigError::Invalid(format!(
                "log_level must be one of {allowed:?}, got {:?}",
                self.log_level
            )));
        }
        Ok(())
    }
}

/// Partial config as stored in TOML files.
#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    loop_interval_ms: Option<u64>,
    log_level: Option<String>,
    event_history_size: Option<usize>,
}

/// Errors produced while loading or validating configuration.
#[derive(Debug)]
pub enum ConfigError {
    /// Filesystem error while reading a config file.
    Io {
        /// Path that failed to load.
        path: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// TOML parse failure.
    Parse {
        /// Parse error message.
        source: String,
    },
    /// Semantic validation failure.
    Invalid(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "failed to read config at {path}: {source}"),
            Self::Parse { source } => write!(f, "failed to parse config: {source}"),
            Self::Invalid(msg) => write!(f, "invalid config: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { .. } | Self::Invalid(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid() {
        let cfg = Config::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn from_toml_parses_fields() {
        let cfg = Config::from_toml(
            r#"
            loop_interval_ms = 50
            log_level = "debug"
            event_history_size = 64
            "#,
        )
        .expect("toml should parse");
        assert_eq!(cfg.loop_interval_ms, 50);
        assert_eq!(cfg.log_level, "debug");
        assert_eq!(cfg.event_history_size, 64);
    }

    #[test]
    fn rejects_zero_history() {
        let cfg = Config {
            event_history_size: 0,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }
}
