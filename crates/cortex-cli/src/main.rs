//! Cortex CLI — `cortex run`, `cortex chat`, sessions, and helpers.

mod app;
mod approver;
mod db_audit;

use anyhow::{Context, Result};
use app::{init_tracing, load_dotenv, write_default_models_toml, AppContext, Paths};
use clap::{Parser, Subcommand};
use cortex_common::SessionId;
use cortex_memory::{open_sqlite, CheckpointState, SessionStore};
use cortex_models::{Session, SessionStatus, TaskStatus};
use cortex_prompts::PromptCatalog;
use cortex_runtime::{AgentLoop, AgentLoopConfig, ContextBuilder, RunInput, RunOutput};
use cortex_security::{redact_text, SecurityPolicy};
use cortex_skills::{select_skills, SkillRegistry};
use cortex_tools::ToolRegistry;
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
    /// Create `.cortex/` config in the workspace.
    Init {
        /// Overwrite models.toml if it exists.
        #[arg(long)]
        force: bool,
    },
    /// Run a single agent task and exit.
    Run {
        /// User prompt / task description.
        prompt: String,
        /// Model alias from models.toml (default: configured default).
        #[arg(long, short)]
        model: Option<String>,
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
    /// List builtin skills.
    List,
    /// Show which skills would activate for a prompt.
    Select {
        /// User prompt / task text.
        prompt: String,
        /// Optional explicit skill ids.
        #[arg(long, value_delimiter = ',')]
        skills: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum SecurityCmd {
    /// Show the effective security policy.
    Show,
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
        Commands::Init { force } => {
            cmd_init(cli.workspace, force).await?;
        }
        Commands::Run {
            prompt,
            model,
            yolo,
            max_turns,
            json,
            session,
            no_save,
            skills,
        } => {
            return cmd_run(
                cli.workspace,
                cli.config,
                prompt,
                model,
                yolo,
                max_turns,
                json,
                session,
                no_save,
                skills,
            )
            .await;
        }
        Commands::Chat {
            model,
            yolo,
            max_turns,
            session,
            skills,
        } => {
            return cmd_chat(
                cli.workspace,
                cli.config,
                model,
                yolo,
                max_turns,
                session,
                skills,
            )
            .await;
        }
        Commands::Tools { command } => match command {
            ToolsCmd::List => cmd_tools_list()?,
        },
        Commands::Models { command } => match command {
            ModelsCmd::List => cmd_models_list(cli.workspace, cli.config)?,
        },
        Commands::Sessions { command } => {
            return cmd_sessions(cli.workspace, cli.config, command).await;
        }
        Commands::Workspace { command } => {
            cmd_workspace(cli.workspace, command)?;
        }
        Commands::Skills { command } => {
            cmd_skills(cli.workspace, command)?;
        }
        Commands::Security { command } => {
            cmd_security(cli.workspace, cli.config, command)?;
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
    workspace: &std::path::Path,
    prompt: &str,
    explicit_skills: &[String],
    quiet: bool,
) -> ContextBuilder {
    let prompts = PromptCatalog::with_builtins();
    // Prefer file-based system prompt when present.
    let system = prompts
        .render("system", &Default::default())
        .unwrap_or_else(|_| cortex_runtime::DEFAULT_SYSTEM_PROMPT.to_string());

    let mut context = ContextBuilder::new(system);
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
    let reg = SkillRegistry::with_builtins();
    let selection = select_skills(&reg, prompt, project, explicit_skills);
    if !quiet {
        eprintln!("skills: {}", selection.skill_ids.join(", "));
        eprintln!("tools:  {}", selection.tools.join(", "));
    }

    let mut skill_body = String::from("## Active skills\n");
    for id in &selection.skill_ids {
        skill_body.push_str(&format!("- {id}\n"));
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

fn cmd_skills(workspace: Option<PathBuf>, command: SkillsCmd) -> Result<()> {
    let reg = SkillRegistry::with_builtins();
    match command {
        SkillsCmd::List => {
            println!("{:<14}  {:<8}  DESCRIPTION", "ID", "ALWAYS");
            for s in reg.all() {
                println!(
                    "{:<14}  {:<8}  {}",
                    s.id,
                    if s.always_on { "yes" } else { "no" },
                    s.description
                );
            }
        }
        SkillsCmd::Select { prompt, skills } => {
            let root = workspace
                .unwrap_or_else(|| std::env::current_dir().expect("cwd"))
                .canonicalize()
                .ok();
            let project = root
                .as_ref()
                .and_then(|r| RepoMap::build(r).ok())
                .map(|m| m.project);
            let sel = select_skills(&reg, &prompt, project.as_ref(), &skills);
            println!("skills: {}", sel.skill_ids.join(", "));
            println!("tools:  {}", sel.tools.join(", "));
            println!("prompts: {}", sel.prompts.join(", "));
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

async fn cmd_init(workspace: Option<PathBuf>, force: bool) -> Result<()> {
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

    println!("\nCortex initialized in {}", workspace.display());
    println!("Next:");
    println!("  export OPENAI_API_KEY=...   # or use ollama");
    println!("  # edit .cortex/models.toml  # set default_model");
    println!("  cortex run \"hello\"");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cmd_run(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    prompt: String,
    model: Option<String>,
    yolo: bool,
    max_turns: u32,
    json: bool,
    session_id: Option<String>,
    no_save: bool,
    skills: Vec<String>,
) -> Result<ExitCode> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths.clone(), yolo)?;
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

    let context = build_context_for_task(&app.paths.workspace, &prompt, &skills, json);

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
    let agent = AgentLoop::new(
        Arc::clone(&resolved.provider),
        resolved.model.clone(),
        app.tools.clone(),
        AgentLoopConfig {
            max_turns,
            context,
            temperature: None,
            max_tokens: None,
            stop_on_max_turns: true,
        },
    );

    let output = agent
        .run(RunInput {
            session,
            prompt,
            cancel,
            tool_ctx,
        })
        .await
        .context("agent run")?;

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
) -> Result<ExitCode> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths.clone(), yolo)?;
    let resolved = app.resolve_model(model.as_deref())?;
    let store = open_store(&paths).await?;

    let mut session =
        load_or_new_session(Some(&store), session_id.as_deref(), &app, &resolved).await?;

    println!(
        "Cortex chat — model {} ({}/{})",
        resolved.alias, resolved.provider_id, resolved.model
    );
    println!("workspace: {}", app.paths.workspace.display());
    println!("session:   {}", session.id);
    println!("db:        {}", app.paths.database.display());
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
        let context = build_context_for_task(&app.paths.workspace, &prompt, &skills, false);

        let cancel = CancellationToken::new();
        let cancel_ctrl = cancel.clone();
        let ctrl = tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancel_ctrl.cancel();
        });

        let tool_ctx = app.tool_context(cancel.clone(), Some(&store), Some(session.id));
        let agent = AgentLoop::new(
            Arc::clone(&resolved.provider),
            resolved.model.clone(),
            app.tools.clone(),
            AgentLoopConfig {
                max_turns,
                context,
                temperature: None,
                max_tokens: None,
                stop_on_max_turns: true,
            },
        );

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

fn cmd_tools_list() -> Result<()> {
    let mut reg = ToolRegistry::new();
    cortex_tools::register_default_tools(&mut reg)?;
    println!("Registered tools:\n");
    for spec in reg.specs() {
        println!("  {:16}  {}", spec.name, spec.description);
    }
    Ok(())
}

fn cmd_models_list(workspace: Option<PathBuf>, config: Option<PathBuf>) -> Result<()> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths, true)?;
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
