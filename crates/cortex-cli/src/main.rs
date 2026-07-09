//! Cortex CLI — `cortex run`, `cortex chat`, sessions, and helpers.

mod app;
mod approver;
mod db_audit;
mod setup_config;
mod setup_tui;

use anyhow::{Context, Result};
use app::{
    bootstrap_home, cortex_home, init_tracing, load_dotenv, write_default_models_toml, AppContext,
    Paths,
};
use clap::{Parser, Subcommand};
use cortex_common::SessionId;
use cortex_core::{EnvelopeHandler, EventBus, EventEnvelope, InMemoryEventBus};
use cortex_memory::{open_sqlite, CheckpointState, SessionStore, VectorStore};
use cortex_models::{Session, SessionStatus, TaskStatus};
use cortex_prompts::PromptCatalog;
use cortex_runtime::{
    maybe_summarize, tools_with_subagent, AgentLoop, AgentLoopConfig, ContextBuilder, RunInput,
    RunOutput, SummarizeConfig,
};
use cortex_security::{redact_text, SecurityPolicy};
use cortex_skills::{
    import_from_markdown, read_skill_source, select_skills, write_imported_skill, ImportOptions,
    SkillRegistry, SkillStore,
};
use cortex_workspace::RepoMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Parser)]
#[command(
    name = "cortex",
    version,
    about = "Cortex — an operating system for AI agents",
    long_about = "Run autonomous coding agents with pluggable providers and tools.\n\n\
                  Examples:\n  \
                  cortex setup\n  \
                  cortex doctor\n  \
                  cortex init\n  \
                  cortex run \"Add a README section about config\"\n  \
                  cortex chat --model ollama\n  \
                  cortex sessions list\n  \
                  cortex skills list\n  \
                  cortex run \"…\" --skills rust,git"
)]
struct Cli {
    /// Workspace root (default: current directory).
    #[arg(long, global = true)]
    workspace: Option<PathBuf>,

