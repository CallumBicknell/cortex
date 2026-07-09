//! Cortex CLI — `cortex run`, `cortex chat`, and helpers.

mod app;
mod approver;

use anyhow::{Context, Result};
use app::{init_tracing, load_dotenv, write_default_models_toml, AppContext, Paths};
use clap::{Parser, Subcommand};
use cortex_models::{Session, TaskStatus};
use cortex_runtime::{AgentLoop, AgentLoopConfig, ContextBuilder, RunInput};
use cortex_tools::ToolRegistry;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
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
                  cortex tools list\n  \
                  cortex models list"
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
        Commands::Init { force } => cmd_init(cli.workspace, force)?,
        Commands::Run {
            prompt,
            model,
            yolo,
            max_turns,
            json,
        } => {
            return cmd_run(
                cli.workspace,
                cli.config,
                prompt,
                model,
                yolo,
                max_turns,
                json,
            )
            .await;
        }
        Commands::Chat {
            model,
            yolo,
            max_turns,
        } => {
            return cmd_chat(cli.workspace, cli.config, model, yolo, max_turns).await;
        }
        Commands::Tools { command } => match command {
            ToolsCmd::List => cmd_tools_list()?,
        },
        Commands::Models { command } => match command {
            ModelsCmd::List => cmd_models_list(cli.workspace, cli.config)?,
        },
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_init(workspace: Option<PathBuf>, force: bool) -> Result<()> {
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

    println!("\nCortex initialized in {}", workspace.display());
    println!("Next:");
    println!("  export OPENAI_API_KEY=...   # or use ollama");
    println!("  # edit .cortex/models.toml  # set default_model");
    println!("  cortex run \"hello\"");
    Ok(())
}

async fn cmd_run(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    prompt: String,
    model: Option<String>,
    yolo: bool,
    max_turns: u32,
    json: bool,
) -> Result<ExitCode> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths, yolo)?;
    let resolved = app.resolve_model(model.as_deref())?;

    if !json {
        println!(
            "workspace: {}\ncortex:    {}\nmodel:     {} ({}/{})\n",
            app.paths.workspace.display(),
            app.paths.cortex_dir.display(),
            resolved.alias,
            resolved.provider_id,
            resolved.model
        );
    }

    let cancel = CancellationToken::new();
    // Ctrl-C cancels the run.
    let cancel_ctrl = cancel.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        cancel_ctrl.cancel();
    });

    let tool_ctx = app.tool_context(cancel.clone());
    let agent = AgentLoop::new(
        Arc::clone(&resolved.provider),
        resolved.model.clone(),
        app.tools.clone(),
        AgentLoopConfig {
            max_turns,
            context: ContextBuilder::default(),
            temperature: None,
            max_tokens: None,
            stop_on_max_turns: true,
        },
    );

    let session = Session::new(
        app.paths.workspace.to_string_lossy(),
        format!("{}/{}", resolved.provider_id, resolved.model),
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

    if json {
        let summary = serde_json::json!({
            "status": format!("{:?}", output.status).to_ascii_lowercase(),
            "run_id": output.run_id.to_string(),
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
    }

    Ok(exit_for_status(output.status))
}

async fn cmd_chat(
    workspace: Option<PathBuf>,
    config: Option<PathBuf>,
    model: Option<String>,
    yolo: bool,
    max_turns: u32,
) -> Result<ExitCode> {
    let paths = Paths::resolve(workspace, config)?;
    let app = AppContext::bootstrap(paths, yolo)?;
    let resolved = app.resolve_model(model.as_deref())?;

    println!(
        "Cortex chat — model {} ({}/{})",
        resolved.alias, resolved.provider_id, resolved.model
    );
    println!("workspace: {}", app.paths.workspace.display());
    println!("Type a message, or /quit to exit. Ctrl-C cancels the current turn.\n");

    let mut session = Session::new(
        app.paths.workspace.to_string_lossy(),
        format!("{}/{}", resolved.provider_id, resolved.model),
    );

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

        let cancel = CancellationToken::new();
        let cancel_ctrl = cancel.clone();
        let ctrl = tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancel_ctrl.cancel();
        });

        let tool_ctx = app.tool_context(cancel.clone());
        let agent = AgentLoop::new(
            Arc::clone(&resolved.provider),
            resolved.model.clone(),
            app.tools.clone(),
            AgentLoopConfig {
                max_turns,
                context: ContextBuilder::default(),
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

        // Carry full history forward.
        session = output.session.clone();
        print_run_human(&output);
        println!();
    }

    Ok(ExitCode::SUCCESS)
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

fn print_run_human(output: &cortex_runtime::RunOutput) {
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
        println!("assistant>\n{msg}\n");
    }
    if let Some(err) = &output.error {
        println!("error: {err}");
    }
    println!(
        "status={:?} turns={} duration_ms={}",
        output.status, output.turns, output.duration_ms
    );
}

fn exit_for_status(status: TaskStatus) -> ExitCode {
    match status {
        TaskStatus::Succeeded => ExitCode::SUCCESS,
        TaskStatus::Cancelled => ExitCode::from(130),
        TaskStatus::Failed | TaskStatus::Pending | TaskStatus::Running => ExitCode::from(1),
    }
}
