//! `plugins.toml` configuration.

use crate::error::{PluginError, Result};
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

/// Root plugins configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginsConfig {
    /// Master switch — when false, no plugins load.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Declared plugins (in load order).
    #[serde(default)]
    pub plugins: Vec<PluginEntry>,
}

fn default_true() -> bool {
    true
}

/// One plugin declaration.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginEntry {
    /// Builtin plugin id (e.g. `echo`).
    pub id: String,
    /// Whether this entry is active.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Free-form settings object (JSON-compatible via TOML).
    #[serde(default)]
    pub settings: Value,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            plugins: vec![PluginEntry {
                id: "echo".into(),
                enabled: true,
                settings: Value::Object(Default::default()),
            }],
        }
    }
}

impl PluginsConfig {
    /// Parse TOML text.
    pub fn from_toml(text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|e| PluginError::Config(e.to_string()))
    }

    /// Load from a file path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())?;
        Self::from_toml(&text)
    }

    /// Enabled plugin entries only (when master switch is on).
    pub fn enabled_entries(&self) -> Vec<&PluginEntry> {
        if !self.enabled {
            return Vec::new();
        }
        self.plugins.iter().filter(|p| p.enabled).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plugins_toml() {
        let cfg = PluginsConfig::from_toml(
            r#"
            enabled = true
            [[plugins]]
            id = "echo"
            enabled = true
            [plugins.settings]
            prefix = "hi"
            "#,
        )
        .unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.plugins.len(), 1);
        assert_eq!(cfg.plugins[0].id, "echo");
        assert_eq!(cfg.plugins[0].settings["prefix"], "hi");
    }

    #[test]
    fn master_off_disables_all() {
        let cfg = PluginsConfig::from_toml(
            r#"
            enabled = false
            [[plugins]]
            id = "echo"
            "#,
        )
        .unwrap();
        assert!(cfg.enabled_entries().is_empty());
    }
}
