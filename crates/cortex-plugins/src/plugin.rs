//! Plugin trait and host-facing context.

use crate::error::Result;
use async_trait::async_trait;
use cortex_tools::ToolRegistry;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Static metadata for a plugin.
#[derive(Debug, Clone)]
pub struct PluginMeta {
    /// Unique id (stable, used in config).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Semver-ish version string.
    pub version: String,
    /// Short description.
    pub description: String,
}

impl PluginMeta {
    /// Construct metadata.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            description: description.into(),
        }
    }
}

/// Mutable context passed to plugins during init (can register tools).
pub struct PluginContext<'a> {
    /// Workspace root.
    pub workspace: PathBuf,
    /// Tool registry shared with the agent loop.
    pub tools: &'a mut ToolRegistry,
    /// Plugin-specific settings from `plugins.toml`.
    pub settings: Value,
}

impl<'a> PluginContext<'a> {
    /// Create a context.
    pub fn new(
        workspace: impl Into<PathBuf>,
        tools: &'a mut ToolRegistry,
        settings: Value,
    ) -> Self {
        Self {
            workspace: workspace.into(),
            tools,
            settings,
        }
    }

    /// Workspace path.
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }
}

/// In-process plugin. Dynamic loading (cdylib) is intentionally out of scope for v0.1.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Static metadata.
    fn meta(&self) -> PluginMeta;

    /// Initialize: register tools, validate settings. Called once before `start`.
    async fn init(&mut self, ctx: &mut PluginContext<'_>) -> Result<()>;

    /// Start after all plugins have initialized. Default: no-op.
    async fn start(&mut self) -> Result<()> {
        Ok(())
    }

    /// Stop during shutdown (reverse order). Default: no-op.
    async fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}
