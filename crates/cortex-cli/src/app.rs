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

/// Embedded default models.toml (same content as repo `config/models.toml`).
pub const EMBEDDED_MODELS_TOML: &str = include_str!("../../../config/models.toml");

/// Resolve the user-global Cortex home directory.
///
/// Precedence: `CORTEX_HOME` → `~/.cortex` (via HOME / USERPROFILE / dirs).
pub fn cortex_home() -> PathBuf {
    if let Ok(p) = std::env::var("CORTEX_HOME") {
        let p = p.trim();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home).join(".cortex");
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        if !profile.is_empty() {
            return PathBuf::from(profile).join(".cortex");
        }
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".cortex");
    }
    // Last resort for exotic environments (tests should set CORTEX_HOME).
    PathBuf::from(".cortex-home")
}

/// Ensure `~/.cortex` layout exists. Writes models.toml from embedded template if missing.
///
/// Never overwrites existing files unless `force` is true (models.toml only).
pub fn bootstrap_home(home: &Path, force: bool) -> Result<BootstrapReport> {
    let mut report = BootstrapReport::default();
    for sub in ["skills", "prompts", "plugins", "data", "cache", "logs"] {
        let p = home.join(sub);
        if !p.is_dir() {
            ensure_dir(&p)?;
            report.created_dirs.push(p);
        }
    }

    let models = home.join("models.toml");
    if models.is_file() && !force {
        report.models_path = models;
        report.models_written = false;
    } else {
        if models.is_file() && force {
            std::fs::remove_file(&models)
                .with_context(|| format!("remove {}", models.display()))?;
        }
        if let Some(parent) = models.parent() {
            ensure_dir(parent)?;
        }
        std::fs::write(&models, EMBEDDED_MODELS_TOML)
            .with_context(|| format!("write {}", models.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&models, std::fs::Permissions::from_mode(0o644));
        }
        report.models_path = models;
        report.models_written = true;
    }

    // Optional .env example note (do not overwrite secrets).
    let env_example = home.join(".env.example");
    if !env_example.is_file() {
        let body = "# Optional: copy to .env and chmod 600\n\
                    # OPENAI_API_KEY=\n\
                    # ANTHROPIC_API_KEY=\n\
                    # OPENROUTER_API_KEY=\n\
                    # CORTEX_LOG_LEVEL=info\n";
        std::fs::write(&env_example, body)
            .with_context(|| format!("write {}", env_example.display()))?;
        report.created_dirs.push(env_example);
    }

    Ok(report)
}

/// Result of [`bootstrap_home`].
#[derive(Debug, Default, Clone)]
pub struct BootstrapReport {
    /// Path to models.toml under home.
    pub models_path: PathBuf,
    /// Whether models.toml was written this call.
    pub models_written: bool,
    /// Directories or auxiliary files created.
    pub created_dirs: Vec<PathBuf>,
}

/// Resolved filesystem layout for a Cortex workspace invocation.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Workspace root (agent file operations).
    pub workspace: PathBuf,
    /// User-global Cortex home (`~/.cortex` or `CORTEX_HOME`).
    pub home: PathBuf,
    /// Models config path actually used.
    pub models_config: PathBuf,
    /// `.cortex` directory under workspace (may not exist until init).
    pub cortex_dir: PathBuf,
    /// SQLite database path.
    pub database: PathBuf,
}

impl Paths {
    /// Discover paths from optional overrides.
    ///
    /// Config precedence for models.toml:
    /// `--config` / `CORTEX_MODELS_CONFIG` → project `.cortex` → home → monorepo `config/` → auto-bootstrap home.
    pub fn resolve(workspace: Option<PathBuf>, models_config: Option<PathBuf>) -> Result<Self> {
        let workspace = workspace
            .unwrap_or_else(|| std::env::current_dir().expect("cwd"))
            .canonicalize()
            .context("failed to resolve workspace path")?;

        let home = cortex_home();
        let cortex_dir = workspace.join(".cortex");

        let models_config = if let Some(p) = models_config {
            p.canonicalize()
                .context("failed to resolve --config path")?
        } else if let Ok(p) = std::env::var("CORTEX_MODELS_CONFIG") {
            PathBuf::from(p)
                .canonicalize()
                .context("CORTEX_MODELS_CONFIG path")?
        } else if let Some(p) = first_existing(&[
            cortex_dir.join("models.toml"),
            home.join("models.toml"),
            PathBuf::from("config/models.toml"),
            workspace.join("config/models.toml"),
        ]) {
            canonicalize_if_possible(&p)
        } else {
            // Auto-bootstrap user home so a installed binary works outside the monorepo.
            let report = bootstrap_home(&home, false)?;
            info!(
                path = %report.models_path.display(),
                written = report.models_written,
                "bootstrapped cortex home models.toml"
            );
            report.models_path
        };

        let database = if let Ok(p) = std::env::var("CORTEX_DATABASE") {
            PathBuf::from(p)
        } else if cortex_dir.is_dir() {
            cortex_dir.join("data").join("cortex.db")
        } else {
            home.join("data").join("cortex.db")
        };

        Ok(Self {
            workspace,
            home,
            models_config,
            cortex_dir,
            database,
        })
    }

    /// Resolve a named config file with standard precedence.
    ///
    /// Order: optional env path → project `.cortex/<name>` → home `<name>` →
    /// cwd `config/<name>` → workspace `config/<name>`.
    pub fn resolve_config_file(&self, name: &str, env_var: &str) -> Option<PathBuf> {
        if let Ok(p) = std::env::var(env_var) {
            let p = PathBuf::from(p);
            if p.is_file() {
                return Some(p);
            }
        }
        first_existing(&[
            self.cortex_dir.join(name),
            self.home.join(name),
            PathBuf::from("config").join(name),
            self.workspace.join("config").join(name),
        ])
    }

