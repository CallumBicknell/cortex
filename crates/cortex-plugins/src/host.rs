//! Plugin host: load, init, start, stop.

use crate::builtins::{builtin_ids, create_builtin};
use crate::config::PluginsConfig;
use crate::error::{PluginError, Result};
use crate::plugin::{Plugin, PluginContext, PluginMeta};
use cortex_tools::ToolRegistry;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Runtime state of a loaded plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    /// Constructed, not yet initialized.
    Loaded,
    /// `init` succeeded.
    Initialized,
    /// `start` succeeded.
    Started,
    /// `stop` completed (or failed after start).
    Stopped,
}

/// Snapshot for CLI listing.
#[derive(Debug, Clone)]
pub struct PluginStatus {
    /// Metadata.
    pub meta: PluginMeta,
    /// Lifecycle state.
    pub state: PluginState,
    /// Whether the entry was enabled in config.
    pub enabled: bool,
}

/// Owns loaded plugins and drives lifecycle.
pub struct PluginHost {
    workspace: PathBuf,
    entries: Vec<HostEntry>,
}

struct HostEntry {
    plugin: Box<dyn Plugin>,
    state: PluginState,
    enabled: bool,
}

impl PluginHost {
    /// Empty host (no plugins).
    pub fn empty(workspace: impl Into<PathBuf>) -> Self {
        Self {
            workspace: workspace.into(),
            entries: Vec::new(),
        }
    }

    /// Load enabled plugins from config, init them (register tools), then start.
    pub async fn load(
        workspace: impl Into<PathBuf>,
        config: &PluginsConfig,
        tools: &mut ToolRegistry,
    ) -> Result<Self> {
        let workspace = workspace.into();
        let mut host = Self {
            workspace: workspace.clone(),
            entries: Vec::new(),
        };

        if !config.enabled {
            info!("plugins disabled via config");
            return Ok(host);
        }

        for entry in config.enabled_entries() {
            let mut plugin = create_builtin(&entry.id).ok_or_else(|| {
                PluginError::Unknown(format!(
                    "{} (known builtins: {})",
                    entry.id,
                    builtin_ids().join(", ")
                ))
            })?;

            let id = plugin.meta().id.clone();
            let mut ctx = PluginContext::new(workspace.clone(), tools, entry.settings.clone());
            plugin
                .init(&mut ctx)
                .await
                .map_err(|e| PluginError::Lifecycle {
                    id: id.clone(),
                    message: e.to_string(),
                })?;

            host.entries.push(HostEntry {
                plugin,
                state: PluginState::Initialized,
                enabled: true,
            });
            info!(plugin = %id, "plugin initialized");
        }

        for entry in &mut host.entries {
            let id = entry.plugin.meta().id.clone();
            entry
                .plugin
                .start()
                .await
                .map_err(|e| PluginError::Lifecycle {
                    id: id.clone(),
                    message: e.to_string(),
                })?;
            entry.state = PluginState::Started;
            info!(plugin = %id, "plugin started");
        }

        Ok(host)
    }

    /// Stop all plugins in reverse order.
    pub async fn stop(&mut self) {
        for entry in self.entries.iter_mut().rev() {
            let id = entry.plugin.meta().id.clone();
            if let Err(e) = entry.plugin.stop().await {
                warn!(plugin = %id, error = %e, "plugin stop failed");
            }
            entry.state = PluginState::Stopped;
        }
    }

    /// Status rows for listing.
    pub fn status(&self) -> Vec<PluginStatus> {
        self.entries
            .iter()
            .map(|e| PluginStatus {
                meta: e.plugin.meta(),
                state: e.state,
                enabled: e.enabled,
            })
            .collect()
    }

    /// Number of loaded plugins.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no plugins are loaded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Workspace root.
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// Known builtin ids (not necessarily loaded).
    pub fn known_builtin_ids() -> &'static [&'static str] {
        builtin_ids()
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        // Best-effort: async stop is preferred via `stop()`. Drop cannot await.
        for entry in &self.entries {
            if entry.state == PluginState::Started {
                tracing::debug!(
                    plugin = %entry.plugin.meta().id,
                    "plugin still started at drop; call PluginHost::stop() for clean shutdown"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginsConfig;
    use cortex_tools::{ToolContext, ToolRegistry};
    use serde_json::json;

    #[tokio::test]
    async fn loads_echo_and_registers_tool() {
        let cfg = PluginsConfig::default();
        let mut tools = ToolRegistry::new();
        let mut host = PluginHost::load(std::env::temp_dir(), &cfg, &mut tools)
            .await
            .unwrap();
        assert_eq!(host.len(), 1);
        assert!(tools.contains("plugin_echo"));

        let tool = tools.get("plugin_echo").unwrap();
        let out = tool
            .execute(
                &ToolContext::for_tests(std::env::temp_dir()),
                json!({ "message": "hello" }),
            )
            .await
            .unwrap();
        assert_eq!(out, "hello");
        host.stop().await;
        assert_eq!(host.status()[0].state, PluginState::Stopped);
    }

    #[tokio::test]
    async fn echo_prefix_setting() {
        let cfg = PluginsConfig::from_toml(
            r#"
            [[plugins]]
            id = "echo"
            [plugins.settings]
            prefix = "P:"
            "#,
        )
        .unwrap();
        let mut tools = ToolRegistry::new();
        let host = PluginHost::load(std::env::temp_dir(), &cfg, &mut tools)
            .await
            .unwrap();
        let tool = tools.get("plugin_echo").unwrap();
        let out = tool
            .execute(
                &ToolContext::for_tests(std::env::temp_dir()),
                json!({ "message": "x" }),
            )
            .await
            .unwrap();
        assert_eq!(out, "P:x");
        drop(host);
    }

    #[tokio::test]
    async fn unknown_plugin_errors() {
        let cfg = PluginsConfig::from_toml(
            r#"
            [[plugins]]
            id = "does_not_exist"
            "#,
        )
        .unwrap();
        let mut tools = ToolRegistry::new();
        let result = PluginHost::load(std::env::temp_dir(), &cfg, &mut tools).await;
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("unknown"));
    }

    #[tokio::test]
    async fn master_disabled() {
        let cfg = PluginsConfig::from_toml(
            r#"
            enabled = false
            [[plugins]]
            id = "echo"
            "#,
        )
        .unwrap();
        let mut tools = ToolRegistry::new();
        let host = PluginHost::load(std::env::temp_dir(), &cfg, &mut tools)
            .await
            .unwrap();
        assert!(host.is_empty());
        assert!(!tools.contains("plugin_echo"));
    }
}
