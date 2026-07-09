//! Shared setup for CLI commands: config paths, providers, tools.

use anyhow::{bail, Context, Result};
use cortex_llm::{ModelsConfig, ProviderRegistry, ResolvedModel};
use cortex_tools::{
    register_default_tools, PermissionMode, PermissionPolicy, ToolContext, ToolExecutor,
    ToolRegistry,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::approver::CliApprover;

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
}

impl AppContext {
    /// Load config, providers, and tools.
    pub fn bootstrap(paths: Paths, yolo: bool) -> Result<Self> {
        let models = ModelsConfig::from_file(&paths.models_config)
            .with_context(|| format!("load {}", paths.models_config.display()))?;
        let registry = ProviderRegistry::from_config(&models)
            .context("build provider registry from models.toml")?;

        let mut tool_reg = ToolRegistry::new();
        register_default_tools(&mut tool_reg).context("register default tools")?;
        let tools = ToolExecutor::new(Arc::new(tool_reg));

        Ok(Self {
            paths,
            registry,
            tools,
            yolo,
        })
    }

    /// Resolve a model alias (or default).
    pub fn resolve_model(&self, alias: Option<&str>) -> Result<ResolvedModel> {
        self.registry
            .resolve(alias)
            .with_context(|| format!("resolve model alias {:?}", alias.unwrap_or("default")))
    }

    /// Build tool context for the workspace.
    pub fn tool_context(&self, cancel: CancellationToken) -> ToolContext {
        let mut policy = PermissionPolicy::default();
        if self.yolo {
            policy = policy.allow_all();
        }
        // Shell is still risky even for coding agents; keep Ask unless yolo.
        if !self.yolo {
            policy.tools.insert("shell".into(), PermissionMode::Ask);
            policy
                .tools
                .insert("write_file".into(), PermissionMode::Ask);
            policy.tools.insert("edit_file".into(), PermissionMode::Ask);
            policy
                .tools
                .insert("git_commit".into(), PermissionMode::Ask);
        }

        ToolContext {
            workspace_root: self.paths.workspace.clone(),
            session_id: None,
            cancel,
            permissions: Arc::new(policy),
            approver: Arc::new(CliApprover::new(self.yolo)),
            default_timeout: Duration::from_secs(60),
        }
    }
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
