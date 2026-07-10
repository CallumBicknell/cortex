//! Host bindings: provider, tools, store, context assembly.

use crate::app::{RunUpdate, UiEvent};
use crate::approver::{TuiApprovalRequest, TuiApprover};
use anyhow::{Context, Result};
use async_trait::async_trait;
use cortex_core::{EnvelopeHandler, EventBus, EventEnvelope, InMemoryEventBus};
use cortex_llm::Provider;
use cortex_memory::{CheckpointState, SessionStore};
use cortex_models::{Role, Session, SessionStatus, TaskStatus};
use cortex_prompts::PromptCatalog;
use cortex_runtime::{AgentLoop, AgentLoopConfig, ContextBuilder, RunInput, SummarizeConfig};
use cortex_skills::{select_skills, SkillRegistry};
use cortex_tools::{AlwaysAllow, Approver, PermissionPolicy, ToolContext, ToolExecutor};
use cortex_workspace::RepoMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_util::sync::CancellationToken;

/// Everything the TUI needs to run agent turns.
pub struct TuiHost {
    /// Workspace root.
    pub workspace: PathBuf,
    /// Database path (display).
    pub database: PathBuf,
    /// Model alias label.
    pub model_alias: String,
    /// Provider id.
    pub provider_id: String,
    /// Provider model id.
    pub model: String,
    /// LLM provider.
    pub provider: Arc<dyn Provider>,
    /// Tools.
    pub tools: ToolExecutor,
    /// Session store.
    pub store: SessionStore,
    /// Default max turns.
    pub max_turns: u32,
    /// Initial yolo.
    pub yolo: bool,
    /// Explicit skills (empty = auto).
    pub skills: Vec<String>,
}

impl TuiHost {
    /// Clone handles for a background run task.
    pub fn clone_for_run(&self) -> Self {
        Self {
            workspace: self.workspace.clone(),
            database: self.database.clone(),
            model_alias: self.model_alias.clone(),
            provider_id: self.provider_id.clone(),
            model: self.model.clone(),
            provider: Arc::clone(&self.provider),
            tools: self.tools.clone(),
            store: self.store.clone(),
            max_turns: self.max_turns,
            yolo: self.yolo,
            skills: self.skills.clone(),
        }
    }

    /// Build context for a user prompt.
    pub fn build_context(&self, prompt: &str, skills: &[String]) -> ContextBuilder {
        let prompts = PromptCatalog::with_builtins();
        let system = prompts
            .render("system", &Default::default())
            .unwrap_or_else(|_| cortex_runtime::DEFAULT_SYSTEM_PROMPT.to_string());
        let mut context = ContextBuilder::new(system);

        if let Some(instr) = cortex_workspace::load_project_instructions(&self.workspace) {
            context = context.with_project_instructions(instr.to_prompt_section());
        }

        if let Ok(map) = RepoMap::build(&self.workspace) {
            context = context.with_repo_map(&map);
            let project = Some(&map.project);
            let reg = SkillRegistry::with_builtins();
            let selection = select_skills(&reg, prompt, project, skills);
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
            context = context
                .with_skill_prompts(skill_body)
                .with_allowed_tools(selection.tools);
        }
        context
    }

    /// Run one agent turn, streaming live events on `tx`, and return completion via `Done`.
    pub async fn run_turn(
        &self,
        session: Session,
        prompt: String,
        yolo: bool,
        max_turns: u32,
        skills: Vec<String>,
        cancel: CancellationToken,
        tx: UnboundedSender<UiEvent>,
        approval_tx: mpsc::UnboundedSender<TuiApprovalRequest>,
    ) {
        let _ = tx.send(UiEvent::Status("running…".into()));
        let context = self.build_context(&prompt, &skills);
        let tool_ctx = self.make_tool_context(cancel.clone(), yolo, Some(session.id), approval_tx);
        let mut agent = AgentLoop::new(
            Arc::clone(&self.provider),
            self.model.clone(),
            self.tools.clone(),
            AgentLoopConfig {
                max_turns,
                context,
                summarize: SummarizeConfig::default(),
                stream_tokens: true,
                ..Default::default()
            },
        );

        let bus = Arc::new(InMemoryEventBus::new(512));
        bus.subscribe(Arc::new(TuiBusBridge { tx: tx.clone() }))
            .await;
        agent = agent.with_event_bus(bus);

        if let Ok(Some((_, s))) = self.store.latest_summary(session.id, Some("rolling")).await {
            agent.set_rolling_summary(Some(s));
        }

        let result = agent
            .run(RunInput {
                session: session.clone(),
                prompt: prompt.clone(),
                cancel,
                tool_ctx,
            })
            .await;

        let update = match result {
            Ok(output) => {
                if let Some(summary) = agent.rolling_summary() {
                    let _ = self
                        .store
                        .save_summary(output.session.id, "rolling", &summary)
                        .await;
                }
                let _ = self.persist(&output.session, &output).await;
                let mut tools_ok = 0u32;
                let mut tools_err = 0u32;
                let logs: Vec<String> = output
                    .tool_results
                    .iter()
                    .map(|t| {
                        if t.is_error {
                            tools_err += 1;
                        } else {
                            tools_ok += 1;
                        }
                        let flag = if t.is_error { "ERR" } else { "ok" };
                        let preview: String = t.output.chars().take(120).collect();
                        format!("[{flag}] {} — {preview}", t.name)
                    })
                    .collect();
                let assistant = output
                    .final_message
                    .clone()
                    .or_else(|| {
                        output
                            .session
                            .messages
                            .iter()
                            .rev()
                            .find(|m| m.role == Role::Assistant && !m.content.is_empty())
                            .map(|m| m.content.clone())
                    })
                    .unwrap_or_else(|| "(no assistant message)".into());
                RunUpdate {
                    ok: matches!(output.status, TaskStatus::Succeeded),
                    session: output.session,
                    assistant,
                    logs,
                    status: format!(
                        "{:?} · {} turns · tools {}/{} · {}ms",
                        output.status,
                        output.turns,
                        tools_ok,
                        tools_ok + tools_err,
                        output.duration_ms
                    ),
                    error: output.error,
                    turns: output.turns,
                    duration_ms: output.duration_ms,
                    tools_ok,
                    tools_err,
                    prompt_tokens: output.total_usage.prompt_tokens,
                    completion_tokens: output.total_usage.completion_tokens,
                }
            }
            Err(e) => RunUpdate {
                ok: false,
                session,
                assistant: String::new(),
                logs: Vec::new(),
                status: "failed".into(),
                error: Some(e.to_string()),
                turns: 0,
                duration_ms: 0,
                tools_ok: 0,
                tools_err: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
            },
        };
        let _ = tx.send(UiEvent::Done(Box::new(update)));
    }

