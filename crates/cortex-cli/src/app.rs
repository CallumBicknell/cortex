//! Shared setup for CLI commands: config paths, providers, tools.

use anyhow::{bail, Context, Result};
use cortex_llm::{ModelsConfig, ProviderRegistry, ResolvedModel};
use cortex_mcp::{load_and_register_mcp, McpConfig};
use cortex_memory::{open_sqlite, SessionStore, VectorStore};
use cortex_plugins::{PluginHost, PluginsConfig};
use cortex_security::{PolicyApprover, SecurityPolicy};
use cortex_skills::SkillStore;
use cortex_tools::{
    register_default_tools_with_browser, register_memory_tools, register_skill_tools,
    BrowserConfig, BrowserHandle, MemoryHandle, SkillStoreHandle, ToolContext, ToolExecutor,
    ToolRegistry,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::approver::CliApprover;
use crate::db_audit::audit_sink_for;
use cortex_common::SessionId;

/// Resolved filesystem layout for a Cortex workspace invocation.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Workspace root (agent file operations).
    pub workspace: PathBuf,
    /// Models config path actually used.
    pub models_config: PathBuf,
    /// `.cortex` directory under workspace (may not exist until init).
    pub cortex_dir: PathBuf,
    /// SQLite database path.
    pub database: PathBuf,
}

impl Paths {
    /// Discover paths from optional overrides.
    pub fn resolve(workspace: Option<PathBuf>, models_config: Option<PathBuf>) -> Result<Self> {
        let workspace = workspace
            .unwrap_or_else(|| std::env::current_dir().expect("cwd"))
            .canonicalize()
            .context("failed to resolve workspace path")?;

        let cortex_dir = workspace.join(".cortex");

        let models_config = if let Some(p) = models_config {
            p.canonicalize()
                .context("failed to resolve --config path")?
        } else if let Ok(p) = std::env::var("CORTEX_MODELS_CONFIG") {
            PathBuf::from(p)
                .canonicalize()
                .context("CORTEX_MODELS_CONFIG path")?
        } else if cortex_dir.join("models.toml").is_file() {
            cortex_dir.join("models.toml")
        } else if Path::new("config/models.toml").is_file() {
            PathBuf::from("config/models.toml")
                .canonicalize()
                .context("config/models.toml")?
        } else {
            // Prefer workspace-relative defaults if present.
            let candidates = [
                workspace.join("config/models.toml"),
                workspace.join(".cortex/models.toml"),
            ];
            candidates
                .into_iter()
                .find(|p| p.is_file())
                .ok_or_else(|| {
                    anyhow::anyhow!("no models.toml found. Run `cortex init` or pass --config PATH")
                })?
        };

        let database = if let Ok(p) = std::env::var("CORTEX_DATABASE") {
            PathBuf::from(p)
        } else {
            cortex_dir.join("data").join("cortex.db")
        };

        Ok(Self {
            workspace,
            models_config,
            cortex_dir,
            database,
        })
    }
}

/// Shared agent wiring for run/chat.
pub struct AppContext {
    /// Paths.
    pub paths: Paths,
    /// Provider registry.
    pub registry: ProviderRegistry,
    /// Tool executor.
    pub tools: ToolExecutor,
    /// Whether auto-approve is on.
    pub yolo: bool,
    /// Loaded security policy.
    pub security: Arc<SecurityPolicy>,
    /// In-process plugin host (keeps plugins alive for the process).
    pub plugins: PluginHost,
}