    /// Prompt dirs: home then monorepo/workspace then project (later load overrides).
    pub fn prompt_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.home.join("prompts"),
            self.workspace.join("prompts"),
            self.cortex_dir.join("prompts"),
        ]
    }
}

fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}

fn canonicalize_if_possible(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
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
        match open_sqlite(&paths.database).await {
            Ok(pool) => {
                let collection = paths.workspace.to_string_lossy().to_string();
                let handle = MemoryHandle::new(VectorStore::new(pool), collection);
                register_memory_tools(&mut tool_reg, handle);
                info!("memory_search tool registered");
            }
            Err(e) => {
                tracing::warn!(
                    path = %paths.database.display(),
                    error = %e,
                    "sqlite memory store unavailable; memory_search not registered"
                );
            }
        }

        // Self-evolving skills write to project .cortex/skills/ (create on save).
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
    if let Some(cand) = paths.resolve_config_file("browser.toml", "CORTEX_BROWSER_CONFIG") {
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
    BrowserHandle::from_env_or_default()
}

/// Load plugins.toml (defaults when missing).
pub fn load_plugins_config(paths: &Paths) -> Result<PluginsConfig> {
    if let Some(cand) = paths.resolve_config_file("plugins.toml", "CORTEX_PLUGINS_CONFIG") {
        let cfg = PluginsConfig::from_file(&cand)
            .map_err(|e| anyhow::anyhow!("load plugins config {}: {e}", cand.display()))?;
        info!(path = %cand.display(), "loaded plugins config");
        return Ok(cfg);
    }
    Ok(PluginsConfig::default())
}

/// Load MCP config if present (optional).
pub fn load_mcp_config(paths: &Paths) -> Result<Option<McpConfig>> {
    if let Some(cand) = paths.resolve_config_file("mcp.toml", "CORTEX_MCP_CONFIG") {
        let cfg = McpConfig::from_file(&cand)
            .with_context(|| format!("load MCP config {}", cand.display()))?;
        return Ok(Some(cfg));
    }
    Ok(None)
}

/// Load security.toml from env, project/home `.cortex`, or monorepo `config/`.
pub fn load_security_policy(paths: &Paths, yolo: bool) -> Result<SecurityPolicy> {
    if let Some(cand) = paths.resolve_config_file("security.toml", "CORTEX_SECURITY_CONFIG") {
        let mut policy = SecurityPolicy::from_file(&cand)
            .with_context(|| format!("load security policy {}", cand.display()))?;
        if yolo {
            policy = policy.with_yolo(true);
        }
        return Ok(policy);
    }
    Ok(SecurityPolicy::default().with_yolo(yolo))
}

/// Load env files: home `.env` first (fill missing), then cwd `.env` (overrides).
pub fn load_dotenv() {
    let home_env = cortex_home().join(".env");
    if home_env.is_file() {
        let _ = dotenvy::from_path(&home_env);
    }
    // Cwd `.env` wins over home for keys present in both.
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
    std::fs::write(dest, EMBEDDED_MODELS_TOML)
        .with_context(|| format!("write {}", dest.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cortex_home_respects_env() {
        let dir = tempdir().unwrap();
        let custom = dir.path().join("my-home");
        std::env::set_var("CORTEX_HOME", &custom);
        assert_eq!(cortex_home(), custom);
        std::env::remove_var("CORTEX_HOME");
    }

    #[test]
    fn bootstrap_writes_models_once() {
        let dir = tempdir().unwrap();
        let home = dir.path().join("home");
        let r1 = bootstrap_home(&home, false).unwrap();
        assert!(r1.models_written);
        assert!(r1.models_path.is_file());
        let r2 = bootstrap_home(&home, false).unwrap();
        assert!(!r2.models_written);
        let body = std::fs::read_to_string(&r2.models_path).unwrap();
        assert!(body.contains("default_model"));
    }

    #[test]
    fn resolve_prefers_project_models_over_home() {
        let dir = tempdir().unwrap();
        let home = dir.path().join("home");
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(ws.join(".cortex")).unwrap();
        bootstrap_home(&home, false).unwrap();
        std::fs::write(
            ws.join(".cortex/models.toml"),
            "default_model = \"project\"\n\n[providers.mock]\nkind = \"mock\"\n\n[models.project]\nprovider = \"mock\"\nmodel = \"p\"\n",
        )
        .unwrap();
        std::env::set_var("CORTEX_HOME", &home);
        let paths = Paths::resolve(Some(ws.clone()), None).unwrap();
        assert!(paths
            .models_config
            .ends_with(Path::new(".cortex/models.toml")));
        assert_eq!(paths.database, ws.join(".cortex/data/cortex.db"));
        std::env::remove_var("CORTEX_HOME");
    }

    #[test]
    fn resolve_db_falls_back_to_home_without_project_cortex() {
        let dir = tempdir().unwrap();
        let home = dir.path().join("home");
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        bootstrap_home(&home, false).unwrap();
        std::env::set_var("CORTEX_HOME", &home);
        // Clear any ambient CORTEX_DATABASE from the test runner.
        std::env::remove_var("CORTEX_DATABASE");
        let paths = Paths::resolve(Some(ws), None).unwrap();
        assert_eq!(paths.database, home.join("data/cortex.db"));
        assert_eq!(paths.models_config, home.join("models.toml"));
        std::env::remove_var("CORTEX_HOME");
    }
}