    fn make_tool_context(
        &self,
        cancel: CancellationToken,
        yolo: bool,
        session_id: Option<cortex_common::SessionId>,
        approval_tx: mpsc::UnboundedSender<TuiApprovalRequest>,
    ) -> ToolContext {
        let approver: Arc<dyn Approver> = if yolo {
            Arc::new(AlwaysAllow)
        } else {
            Arc::new(TuiApprover::new(approval_tx))
        };
        let permissions = if yolo {
            PermissionPolicy::default().allow_all()
        } else {
            PermissionPolicy::default()
        };
        ToolContext {
            workspace_root: self.workspace.clone(),
            session_id,
            cancel,
            permissions: Arc::new(permissions),
            approver,
            default_timeout: Duration::from_secs(60),
        }
    }

    async fn persist(&self, session: &Session, output: &cortex_runtime::RunOutput) -> Result<()> {
        let mut session = session.clone();
        session.status = match output.status {
            TaskStatus::Succeeded => SessionStatus::Completed,
            TaskStatus::Failed => SessionStatus::Failed,
            TaskStatus::Cancelled => SessionStatus::Paused,
            TaskStatus::Pending | TaskStatus::Running => SessionStatus::Active,
        };
        session.updated_at = chrono::Utc::now();
        self.store
            .persist_run(
                &session,
                CheckpointState {
                    run_id: Some(output.run_id),
                    phase: format!("{:?}", output.phase).to_ascii_lowercase(),
                    turns: output.turns,
                    note: output.error.clone(),
                },
                Some("tui-run".into()),
            )
            .await
            .context("persist tui run")?;
        Ok(())
    }

    /// Load recent sessions for the sidebar.
    pub async fn list_sessions(&self, limit: u32) -> Result<Vec<cortex_memory::SessionSummary>> {
        self.store
            .list_sessions(limit)
            .await
            .context("list sessions")
    }

    /// Load a session by id.
    pub async fn load_session(&self, id: cortex_common::SessionId) -> Result<Session> {
        self.store.load_session(id).await.context("load session")
    }

    /// Soft-archive a session.
    pub async fn archive_session(&self, id: cortex_common::SessionId) -> Result<()> {
        self.store
            .archive_session(id)
            .await
            .context("archive session")
    }
}

/// Bridge agent event bus → TUI channel.
struct TuiBusBridge {
    tx: UnboundedSender<UiEvent>,
}

#[async_trait]
impl EnvelopeHandler for TuiBusBridge {
    async fn handle(&self, event: EventEnvelope) {
        match event.kind.as_str() {
            "agent.assistant_text_delta" => {
                if let Some(text) = event.payload.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        let _ = self.tx.send(UiEvent::StreamDelta(text.to_string()));
                    }
                }
            }
            "agent.tool_call.requested" => {
                let name = event
                    .payload
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool");
                let _ = self.tx.send(UiEvent::ToolLog(format!("→ {name}")));
                let _ = self.tx.send(UiEvent::Status(format!("tool: {name}")));
            }
            "agent.tool_call.completed" => {
                let name = event
                    .payload
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool");
                let err = event
                    .payload
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let flag = if err { "ERR" } else { "ok" };
                let _ = self.tx.send(UiEvent::ToolLog(format!("[{flag}] {name}")));
            }
            "agent.tool_call.failed" => {
                let name = event
                    .payload
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool");
                let _ = self.tx.send(UiEvent::ToolLog(format!("[ERR] {name}")));
            }
            "agent.subagent.started" => {
                let _ = self.tx.send(UiEvent::ToolLog("↳ sub-agent started".into()));
                let _ = self.tx.send(UiEvent::Status("sub-agent…".into()));
            }
            "agent.subagent.finished" => {
                let _ = self
                    .tx
                    .send(UiEvent::ToolLog("↳ sub-agent finished".into()));
            }
            "agent.loop.phase_changed" => {
                if let Some(phase) = event
                    .payload
                    .get("to")
                    .or_else(|| event.payload.get("phase"))
                    .and_then(|v| v.as_str())
                {
                    let _ = self.tx.send(UiEvent::Status(format!("phase: {phase}")));
                }
            }
            _ => {}
        }
    }
}
