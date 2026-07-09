//! Shared API server state.

use cortex_llm::ProviderRegistry;
use cortex_memory::SessionStore;
use cortex_tools::ToolExecutor;
use std::path::PathBuf;
use std::sync::Arc;

/// Shared state for all HTTP handlers.
pub struct ApiState {
    /// Workspace root for agent tools.
    pub workspace: PathBuf,
    /// Models config path (display).
    pub models_config: PathBuf,
    /// Database path (display).
    pub database: PathBuf,
    /// Provider registry.
    pub registry: ProviderRegistry,
    /// Tool executor.
    pub tools: ToolExecutor,
    /// Session store.
    pub store: SessionStore,
    /// Default auto-approve for runs when request omits yolo.
    pub default_yolo: bool,
    /// Default max turns.
    pub default_max_turns: u32,
    /// Optional bearer token (None = open, localhost intended).
    pub api_token: Option<String>,
    /// Server version string.
    pub version: String,
}

impl ApiState {
    /// Whether auth is required and the header matches.
    pub fn authorize(&self, auth_header: Option<&str>) -> bool {
        let Some(expected) = &self.api_token else {
            return true;
        };
        match auth_header {
            Some(h) if h == expected => true,
            Some(h) if h.strip_prefix("Bearer ") == Some(expected.as_str()) => true,
            _ => false,
        }
    }
}

/// Thin wrapper so axum can extract Arc state.
pub type SharedState = Arc<ApiState>;