    /// Path to models.toml.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Verbose logging.
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Create user-global `~/.cortex` (models, skills, data). Safe to re-run.
    ///
    /// On a TTY, launches a full-screen setup wizard (unless `--no-wizard` or
    /// non-interactive flags are set).
    Setup {
        /// Overwrite home models.toml if it exists.
        #[arg(long)]
        force: bool,
        /// Force the TUI setup wizard (requires a TTY).
        #[arg(long)]
        wizard: bool,
        /// Skip the TUI wizard even on a TTY (bootstrap files only).
        #[arg(long)]
        no_wizard: bool,
        /// Set default model without TUI: default|ollama|openai|anthropic|openrouter.
        #[arg(long, value_name = "ALIAS")]
        default_model: Option<String>,
        /// When default is ollama, set the Ollama model id (e.g. llama3.2).
        #[arg(long, value_name = "MODEL")]
        ollama_model: Option<String>,
    },
    /// Print install health: paths, config, env key presence (no secrets).
    Doctor,
    /// Create project `.cortex/` config in the workspace.
    Init {
        /// Overwrite models.toml if it exists.
        #[arg(long)]
        force: bool,
        /// Scaffold Foundry/Web3 MCP + skill hints under `.cortex/`.
        #[arg(long)]
        web3: bool,
    },
    /// Reinstall / print how to update the cortex binary (Unix).
    Update {
        /// Print the install command only (do not execute).
        #[arg(long)]
        dry_run: bool,
    },
    /// Run a single agent task and exit.
    Run {
        /// User prompt / task description.
        prompt: String,
        /// Model alias from models.toml (default: configured default).
        #[arg(long, short)]
        model: Option<String>,
        /// Stream assistant text tokens to stderr (when the provider supports it).
        #[arg(long)]
        stream: bool,
        /// Auto-approve all tools (dangerous).
        #[arg(long)]
        yolo: bool,
        /// Maximum LLM turns.
        #[arg(long, default_value_t = 32)]
        max_turns: u32,
        /// Emit machine-readable JSON summary on stdout.
        #[arg(long)]
        json: bool,
        /// Resume an existing session id instead of creating a new one.
        #[arg(long)]
        session: Option<String>,
        /// Disable SQLite persistence for this run.
        #[arg(long)]
        no_save: bool,
        /// Comma-separated skill ids (default: auto-select from prompt + project).
        #[arg(long, value_delimiter = ',')]
        skills: Vec<String>,
        /// Plan mode: prefer read/plan before writes; inject plan guidance.
        #[arg(long)]
        plan: bool,
        /// After file edits, run the project test command (from project detect or --verify-cmd).
        #[arg(long)]
        verify: bool,
        /// Override verify shell command (implies --verify).
        #[arg(long)]
        verify_cmd: Option<String>,
    },
    /// Interactive multi-turn chat REPL.
    Chat {
        /// Model alias.
        #[arg(long, short)]
        model: Option<String>,
        /// Auto-approve tools.
        #[arg(long)]
        yolo: bool,
        /// Maximum LLM turns per user message.
        #[arg(long, default_value_t = 32)]
        max_turns: u32,
        /// Resume session id.
        #[arg(long)]
        session: Option<String>,
        /// Comma-separated skill ids (default: auto).
        #[arg(long, value_delimiter = ',')]
        skills: Vec<String>,
        /// Plan mode for each turn.
        #[arg(long)]
        plan: bool,
        /// After file edits, run project tests.
        #[arg(long)]
        verify: bool,
        /// Override verify shell command (implies --verify).
        #[arg(long)]
        verify_cmd: Option<String>,
    },
    /// Tool helpers.
    Tools {
        #[command(subcommand)]
        command: ToolsCmd,
    },
    /// Model helpers.
    Models {
        #[command(subcommand)]
        command: ModelsCmd,
    },
    /// Durable session store helpers.
    Sessions {
        #[command(subcommand)]
        command: SessionsCmd,
    },
    /// Workspace inspection (repo map, project detect).
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCmd,
    },
    /// Skill packs (capability catalogs — not hard-coded modes).
    Skills {
        #[command(subcommand)]
        command: SkillsCmd,
    },
    /// Security policy helpers.
    Security {
        #[command(subcommand)]
        command: SecurityCmd,
    },
    /// In-process plugins (builtin factory).
    Plugins {
        #[command(subcommand)]
        command: PluginsCmd,
    },
    /// Vector memory index and search.
    Memory {
        #[command(subcommand)]
        command: MemoryCmd,
    },
    /// Tree-sitter code outlines (Rust / Python).
    Parse {
        #[command(subcommand)]
        command: ParseCmd,
    },
    /// Run evaluation fixtures (mock agent scoring).
    Eval {
        #[command(subcommand)]
        command: EvalCmd,
    },
    /// Start the HTTP API server.
    Serve {
        /// Bind address (host:port).
        #[arg(long, default_value = "127.0.0.1:8080")]
        bind: String,
        /// Auto-approve tools for API runs by default.
        #[arg(long, default_value_t = true)]
        yolo: bool,
        /// Disable default yolo for API runs.
        #[arg(long)]
        no_yolo: bool,
        /// Default max turns for runs.
        #[arg(long, default_value_t = 32)]
        max_turns: u32,
        /// Optional API bearer token (or set CORTEX_API_TOKEN).
        #[arg(long, env = "CORTEX_API_TOKEN")]
        token: Option<String>,
    },
    /// Interactive terminal UI (chat, sessions, tool log).
    Tui {
        /// Model alias.
        #[arg(long, short)]
        model: Option<String>,
        /// Auto-approve tools (recommended for TUI).
        #[arg(long, default_value_t = true)]
        yolo: bool,
        /// Require tool approval (disables default yolo).
        #[arg(long)]
        no_yolo: bool,
        /// Maximum LLM turns per message.
        #[arg(long, default_value_t = 32)]
        max_turns: u32,
        /// Comma-separated skill ids (default: auto).
        #[arg(long, value_delimiter = ',')]
        skills: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum EvalCmd {
    /// List fixtures in a directory.
    List {
        /// Directory of `*.toml` fixtures (default: `evals/`).
        #[arg(long, default_value = "evals")]
        dir: PathBuf,
    },
    /// Run all fixtures and print a report.
    Run {
        /// Directory of `*.toml` fixtures (default: `evals/`).
        #[arg(long, default_value = "evals")]
        dir: PathBuf,
        /// Emit JSON report.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ToolsCmd {
    /// List registered builtin tools.
    List,
}

#[derive(Debug, Subcommand)]
enum ModelsCmd {
    /// List configured model aliases and providers.
    List,
}

#[derive(Debug, Subcommand)]
enum WorkspaceCmd {
    /// Print detected project info.
    Info,
    /// Print the repo map used in agent context.
    Map {
        /// Max files to index.
        #[arg(long, default_value_t = 400)]
        max_files: usize,
    },
}

#[derive(Debug, Subcommand)]
enum SkillsCmd {
    /// List builtin and learned skills.
    List,
    /// Show which skills would activate for a prompt.
    Select {
        /// User prompt / task text.
        prompt: String,
        /// Optional explicit skill ids.
        #[arg(long, value_delimiter = ',')]
        skills: Vec<String>,
    },
    /// Import a SKILL.md (or directory containing it) from a path or https URL.
    /// Does not auto-run on startup — explicit only.
    Import {
        /// Local path or https:// URL to SKILL.md (or a directory with SKILL.md).
        source: String,
        /// Override skill id ([A-Za-z0-9_-]+).
        #[arg(long)]
        id: Option<String>,
        /// Comma-separated tool allow-list (default: coding + shell + web tools).
        #[arg(long, value_delimiter = ',')]
        tools: Vec<String>,
        /// Extra tags (comma-separated).
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Parse and print without writing files.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SecurityCmd {
    /// Show the effective security policy.
    Show,
}

#[derive(Debug, Subcommand)]
enum PluginsCmd {
    /// List loaded plugins and known builtins.
    List,
}

#[derive(Debug, Subcommand)]
enum ParseCmd {
    /// Print a symbol outline for a source file.
    Outline {
        /// File path (relative to workspace or absolute).
        path: PathBuf,
        /// Emit JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum MemoryCmd {
    /// Index workspace text files into the local vector store.
    Index {
        /// Max files to index.
        #[arg(long, default_value_t = 200)]
        max_files: usize,
        /// Clear collection before indexing.
        #[arg(long)]
        clear: bool,
    },
    /// Semantic search the memory index.
    Search {
        /// Query text.
        query: String,
        /// Number of hits.
        #[arg(long, short = 'k', default_value_t = 5)]
        top_k: usize,
    },
    /// Show index stats.
    Stats,
    /// Summarize a session (LLM or extractive) and store the result.
    Summarize {
        /// Session id.
        session: String,
        /// Use extractive-only (no LLM call).
        #[arg(long)]
        extractive: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SessionsCmd {
    /// List recent sessions.
    List {
        /// Max rows.
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Show a session (messages).
    Show {
        /// Session id (UUID).
        id: String,
    },
    /// Resume a session into interactive chat.
    Resume {
        /// Session id.
        id: String,
        /// Model alias override.
        #[arg(long, short)]
        model: Option<String>,
        /// Auto-approve tools.
        #[arg(long)]
        yolo: bool,
        /// Max turns per message.
        #[arg(long, default_value_t = 32)]
        max_turns: u32,
    },
    /// Export a session as JSON.
    Export {
        /// Session id.
        id: String,
        /// Output file (default: stdout).
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Archive a session.
    Archive {
        /// Session id.
        id: String,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    load_dotenv();
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match run(cli).await {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(1)
        }
    }
}

async fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Commands::Setup {
            force,
            wizard,
            no_wizard,
            default_model,
            ollama_model,
        } => {
            cmd_setup(force, wizard, no_wizard, default_model, ollama_model)?;
        }
        Commands::Doctor => {
            cmd_doctor(cli.workspace, cli.config)?;
        }
        Commands::Init { force, web3 } => {
            cmd_init(cli.workspace, force, web3).await?;
        }
        Commands::Update { dry_run } => {
            cmd_update(dry_run)?;
        }
        Commands::Run {
            prompt,
            model,
            stream,
            yolo,
            max_turns,
            json,
            session,
            no_save,
            skills,
            plan,
            verify,
            verify_cmd,
        } => {
            return cmd_run(
                cli.workspace,
                cli.config,
                prompt,
                model,
                stream,
                yolo,
                max_turns,
                json,
                session,
                no_save,
                skills,
                plan,
                verify,
                verify_cmd,
            )
            .await;
        }
        Commands::Chat {
            model,
            yolo,
            max_turns,
            session,
            skills,
            plan,
            verify,
            verify_cmd,
        } => {
            return cmd_chat(
                cli.workspace,
                cli.config,
                model,
                yolo,
                max_turns,
                session,
                skills,
                plan,
                verify,
                verify_cmd,
            )
            .await;
        }
        Commands::Tools { command } => match command {
            ToolsCmd::List => {
                return cmd_tools_list(cli.workspace, cli.config).await;
            }
        },
        Commands::Models { command } => match command {
            ModelsCmd::List => cmd_models_list(cli.workspace, cli.config).await?,
        },
        Commands::Sessions { command } => {
            return cmd_sessions(cli.workspace, cli.config, command).await;
        }
        Commands::Workspace { command } => {
            cmd_workspace(cli.workspace, command)?;
        }
        Commands::Skills { command } => {
            cmd_skills(cli.workspace, command).await?;
        }
        Commands::Security { command } => {
            cmd_security(cli.workspace, cli.config, command)?;
        }
        Commands::Plugins { command } => {
            return cmd_plugins(cli.workspace, cli.config, command).await;
        }
        Commands::Memory { command } => {
            return cmd_memory(cli.workspace, cli.config, command).await;
        }
        Commands::Parse { command } => {
            cmd_parse(cli.workspace, command)?;
        }
        Commands::Tui {
            model,
            yolo,
            no_yolo,
            max_turns,
            skills,
        } => {
            return cmd_tui(
                cli.workspace,
                cli.config,
                model,
                yolo && !no_yolo,
                max_turns,
                skills,
            )
            .await;
        }
        Commands::Serve {
            bind,
            yolo,
            no_yolo,
            max_turns,
            token,
        } => {
            return cmd_serve(
                cli.workspace,
                cli.config,
                bind,
                yolo && !no_yolo,
                max_turns,
                token,
            )
            .await;
        }
        Commands::Eval { command } => {
            return cmd_eval(cli.workspace, command).await;
        }
    }
    Ok(ExitCode::SUCCESS)
}

async fn cmd_eval(workspace: Option<PathBuf>, command: EvalCmd) -> Result<ExitCode> {
    let root = workspace.unwrap_or_else(|| std::env::current_dir().expect("cwd"));
    match command {
        EvalCmd::List { dir } => {
            let dir = if dir.is_absolute() {
                dir
            } else {
                root.join(dir)
            };
            let paths = cortex_eval::discover_fixtures(&dir)
                .with_context(|| format!("discover {}", dir.display()))?;
            if paths.is_empty() {
                println!("no fixtures in {}", dir.display());
            } else {
                for p in paths {
                    match cortex_eval::load_fixture(&p) {
                        Ok(f) => println!("{:<16}  {}", f.id, f.description),
                        Err(e) => println!("{}  (error: {e})", p.display()),
                    }
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        EvalCmd::Run { dir, json } => {
            let dir = if dir.is_absolute() {
                dir
            } else {
                root.join(dir)
            };
            let report = cortex_eval::run_suite(&dir)
                .await
                .with_context(|| format!("run suite {}", dir.display()))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for c in &report.cases {
                    let mark = if c.passed { "PASS" } else { "FAIL" };
                    println!(
                        "[{mark}] {:<16} turns={} {}ms",
                        c.id, c.turns, c.duration_ms
                    );
                    for f in &c.failures {
                        println!("       - {f}");
                    }
                }
                println!(
                    "\n{} passed, {} failed (of {})",
                    report.passed,
                    report.failed,
                    report.cases.len()
                );
            }
            if report.all_passed() {
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::from(1))
            }
        }
    }
}

async fn cmd_serve(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    bind: String,
    yolo: bool,
    max_turns: u32,
    token: Option<String>,
) -> Result<ExitCode> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths, yolo).await?;
    let store = app.open_store().await?;
    let addr: std::net::SocketAddr = bind
        .parse()
        .with_context(|| format!("invalid --bind address: {bind}"))?;

    let state = cortex_api::ApiState {
        workspace: app.paths.workspace.clone(),
        models_config: app.paths.models_config.clone(),
        database: app.paths.database.clone(),
        registry: app.registry,
        tools: app.tools,
        store,
        default_yolo: yolo,
        default_max_turns: max_turns,
        api_token: token.filter(|t| !t.is_empty()),
        version: env!("CARGO_PKG_VERSION").into(),
    };

    eprintln!("Cortex API listening on http://{addr}");
    eprintln!("  GET  /health");
    eprintln!("  GET  /v1/info /v1/models /v1/tools /v1/sessions");
    eprintln!("  POST /v1/runs");
    if state.api_token.is_some() {
        eprintln!("  auth: bearer token required");
    } else {
        eprintln!("  auth: open (set --token or CORTEX_API_TOKEN to require auth)");
    }

    cortex_api::serve(state, addr)
        .await
        .context("HTTP server")?;
    Ok(ExitCode::SUCCESS)
}

async fn cmd_tui(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    model: Option<String>,
    yolo: bool,
    max_turns: u32,
    skills: Vec<String>,
) -> Result<ExitCode> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths, yolo).await?;
    let resolved = app.resolve_model(model.as_deref())?;
    let store = app.open_store().await?;

    // Avoid tracing noise over the alternate screen.
    let host = cortex_tui::TuiHost {
        workspace: app.paths.workspace.clone(),
        database: app.paths.database.clone(),
        model_alias: resolved.alias.clone(),
        provider_id: resolved.provider_id.clone(),
        model: resolved.model.clone(),
        provider: Arc::clone(&resolved.provider),
        tools: app.tools.clone(),
        store,
        max_turns,
        yolo,
        skills,
    };

    if let Err(e) = cortex_tui::run(host).await {
        eprintln!("tui error: {e:#}");
        return Ok(ExitCode::from(1));
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_parse(workspace: Option<PathBuf>, command: ParseCmd) -> Result<()> {
    match command {
        ParseCmd::Outline { path, json } => {
            let paths = Paths::resolve(workspace, None)?;
            let full = if path.is_absolute() {
                path
            } else {
                paths.workspace.join(&path)
            };
            let outline = cortex_parse::outline_file(&full)
                .with_context(|| format!("outline {}", full.display()))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&outline)?);
            } else {
                print!("{}", cortex_parse::format_outline(&outline));
            }
        }
    }
    Ok(())
}

async fn cmd_memory(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    command: MemoryCmd,
) -> Result<ExitCode> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths, false).await?;
    let collection = app.paths.workspace.to_string_lossy().to_string();
    let store = app.open_vector_store().await?;

    match command {
        MemoryCmd::Stats => {
            let n = store.count(&collection).await?;
            println!("collection: {collection}");
            println!("chunks: {n}");
            println!("db: {}", app.paths.database.display());
        }
        MemoryCmd::Search { query, top_k } => {
            let hits = store
                .search_text_local(&collection, &query, top_k)
                .await
                .context("memory search")?;
            if hits.is_empty() {
                println!("no hits (try `cortex memory index` first)");
            } else {
                for (i, h) in hits.iter().enumerate() {
                    println!(
                        "[{}] score={:.3} {} ({})",
                        i + 1,
                        h.score,
                        h.chunk.source_id,
                        h.chunk.source_kind
                    );
                    let preview: String = h.chunk.content.chars().take(200).collect();
                    println!("    {preview}");
                    println!();
                }
            }
        }
        MemoryCmd::Index { max_files, clear } => {
            if clear {
                let n = store.clear_collection(&collection).await?;
                eprintln!("cleared {n} chunks");
            }
            let indexed =
                index_workspace_files(&store, &app.paths.workspace, &collection, max_files).await?;
            println!("indexed {indexed} files into collection");
            println!("total chunks: {}", store.count(&collection).await?);
        }
        MemoryCmd::Summarize {
            session,
            extractive,
        } => {
            let sid = parse_session_id(&session)?;
            let session_store = app.open_store().await?;
            let sess = session_store.load_session(sid).await?;
            let resolved = app.resolve_model(None)?;
            let cfg = SummarizeConfig {
                enabled: true,
                message_threshold: 1,
                token_threshold: 0,
                keep_recent: 4,
                use_llm: !extractive,
                extractive_max_chars: 3000,
            };
            let outcome = maybe_summarize(
                &resolved.provider,
                &resolved.model,
                &sess.messages,
                None,
                &cfg,
            )
            .await
            .ok_or_else(|| anyhow::anyhow!("session too short to summarize"))?;
            session_store
                .save_summary(sid, "rolling", &outcome.summary)
                .await?;
            println!(
                "saved summary (llm={}, folded={}):\n{}",
                outcome.used_llm, outcome.folded_messages, outcome.summary
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}

async fn index_workspace_files(
    store: &VectorStore,
    workspace: &std::path::Path,
    collection: &str,
    max_files: usize,
) -> Result<usize> {
    let files = cortex_workspace::list_files(workspace, max_files).context("list files")?;
    let mut count = 0usize;
    for rel in files {
        let path = workspace.join(&rel);
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if !meta.is_file() || meta.len() > 256 * 1024 {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Skip likely-binary / empty.
        if text
            .chars()
            .filter(|c| c.is_control() && *c != '\n' && *c != '\t')
            .count()
            > 8
        {
            continue;
        }
        if text.trim().is_empty() {
            continue;
        }
        let source_id = rel.to_string_lossy().to_string();
        let content = if text.len() > 12_000 {
            format!("{}…", &text[..12_000])
        } else {
            text
        };
        store
            .index_text_local(collection, &source_id, "file", &content)
            .await
            .with_context(|| format!("index {source_id}"))?;
        count += 1;
    }
    Ok(count)
}

async fn cmd_plugins(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    command: PluginsCmd,
) -> Result<ExitCode> {
    match command {
        PluginsCmd::List => {
            let paths = Paths::resolve(workspace, config)?;
            let app = AppContext::bootstrap(paths, false).await?;
            println!(
                "Known builtins: {}",
                cortex_plugins::builtin_ids().join(", ")
            );
            println!();
            if app.plugins.is_empty() {
                println!("No plugins loaded (check config/plugins.toml).");
            } else {
                println!("Loaded plugins:");
                for st in app.plugins.status() {
                    println!(
                        "  {:12} v{:<8} [{:?}]  {}",
                        st.meta.id, st.meta.version, st.state, st.meta.description
                    );
                }
            }
            println!();
            println!("Tools from plugins (and all tools):");
            // Re-list is heavy; just point at tools list for full set.
            println!("  (use `cortex tools list` — look for plugin_* names)");
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_security(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    command: SecurityCmd,
) -> Result<()> {
    match command {
        SecurityCmd::Show => {
            let paths = Paths::resolve(workspace, config)?;
            let policy = app::load_security_policy(&paths, false)?;
            let _ = &policy as &SecurityPolicy;
            println!(
                "yolo={} sandbox={} shell_timeout_secs={}",
                policy.yolo, policy.sandbox_workspace, policy.shell_timeout_secs
            );
            println!("default_mode={:?}", policy.default_mode);
            println!("tools:");
            let mut names: Vec<_> = policy.tools.keys().cloned().collect();
            names.sort();
            for name in names {
                println!("  {name}: {:?}", policy.tools[&name]);
            }
            println!(
                "shell_deny_patterns: {}",
                policy.shell_deny_patterns.join(" | ")
            );
            println!("scrub_env: {}", policy.scrub_env.join(", "));
            println!("http_block_hosts: {}", policy.http_block_hosts.join(", "));
        }
    }
    Ok(())
}

fn build_context_for_task(
    paths: &Paths,
    prompt: &str,
    explicit_skills: &[String],
    quiet: bool,
) -> ContextBuilder {
    let workspace = paths.workspace.as_path();
    let mut prompts = PromptCatalog::with_builtins();
    // Home then monorepo/workspace then project (later overrides earlier).
    for dir in paths.prompt_dirs() {
        let _ = prompts.load_dir(&dir);
    }
    // Prefer file-based system prompt when present.
    let system = prompts
        .render("system", &Default::default())
        .unwrap_or_else(|_| cortex_runtime::DEFAULT_SYSTEM_PROMPT.to_string());

    let mut context = ContextBuilder::new(system);
    if let Some(instr) = cortex_workspace::load_project_instructions(workspace) {
        if !quiet {
            eprintln!("project instructions: {}", instr.path.display());
        }
        context = context.with_project_instructions(instr.to_prompt_section());
    }
    let map = RepoMap::build(workspace).ok();
    if let Some(ref map) = map {
        if !quiet {
            eprintln!(
                "workspace map: {} files, {}",
                map.file_count,
                map.project.summary().replace('\n', "; ")
            );
        }
        context = context.with_repo_map(map);
    } else if !quiet {
        eprintln!("warning: repo map unavailable");
    }

    let project = map.as_ref().map(|m| &m.project);
    let home_store = SkillStore::new(paths.home.join("skills"));
    let project_store = SkillStore::for_workspace(workspace);
    let reg = SkillRegistry::with_builtins_and_stores(&[&home_store, &project_store]);
    let selection = select_skills(&reg, prompt, project, explicit_skills);
    if !quiet {
        eprintln!("skills: {}", selection.skill_ids.join(", "));
        eprintln!("tools:  {}", selection.tools.join(", "));
    }

    let mut skill_body = String::from("## Active skills\n");
    for id in &selection.skill_ids {
        skill_body.push_str(&format!("- {id}\n"));
        // Attach learned skill notes when present (project overrides home).
        for store in [&home_store, &project_store] {
            if let Ok(docs) = store.load_all() {
                if let Some(doc) = docs.iter().find(|d| d.skill.id == *id) {
                    if !doc.notes.trim().is_empty() {
                        skill_body.push_str(&format!(
                            "  notes: {}\n",
                            doc.notes.lines().next().unwrap_or("")
                        ));
                    }
                }
            }
        }
    }
    skill_body.push('\n');
    for pid in &selection.prompts {
        if let Ok(p) = prompts.get(pid) {
            skill_body.push_str(&format!("### {pid}\n{}\n\n", p.body.trim()));
        }
    }

    context
        .with_skill_prompts(skill_body)
        .with_allowed_tools(selection.tools)
}

async fn cmd_skills(workspace: Option<PathBuf>, command: SkillsCmd) -> Result<()> {
    let paths = Paths::resolve(workspace, None)?;
    let home_store = SkillStore::new(paths.home.join("skills"));
    let project_store = SkillStore::for_workspace(&paths.workspace);
    let reg = SkillRegistry::with_builtins_and_stores(&[&home_store, &project_store]);
    match command {
        SkillsCmd::List => {
            println!(
                "{:<18}  {:<8}  {:<10}  DESCRIPTION",
                "ID", "ALWAYS", "ORIGIN"
            );
            for s in reg.all() {
                let origin = [&home_store, &project_store]
                    .into_iter()
                    .find_map(|store| {
                        store.load_all().ok().and_then(|docs| {
                            docs.into_iter()
                                .find(|d| d.skill.id == s.id)
                                .map(|d| format!("{:?}", d.origin).to_ascii_lowercase())
                        })
                    })
                    .unwrap_or_else(|| "builtin".into());
                println!(
                    "{:<18}  {:<8}  {:<10}  {}",
                    s.id,
                    if s.always_on { "yes" } else { "no" },
                    origin,
                    s.description
                );
            }
        }
        SkillsCmd::Select { prompt, skills } => {
            let project = RepoMap::build(&paths.workspace).ok().map(|m| m.project);
            let sel = select_skills(&reg, &prompt, project.as_ref(), &skills);
            println!("skills: {}", sel.skill_ids.join(", "));
            println!("tools:  {}", sel.tools.join(", "));
            println!("prompts: {}", sel.prompts.join(", "));
        }
        SkillsCmd::Import {
            source,
            id,
            tools,
            tags,
            dry_run,
        } => {
            let (source_label, content) =
                if source.starts_with("https://") || source.starts_with("http://") {
                    if source.starts_with("http://") {
                        anyhow::bail!("refusing plain http:// skill import (use https://)");
                    }
                    eprintln!("fetching {source} …");
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(60))
                        .user_agent(format!(
                            "cortex-skills-import/{}",
                            env!("CARGO_PKG_VERSION")
                        ))
                        .build()?;
                    let resp = client
                        .get(&source)
                        .send()
                        .await
                        .with_context(|| format!("GET {source}"))?;
                    if !resp.status().is_success() {
                        anyhow::bail!("fetch failed: HTTP {}", resp.status());
                    }
                    let text = resp.text().await.context("read response body")?;
                    (source.clone(), text)
                } else {
                    let path = PathBuf::from(&source);
                    let path = if path.is_absolute() {
                        path
                    } else {
                        paths.workspace.join(path)
                    };
                    read_skill_source(&path).map_err(|e| anyhow::anyhow!("{e}"))?
                };

            let imported = import_from_markdown(
                &content,
                ImportOptions {
                    id,
                    tools,
                    tags,
                    source: source_label.clone(),
                },
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

            println!("id:          {}", imported.document.skill.id);
            println!("description: {}", imported.document.skill.description);
            println!("tools:       {}", imported.document.skill.tools.join(", "));
            println!("tags:        {}", imported.document.skill.tags.join(", "));
            println!("prompt_id:   {}", imported.prompt_id);
            println!("source:      {source_label}");
            if dry_run {
                println!("dry-run: not written");
                return Ok(());
            }
            let (skill_path, prompt_path) = write_imported_skill(&paths.workspace, &imported)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("wrote skill  {}", skill_path.display());
            println!("wrote prompt {}", prompt_path.display());
            println!(
                "activate with: cortex run \"…\" --skills {}",
                imported.document.skill.id
            );
        }
    }
    Ok(())
}

fn cmd_workspace(workspace: Option<PathBuf>, command: WorkspaceCmd) -> Result<()> {
    let root = workspace
        .unwrap_or_else(|| std::env::current_dir().expect("cwd"))
        .canonicalize()
        .context("workspace")?;
    match command {
        WorkspaceCmd::Info => {
            let map = RepoMap::build(&root).context("build repo map")?;
            println!("root: {}", map.root.display());
            println!("{}", map.project.summary());
            println!("files_indexed: {}", map.file_count);
        }
        WorkspaceCmd::Map { max_files } => {
            let map =
                RepoMap::build_with_limits(&root, max_files, 120).context("build repo map")?;
            print!("{}", map.to_prompt_section());
        }
    }
    Ok(())
}

async fn open_store(paths: &Paths) -> Result<SessionStore> {
    if let Some(parent) = paths.database.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let pool = open_sqlite(&paths.database)
        .await
        .with_context(|| format!("open database {}", paths.database.display()))?;
    Ok(SessionStore::new(pool))
}

fn cmd_setup(
    force: bool,
    wizard: bool,
    no_wizard: bool,
    default_model: Option<String>,
    ollama_model: Option<String>,
) -> Result<()> {
    let home = cortex_home();
    let report = bootstrap_home(&home, force)?;
    println!("Cortex home: {}", home.display());
    if report.models_written {
        println!("✓ wrote {}", report.models_path.display());
    } else {
        println!(
            "✓ {} already present (use --force to overwrite)",
            report.models_path.display()
        );
    }
    for p in &report.created_dirs {
        println!("✓ {}", p.display());
    }

    let models_path = report.models_path.clone();
    let mut chosen: Option<String> = None;

    // Non-interactive flags win.
    if let Some(alias) = default_model {
        let preset = setup_config::preset_from_flags(&alias, ollama_model.as_deref())?;
        setup_config::write_setup_models_toml(&models_path, &preset)?;
        println!(
            "✓ wrote {} (default_model = \"{}\")",
            models_path.display(),
            preset.alias()
        );
        chosen = Some(preset.alias().to_string());
    } else {
        let want_tui = (wizard || stdin_is_tty()) && !no_wizard;
        if want_tui {
            if !stdin_is_tty() {
                anyhow::bail!("setup wizard requires an interactive terminal (TTY); use --default-model or --no-wizard");
            }
            match setup_tui::run_setup_tui(&home, &models_path) {
                Ok(alias) => {
                    println!("✓ default_model = \"{alias}\"");
                    chosen = Some(alias);
                }
                Err(e) if e.to_string().contains("cancelled") => {
                    println!("setup wizard cancelled — home files kept");
                }
                Err(e) => return Err(e),
            }
        }
    }

    if let Some(alias) = chosen.as_deref() {
        print_setup_key_hints(alias);
    }

    println!("\nNext:");
    println!("  cortex doctor");
    println!("  cortex models list");
    println!("  cd my-project && cortex run \"hello\"");
    println!("  # TUI wizard:     cortex setup --wizard");
    println!("  # non-interactive: cortex setup --default-model ollama --ollama-model llama3.2");
    println!("  # files only:     cortex setup --no-wizard");
    println!("  # project:        cortex init  |  cortex init --web3");
    Ok(())
}

fn stdin_is_tty() -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        let tty_ok = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .is_ok();
        let stdin_char = std::fs::metadata("/dev/stdin")
            .map(|m| m.file_type().is_char_device())
            .unwrap_or(false);
        tty_ok && stdin_char
    }
    #[cfg(not(unix))]
    {
        std::env::var_os("TERM").is_some()
    }
}

fn print_setup_key_hints(alias: &str) {
    match alias {
        "openai" => println!("\nSet:  export OPENAI_API_KEY=sk-…"),
        "anthropic" => println!("\nSet:  export ANTHROPIC_API_KEY=…"),
        "openrouter" => println!("\nSet:  export OPENROUTER_API_KEY=…"),
        "ollama" => println!("\nEnsure Ollama is running:  ollama serve && ollama pull <model>"),
        "default" | "mock" => println!("\nOffline mock provider — no API key needed."),
        other => println!("\nConfigured provider alias `{other}` — set any api_key_env if needed."),
    }
}

fn cmd_doctor(workspace: Option<PathBuf>, config: Option<PathBuf>) -> Result<()> {
    let paths = Paths::resolve(workspace, config)?;
    let home_writable = std::fs::create_dir_all(&paths.home).is_ok()
        && std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(paths.home.join(".doctor-write-test"))
            .and_then(|f| {
                drop(f);
                std::fs::remove_file(paths.home.join(".doctor-write-test"))
            })
            .is_ok();

    let on_path = std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p).any(|dir| {
                let cand = dir.join("cortex");
                cand.is_file() || {
                    let mut w = cand.clone();
                    w.set_extension("exe");
                    w.is_file()
                }
            })
        })
        .unwrap_or(false);

    let key_envs = ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"];

    println!("cortex doctor");
    println!("  version:     {}", env!("CARGO_PKG_VERSION"));
    println!(
        "  binary:      {}",
        std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "?".into())
    );
    println!("  home:        {}", paths.home.display());
    println!("  home_ok:     {home_writable}");
    println!("  workspace:   {}", paths.workspace.display());
    println!("  project_dir: {}", paths.cortex_dir.display());
    println!("  project_ok:  {}", paths.cortex_dir.is_dir());
    println!("  models:      {}", paths.models_config.display());
    println!("  models_ok:   {}", paths.models_config.is_file());
    println!("  database:    {}", paths.database.display());
    println!("  on_path:     {on_path}");
    println!("  env keys (set? never printed):");
    for k in key_envs {
        let set = std::env::var_os(k).is_some_and(|v| !v.is_empty());
        println!("    {k}: {}", if set { "yes" } else { "no" });
    }
    if let Ok(models) = cortex_llm::ModelsConfig::from_file(&paths.models_config) {
        println!(
            "  default_model: {}",
            models.default_model.as_deref().unwrap_or("(unset)")
        );
        let mut keys: Vec<_> = models.providers.keys().cloned().collect();
        keys.sort();
        println!("  providers:     {}", keys.join(", "));
    }
    Ok(())
}

/// Print `agent.assistant_text_delta` payloads to stderr for `--stream`.
struct StreamPrinter;

#[async_trait::async_trait]
impl EnvelopeHandler for StreamPrinter {
    async fn handle(&self, event: EventEnvelope) {
        if event.kind != "agent.assistant_text_delta" {
            return;
        }
        if let Some(text) = event.payload.get("text").and_then(|v| v.as_str()) {
            let mut err = io::stderr().lock();
            let _ = write!(err, "{text}");
            let _ = err.flush();
        }
    }
}

const FOUNDRY_MCP_STUB: &str = include_str!("../../../examples/mcp/foundry.mcp.toml");

const WEB3_INSTRUCTIONS: &str = r#"# Cortex Web3 / smart-contract project

This project was initialized with `cortex init --web3`.

## Guidance for the agent

- Prefer Foundry when `foundry.toml` is present.
- Prefer fixed tools over freeform shell when available:
  - Foundry: `forge_build` / `forge_test` / `forge_test_match` (foundry_helpers)
  - Static: `slither_scan` / `slither_human_summary` / `aderyn_scan` (sc_analyzers)
- For audits use skills `sc_security`, `solidity`, and optionally `sc_xray`.
- Multi-lens: tool `audit_lenses` (parallel specialty reviewers).
- Write durable reports with `write_audit_report` under `.cortex/audits/`.
- Enable Foundry MCP: see `.cortex/mcp.toml` (requires Node + forge on PATH).
- External packs: https://skills.eth.sh/ — `cortex skills import <url-or-path>`.

## Honest limits

- Assisted review is not a professional audit.
- Note when Slither/Aderyn/forge are missing; do not invent tool output.
"#;

const FOUNDRY_HELPERS_PLUGIN: &str = include_str!("../../../plugins/foundry_helpers/plugin.toml");
const SC_ANALYZERS_PLUGIN: &str = include_str!("../../../plugins/sc_analyzers/plugin.toml");

fn write_web3_plugin(
    cortex_dir: &std::path::Path,
    id: &str,
    body: &str,
    force: bool,
) -> Result<()> {
    let plug_dir = cortex_dir.join("plugins").join(id);
    let plug_toml = plug_dir.join("plugin.toml");
    if plug_toml.exists() && !force {
        println!(
            "✓ {} already exists (use --force to overwrite)",
            plug_toml.display()
        );
        return Ok(());
    }
    std::fs::create_dir_all(&plug_dir).with_context(|| format!("create {}", plug_dir.display()))?;
    std::fs::write(&plug_toml, body).with_context(|| format!("write {}", plug_toml.display()))?;
    println!("✓ wrote {} ({id} tools)", plug_toml.display());
    Ok(())
}

fn cmd_update(dry_run: bool) -> Result<()> {
    let install = "curl -fsSL https://raw.githubusercontent.com/CallumBicknell/cortex/main/scripts/install.sh | sh";
    println!("Cortex update (Linux/macOS)");
    println!("  current: {}", env!("CARGO_PKG_VERSION"));
    println!("  reinstall latest release into ~/.local/bin:");
    println!("    {install}");
    println!("  pin a version:");
    println!("    CORTEX_VERSION=v0.2.1 {install}");
    println!("  from source:");
    println!(
        "    cargo install --git https://github.com/CallumBicknell/cortex --locked --bin cortex"
    );
    if dry_run {
        println!("dry-run: not executing install script");
        return Ok(());
    }
    // Only auto-run when stdout is a TTY-ish interactive intent; still require network.
    // Prefer printing unless CORTEX_UPDATE_EXEC=1 for scripted upgrades.
    if std::env::var_os("CORTEX_UPDATE_EXEC").is_some() {
        println!("running install script (CORTEX_UPDATE_EXEC set)…");
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(install)
            .status()
            .context("run install.sh")?;
        if !status.success() {
            anyhow::bail!("install script exited with {status}");
        }
    } else {
        println!("\nTo execute the installer now:");
        println!("  CORTEX_UPDATE_EXEC=1 cortex update");
        println!("  # or paste the curl | sh line above");
    }
    Ok(())
}

async fn cmd_init(workspace: Option<PathBuf>, force: bool, web3: bool) -> Result<()> {
    // Ensure user home exists so global defaults are ready.
    let home = cortex_home();
    let _ = bootstrap_home(&home, false);

    let workspace = workspace
        .unwrap_or_else(|| std::env::current_dir().expect("cwd"))
        .canonicalize()
        .context("workspace")?;
    let cortex_dir = workspace.join(".cortex");
    std::fs::create_dir_all(cortex_dir.join("data")).context("create .cortex/data")?;
    std::fs::create_dir_all(cortex_dir.join("sessions")).context("create .cortex/sessions")?;

    let models_path = cortex_dir.join("models.toml");
    if models_path.exists() && !force {
        println!(
            "✓ {} already exists (use --force to overwrite)",
            models_path.display()
        );
    } else {
        if models_path.exists() && force {
            std::fs::remove_file(&models_path)?;
        }
        write_default_models_toml(&models_path)?;
        println!("✓ wrote {}", models_path.display());
    }

    let gitignore = cortex_dir.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(&gitignore, "data/\nsessions/\n*.db\n.env\n")?;
        println!("✓ wrote {}", gitignore.display());
    }

    // Initialize empty SQLite DB.
    let db_path = cortex_dir.join("data").join("cortex.db");
    let _ = open_sqlite(&db_path).await.context("init database")?;
    println!("✓ database {}", db_path.display());

    if web3 {
        let mcp_path = cortex_dir.join("mcp.toml");
        if mcp_path.exists() && !force {
            println!(
                "✓ {} already exists (use --force to overwrite)",
                mcp_path.display()
            );
        } else {
            std::fs::write(&mcp_path, FOUNDRY_MCP_STUB)
                .with_context(|| format!("write {}", mcp_path.display()))?;
            println!("✓ wrote {} (Foundry MCP sample)", mcp_path.display());
        }

        let instr_path = cortex_dir.join("instructions.md");
        if instr_path.exists() && !force {
            println!(
                "✓ {} already exists (use --force to overwrite)",
                instr_path.display()
            );
        } else {
            std::fs::write(&instr_path, WEB3_INSTRUCTIONS)
                .with_context(|| format!("write {}", instr_path.display()))?;
            println!("✓ wrote {}", instr_path.display());
        }

        let agents = workspace.join("AGENTS.md");
        if !agents.exists() {
            let stub = "# Project agent notes\n\n\
                        See `.cortex/instructions.md` for Web3/audit defaults.\n\
                        Prefer `cortex run \"…\" --skills sc_security,solidity`.\n\
                        Prefer `forge_build` / `forge_test` / `slither_scan` when available.\n";
            std::fs::write(&agents, stub).ok();
            println!("✓ wrote {}", agents.display());
        }

        // Fixed-arg forge + analyzer tools (auto-discovered under .cortex/plugins/).
        write_web3_plugin(
            &cortex_dir,
            "foundry_helpers",
            FOUNDRY_HELPERS_PLUGIN,
            force,
        )?;
        write_web3_plugin(&cortex_dir, "sc_analyzers", SC_ANALYZERS_PLUGIN, force)?;
    }

    println!("\nCortex project initialized in {}", workspace.display());
    println!("User home (global): {}", home.display());
    println!("Next:");
    println!("  export OPENAI_API_KEY=...   # or use ollama");
    println!("  # edit .cortex/models.toml  # project override of default_model");
    if web3 {
        println!("  # ensure forge + node/npx on PATH for MCP");
        println!("  cortex tools list | grep mcp_foundry   # after MCP starts");
        println!("  cortex run \"Audit this repo\" --skills sc_security,solidity --yolo");
    } else {
        println!("  cortex run \"hello\"");
        println!("  # Web3 scaffold: cortex init --web3");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cmd_run(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    prompt: String,
    model: Option<String>,
    stream: bool,
    yolo: bool,
    max_turns: u32,
    json: bool,
    session_id: Option<String>,
    no_save: bool,
    skills: Vec<String>,
    plan: bool,
    verify: bool,
    verify_cmd: Option<String>,
) -> Result<ExitCode> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths.clone(), yolo).await?;
    let resolved = app.resolve_model(model.as_deref())?;
    let store = if no_save {
        None
    } else {
        Some(open_store(&paths).await?)
    };

    if !json {
        println!(
            "workspace: {}\ncortex:    {}\ndb:        {}\nmodel:     {} ({}/{})\n",
            app.paths.workspace.display(),
            app.paths.cortex_dir.display(),
            app.paths.database.display(),
            resolved.alias,
            resolved.provider_id,
            resolved.model
        );
    }

    let context = build_context_for_task(&app.paths, &prompt, &skills, json);

    let cancel = CancellationToken::new();
    let cancel_ctrl = cancel.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        cancel_ctrl.cancel();
    });

    let session =
        load_or_new_session(store.as_ref(), session_id.as_deref(), &app, &resolved).await?;
    let session_id_for_ctx = session.id;

    let tool_ctx = app.tool_context(cancel.clone(), store.as_ref(), Some(session_id_for_ctx));
    let (verify_after_writes, verify_command) =
        resolve_verify(verify, verify_cmd, &app.paths.workspace);
    let want_stream = stream && !json;
    let loop_cfg = AgentLoopConfig {
        max_turns,
        context,
        summarize: SummarizeConfig::default(),
        plan_mode: plan,
        verify_after_writes,
        verify_command,
        stream_tokens: want_stream,
        ..Default::default()
    };
    let tools = tools_with_subagent(
        &app.tools,
        Arc::clone(&resolved.provider),
        resolved.model.clone(),
        loop_cfg.clone(),
    );
    let mut agent = AgentLoop::new(
        Arc::clone(&resolved.provider),
        resolved.model.clone(),
        tools,
        loop_cfg,
    );
    if want_stream {
        let bus = Arc::new(InMemoryEventBus::new(256));
        bus.subscribe(Arc::new(StreamPrinter)).await;
        agent = agent.with_event_bus(bus);
    }
    if let Some(store) = &store {
        if let Ok(Some((_, s))) = store
            .latest_summary(session_id_for_ctx, Some("rolling"))
            .await
        {
            agent.set_rolling_summary(Some(s));
        }
    }

    let output = agent
        .run(RunInput {
            session,
            prompt,
            cancel,
            tool_ctx,
        })
        .await
        .context("agent run")?;

    if want_stream {
        // Finish the streamed line before summary output.
        let _ = writeln!(io::stderr());
    }

    if let (Some(store), Some(summary)) = (&store, agent.rolling_summary()) {
        let _ = store
            .save_summary(session_id_for_ctx, "rolling", &summary)
            .await;
    }

    let checkpoint_id = if let Some(store) = &store {
        Some(persist_output(store, &output).await?)
    } else {
        None
    };

    if json {
        let summary = serde_json::json!({
            "status": format!("{:?}", output.status).to_ascii_lowercase(),
            "session_id": output.session.id.to_string(),
            "run_id": output.run_id.to_string(),
            "checkpoint_id": checkpoint_id.map(|c| c.to_string()),
            "turns": output.turns,
            "duration_ms": output.duration_ms,
            "final_message": output.final_message,
            "error": output.error,
            "tool_results": output.tool_results.iter().map(|r| {
                serde_json::json!({
                    "name": r.name,
                    "is_error": r.is_error,
                    "output": r.output,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_run_human(&output);
        if let Some(cid) = checkpoint_id {
            println!("saved session={} checkpoint={}", output.session.id, cid);
        }
    }

    Ok(exit_for_status(output.status))
}

async fn cmd_chat(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    model: Option<String>,
    yolo: bool,
    max_turns: u32,
    session_id: Option<String>,
    skills: Vec<String>,
    plan: bool,
    verify: bool,
    verify_cmd: Option<String>,
) -> Result<ExitCode> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths.clone(), yolo).await?;
    let resolved = app.resolve_model(model.as_deref())?;
    let store = open_store(&paths).await?;
    let (verify_after_writes, verify_command) =
        resolve_verify(verify, verify_cmd, &app.paths.workspace);

    let mut session =
        load_or_new_session(Some(&store), session_id.as_deref(), &app, &resolved).await?;

    println!(
        "Cortex chat — model {} ({}/{})",
        resolved.alias, resolved.provider_id, resolved.model
    );
    println!("workspace: {}", app.paths.workspace.display());
    println!("session:   {}", session.id);
    println!("db:        {}", app.paths.database.display());
    if plan {
        println!("plan mode: on");
    }
    if verify_after_writes {
        println!(
            "verify:    {}",
            verify_command.as_deref().unwrap_or("(none)")
        );
    }
    println!("Type a message, or /quit to exit. Ctrl-C cancels the current turn.\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    loop {
        write!(stdout, "you> ")?;
        stdout.flush()?;
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let prompt = line.trim().to_string();
        if prompt.is_empty() {
            continue;
        }
        if matches!(prompt.as_str(), "/quit" | "/exit" | ":q") {
            break;
        }

        // Re-select skills per turn from the latest prompt (plus explicit flags).
        let context = build_context_for_task(&app.paths, &prompt, &skills, false);

        let cancel = CancellationToken::new();
        let cancel_ctrl = cancel.clone();
        let ctrl = tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancel_ctrl.cancel();
        });

        let tool_ctx = app.tool_context(cancel.clone(), Some(&store), Some(session.id));
        let loop_cfg = AgentLoopConfig {
            max_turns,
            context,
            summarize: SummarizeConfig::default(),
            plan_mode: plan,
            verify_after_writes,
            verify_command: verify_command.clone(),
            ..Default::default()
        };
        let tools = tools_with_subagent(
            &app.tools,
            Arc::clone(&resolved.provider),
            resolved.model.clone(),
            loop_cfg.clone(),
        );
        let agent = AgentLoop::new(
            Arc::clone(&resolved.provider),
            resolved.model.clone(),
            tools,
            loop_cfg,
        );
        if let Ok(Some((_, s))) = store.latest_summary(session.id, Some("rolling")).await {
            agent.set_rolling_summary(Some(s));
        }

        let output = agent
            .run(RunInput {
                session: session.clone(),
                prompt,
                cancel,
                tool_ctx,
            })
            .await
            .context("chat turn")?;
        ctrl.abort();

        if let Some(summary) = agent.rolling_summary() {
            let _ = store
                .save_summary(output.session.id, "rolling", &summary)
                .await;
        }

        session = output.session.clone();
        let _ = persist_output(&store, &output).await?;
        print_run_human(&output);
        println!();
    }

    Ok(ExitCode::SUCCESS)
}

async fn cmd_sessions(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    command: SessionsCmd,
) -> Result<ExitCode> {
    let paths = Paths::resolve(workspace, config)?;
    let store = open_store(&paths).await?;

    match command {
        SessionsCmd::List { limit } => {
            let rows = store.list_sessions(limit).await?;
            if rows.is_empty() {
                println!("(no sessions)");
            } else {
                println!(
                    "{:<36}  {:<10}  {:>5}  {:<20}  MODEL",
                    "ID", "STATUS", "MSGS", "UPDATED"
                );
                for s in rows {
                    println!(
                        "{:<36}  {:<10}  {:>5}  {:<20}  {}",
                        s.id,
                        format!("{:?}", s.status).to_ascii_lowercase(),
                        s.message_count,
                        s.updated_at.format("%Y-%m-%d %H:%M:%S"),
                        s.model
                    );
                }
            }
        }
        SessionsCmd::Show { id } => {
            let sid = parse_session_id(&id)?;
            let session = store.load_session(sid).await?;
            println!("session {}", session.id);
            println!("status  {:?}", session.status);
            println!("model   {}", session.model);
            println!("workspace {}", session.workspace);
            println!("messages {}\n", session.message_count());
            for (i, msg) in session.messages.iter().enumerate() {
                println!("[{i}] {:?}: {}", msg.role, truncate(&msg.content, 200));
                if !msg.tool_calls.is_empty() {
                    for tc in &msg.tool_calls {
                        println!("      tool_call {} {}", tc.name, tc.arguments);
                    }
                }
            }
            if let Some(cp) = store.latest_checkpoint(sid).await? {
                println!(
                    "\nlatest checkpoint {} phase={} turns={}",
                    cp.id, cp.state.phase, cp.state.turns
                );
            }
        }
        SessionsCmd::Resume {
            id,
            model,
            yolo,
            max_turns,
        } => {
            return cmd_chat(
                Some(paths.workspace.clone()),
                Some(paths.models_config.clone()),
                model,
                yolo,
                max_turns,
                Some(id),
                Vec::new(),
                false,
                false,
                None,
            )
            .await;
        }
        SessionsCmd::Export { id, output } => {
            let sid = parse_session_id(&id)?;
            let export = store.export_session(sid).await?;
            let pretty = serde_json::to_string_pretty(&export)?;
            if let Some(path) = output {
                std::fs::write(&path, pretty)
                    .with_context(|| format!("write {}", path.display()))?;
                println!("wrote {}", path.display());
            } else {
                println!("{pretty}");
            }
        }
        SessionsCmd::Archive { id } => {
            let sid = parse_session_id(&id)?;
            store.archive_session(sid).await?;
            println!("archived {sid}");
        }
    }
    Ok(ExitCode::SUCCESS)
}

async fn load_or_new_session(
    store: Option<&SessionStore>,
    session_id: Option<&str>,
    app: &AppContext,
    resolved: &cortex_llm::ResolvedModel,
) -> Result<Session> {
    if let Some(id_str) = session_id {
        let store = store.context("--session requires a database (omit --no-save)")?;
        let sid = parse_session_id(id_str)?;
        let mut session = store
            .load_session(sid)
            .await
            .with_context(|| format!("load session {sid}"))?;
        session.status = SessionStatus::Active;
        session.model = format!("{}/{}", resolved.provider_id, resolved.model);
        return Ok(session);
    }
    Ok(Session::new(
        app.paths.workspace.to_string_lossy(),
        format!("{}/{}", resolved.provider_id, resolved.model),
    ))
}

async fn persist_output(
    store: &SessionStore,
    output: &RunOutput,
) -> Result<cortex_common::CheckpointId> {
    let mut session = output.session.clone();
    session.status = match output.status {
        TaskStatus::Succeeded => SessionStatus::Completed,
        TaskStatus::Failed => SessionStatus::Failed,
        TaskStatus::Cancelled => SessionStatus::Paused,
        TaskStatus::Pending | TaskStatus::Running => SessionStatus::Active,
    };
    session.updated_at = chrono::Utc::now();

    for tr in &output.tool_results {
        // Best-effort tool audit trail.
        let call = cortex_models::ToolCall {
            id: tr.tool_call_id,
            name: tr.name.clone(),
            arguments: serde_json::json!({}),
        };
        let _ = store.save_tool_trace(session.id, &call, tr).await;
    }

    let phase = format!("{:?}", output.phase).to_ascii_lowercase();
    let cp = store
        .persist_run(
            &session,
            CheckpointState {
                run_id: Some(output.run_id),
                phase,
                turns: output.turns,
                note: output.error.clone(),
            },
            Some("after-run".into()),
        )
        .await
        .context("persist session")?;

    let _ = store
        .append_event(
            Some(session.id),
            "cli.run.completed",
            &serde_json::json!({
                "run_id": output.run_id.to_string(),
                "status": format!("{:?}", output.status),
                "turns": output.turns,
            }),
            None,
        )
        .await;

    Ok(cp.id)
}

fn parse_session_id(s: &str) -> Result<SessionId> {
    SessionId::from_str(s).map_err(|e| anyhow::anyhow!("invalid session id: {e}"))
}

/// Resolve verify-after-writes settings from CLI flags + project fingerprint.
fn resolve_verify(
    verify: bool,
    verify_cmd: Option<String>,
    workspace: &std::path::Path,
) -> (bool, Option<String>) {
    let cmd = verify_cmd.filter(|s| !s.trim().is_empty()).or_else(|| {
        if !verify {
            return None;
        }
        cortex_workspace::ProjectInfo::detect(workspace).test_command
    });
    let enabled = verify || cmd.is_some();
    (enabled, cmd)
}

async fn cmd_tools_list(workspace: Option<PathBuf>, config: Option<PathBuf>) -> Result<ExitCode> {
    // Full bootstrap so MCP + plugins appear alongside builtins.
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths, false).await?;
    // Ignore BrokenPipe so `cortex tools list | head` does not panic (SIGPIPE).
    let _ = writeln_stdout("Registered tools:\n");
    for spec in app.tools.registry().specs() {
        if !writeln_stdout(&format!("  {:16}  {}", spec.name, spec.description)) {
            break;
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Write a line to stdout; returns false on broken pipe (caller may stop).
fn writeln_stdout(line: &str) -> bool {
    use std::io::{self, Write};
    let mut out = io::stdout();
    match writeln!(out, "{line}") {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => false,
        Err(e) => {
            eprintln!("stdout write error: {e}");
            false
        }
    }
}

async fn cmd_models_list(workspace: Option<PathBuf>, config: Option<PathBuf>) -> Result<()> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths, true).await?;
    println!("config: {}\n", app.paths.models_config.display());
    println!("Providers:");
    for id in app.registry.provider_ids() {
        println!("  - {id}");
    }
    println!("\nModel aliases:");
    for name in app.registry.alias_names() {
        match app.registry.resolve(Some(&name)) {
            Ok(m) => println!("  {:16}  -> {} / {}", name, m.provider_id, m.model),
            Err(e) => println!("  {name:16}  (error: {e})"),
        }
    }
    Ok(())
}

fn print_run_human(output: &RunOutput) {
    if !output.tool_results.is_empty() {
        println!("tools:");
        for r in &output.tool_results {
            let flag = if r.is_error { "ERR" } else { "ok" };
            let preview: String = r.output.chars().take(120).collect();
            println!("  [{flag}] {} — {}", r.name, preview.replace('\n', " "));
        }
        println!();
    }
    if let Some(msg) = &output.final_message {
        println!("assistant>\n{}\n", redact_text(msg));
    }
    if let Some(err) = &output.error {
        println!("error: {}", redact_text(err));
    }
    println!(
        "status={:?} turns={} duration_ms={} session={}",
        output.status, output.turns, output.duration_ms, output.session.id
    );
}

fn truncate(s: &str, max: usize) -> String {
    let mut t: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        t.push('…');
    }
    t.replace('\n', " ")
}

fn exit_for_status(status: TaskStatus) -> ExitCode {
    match status {
        TaskStatus::Succeeded => ExitCode::SUCCESS,
        TaskStatus::Cancelled => ExitCode::from(130),
        TaskStatus::Failed | TaskStatus::Pending | TaskStatus::Running => ExitCode::from(1),
    }
}