impl AppContext {
    /// Load config, providers, tools, MCP servers, and security policy.
    pub async fn bootstrap(paths: Paths, yolo: bool) -> Result<Self> {
        let models = ModelsConfig::from_file(&paths.models_config)
            .with_context(|| format!("load {}", paths.models_config.display()))?;
        let registry = ProviderRegistry::from_config(&models)
            .context("build provider registry from models.toml")?;

        let mut tool_reg = ToolRegistry::new();
        let browser = load_browser_handle(&paths);
        register_default_tools_with_browser(&mut tool_reg, browser)
            .context("register default tools")?;

        if let Some(mcp_cfg) = load_mcp_config(&paths)? {
            let n = load_and_register_mcp(&mcp_cfg, &mut tool_reg).await;
            if n > 0 {
                info!(tools = n, "MCP tools registered");
            }
        }

        let plugins_cfg = load_plugins_config(&paths)?;
        let plugins = PluginHost::load(&paths.workspace, &plugins_cfg, &mut tool_reg)
            .await
            .context("load plugins")?;
        if !plugins.is_empty() {
            info!(count = plugins.len(), "plugins loaded");
        }

        // Memory tools when SQLite DB is available (created on first open).
        if let Ok(pool) = open_sqlite(&paths.database).await {
            let collection = paths.workspace.to_string_lossy().to_string();
            let handle = MemoryHandle::new(VectorStore::new(pool), collection);
            register_memory_tools(&mut tool_reg, handle);
            info!("memory_search tool registered");
        }

        // Self-evolving skills under .cortex/skills/
        let skill_store = SkillStore::for_workspace(&paths.workspace);
        register_skill_tools(&mut tool_reg, SkillStoreHandle::new(skill_store));
        info!("skill evolution tools registered");

        let tools = ToolExecutor::new(Arc::new(tool_reg));

        let security = load_security_policy(&paths, yolo)?;

        Ok(Self {
            paths,
            registry,
            tools,
            yolo,
            security: Arc::new(security),
            plugins,
        })
    }

    /// Open the session/vector store for this workspace.
    pub async fn open_store(&self) -> Result<SessionStore> {
        let pool = open_sqlite(&self.paths.database)
            .await
            .with_context(|| format!("open database {}", self.paths.database.display()))?;
        Ok(SessionStore::new(pool))
    }

    /// Vector store sharing the workspace database.
    pub async fn open_vector_store(&self) -> Result<VectorStore> {
        let pool = open_sqlite(&self.paths.database)
            .await
            .with_context(|| format!("open database {}", self.paths.database.display()))?;
        Ok(VectorStore::new(pool))
    }

    /// Resolve a model alias (or default).
    pub fn resolve_model(&self, alias: Option<&str>) -> Result<ResolvedModel> {
        self.registry
            .resolve(alias)
            .with_context(|| format!("resolve model alias {:?}", alias.unwrap_or("default")))
    }

    /// Build tool context for the workspace (optional DB audit + session id).
    pub fn tool_context(
        &self,
        cancel: CancellationToken,
        store: Option<&SessionStore>,
        session_id: Option<SessionId>,
    ) -> ToolContext {
        let mut sec = (*self.security).clone();
        if self.yolo {
            sec = sec.with_yolo(true);
        }
        let sec = Arc::new(sec);
        let policy = sec.to_permission_policy();
        let audit = audit_sink_for(store);
        let inner = Arc::new(CliApprover::new(sec.yolo));
        let approver = Arc::new(PolicyApprover::new(
            Arc::clone(&sec),
            inner,
            audit,
            session_id,
        ));

        ToolContext {
            workspace_root: self.paths.workspace.clone(),
            session_id,
            cancel,
            permissions: Arc::new(policy),
            approver,
            default_timeout: Duration::from_secs(sec.shell_timeout_secs.max(1)),
        }
    }
}

/// Load browser/CDP config (Obscura default).
pub fn load_browser_handle(paths: &Paths) -> BrowserHandle {
    let candidates = [
        std::env::var("CORTEX_BROWSER_CONFIG")
            .ok()
            .map(PathBuf::from),
        Some(paths.cortex_dir.join("browser.toml")),
        Some(PathBuf::from("config/browser.toml")),
        Some(paths.workspace.join("config/browser.toml")),
    ];
    for cand in candidates.into_iter().flatten() {
        if cand.is_file() {
            match BrowserConfig::from_file(&cand) {
                Ok(mut cfg) => {
                    // Env still wins for endpoint overrides.
                    let env = BrowserConfig::from_env_or_default();
                    if !env.cdp_url.is_empty() {
                        cfg.cdp_url = env.cdp_url;
                    }
                    if !env.discovery_url.is_empty() {
                        cfg.discovery_url = env.discovery_url;
                    }
                    if std::env::var("CORTEX_BROWSER_ENABLED").is_ok() {
                        cfg.enabled = env.enabled;
                    }
                    if std::env::var("CORTEX_BROWSER_BACKEND").is_ok() {
                        cfg.backend = env.backend;
                    }
                    info!(path = %cand.display(), backend = ?cfg.backend, "loaded browser config");
                    return BrowserHandle::new(cfg);
                }
                Err(err) => {
                    tracing::warn!(path = %cand.display(), error = %err, "invalid browser.toml");
                }
            }
        }
    }
    BrowserHandle::from_env_or_default()
}

/// Load plugins.toml (defaults when missing).
pub fn load_plugins_config(paths: &Paths) -> Result<PluginsConfig> {
    let candidates = [
        std::env::var("CORTEX_PLUGINS_CONFIG")
            .ok()
            .map(PathBuf::from),
        Some(paths.cortex_dir.join("plugins.toml")),
        Some(PathBuf::from("config/plugins.toml")),
        Some(paths.workspace.join("config/plugins.toml")),
    ];
    for cand in candidates.into_iter().flatten() {
        if cand.is_file() {
            let cfg = PluginsConfig::from_file(&cand)
                .map_err(|e| anyhow::anyhow!("load plugins config {}: {e}", cand.display()))?;
            info!(path = %cand.display(), "loaded plugins config");
            return Ok(cfg);
        }
    }
    Ok(PluginsConfig::default())
}

/// Load MCP config if present (optional).
pub fn load_mcp_config(paths: &Paths) -> Result<Option<McpConfig>> {
    let candidates = [
        std::env::var("CORTEX_MCP_CONFIG").ok().map(PathBuf::from),
        Some(paths.cortex_dir.join("mcp.toml")),
        Some(PathBuf::from("config/mcp.toml")),
        Some(paths.workspace.join("config/mcp.toml")),
    ];
    for cand in candidates.into_iter().flatten() {
        if cand.is_file() {
            let cfg = McpConfig::from_file(&cand)
                .with_context(|| format!("load MCP config {}", cand.display()))?;
            return Ok(Some(cfg));
        }
    }
    Ok(None)
}

/// Load security.toml from env, `.cortex/security.toml`, or `config/security.toml`.
pub fn load_security_policy(paths: &Paths, yolo: bool) -> Result<SecurityPolicy> {
    let candidates = [
        std::env::var("CORTEX_SECURITY_CONFIG")
            .ok()
            .map(PathBuf::from),
        Some(paths.cortex_dir.join("security.toml")),
        Some(PathBuf::from("config/security.toml")),
        Some(paths.workspace.join("config/security.toml")),
    ];
    for cand in candidates.into_iter().flatten() {
        if cand.is_file() {
            let mut policy = SecurityPolicy::from_file(&cand)
                .with_context(|| format!("load security policy {}", cand.display()))?;
            if yolo {
                policy = policy.with_yolo(true);
            }
            return Ok(policy);
        }
    }
    Ok(SecurityPolicy::default().with_yolo(yolo))
}

/// Load `.env` from cwd if present (non-fatal).
pub fn load_dotenv() {
    let _ = dotenvy::dotenv();
}

/// Init tracing from RUST_LOG / CORTEX_LOG_LEVEL.
pub fn init_tracing(verbose: bool) {
    let default = if verbose { "debug" } else { "info" };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = std::env::var("CORTEX_LOG_LEVEL").unwrap_or_else(|_| default.into());
        tracing_subscriber::EnvFilter::new(format!("cortex={level},cortex_cli={level}"))
    });
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

/// Ensure a directory exists.
pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("create directory {}", path.display()))?;
    Ok(())
}

/// Copy default models.toml template into destination if missing.
pub fn write_default_models_toml(dest: &Path) -> Result<()> {
    if dest.exists() {
        bail!("{} already exists", dest.display());
    }
    if let Some(parent) = dest.parent() {
        ensure_dir(parent)?;
    }
    let template = include_str!("../../../config/models.toml");
    std::fs::write(dest, template).with_context(|| format!("write {}", dest.display()))?;
    Ok(())
}
