//! Host bindings: provider, tools, store, context assembly.

use crate::app::RunUpdate;
use anyhow::{Context, Result};
use cortex_llm::Provider;
use cortex_memory::{CheckpointState, SessionStore};
use cortex_models::{Role, Session, SessionStatus, TaskStatus};
use cortex_prompts::PromptCatalog;
use cortex_runtime::{AgentLoop, AgentLoopConfig, ContextBuilder, RunInput, SummarizeConfig};
use cortex_skills::{select_skills, SkillRegistry};
use cortex_tools::{
    AlwaysAllow, AlwaysDeny, Approver, PermissionPolicy, ToolContext, ToolExecutor,
};
use cortex_workspace::RepoMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
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

    /// Run one agent turn and return a UI update.
    pub async fn run_turn(
        &self,
        session: Session,
        prompt: String,
        yolo: bool,
        max_turns: u32,
        skills: Vec<String>,
        cancel: CancellationToken,
    ) -> RunUpdate {
        let context = self.build_context(&prompt, &skills);
        let tool_ctx = self.make_tool_context(cancel.clone(), yolo, Some(session.id));
        let agent = AgentLoop::new(
            Arc::clone(&self.provider),
            self.model.clone(),
            self.tools.clone(),
            AgentLoopConfig {
                max_turns,
                context,
                summarize: SummarizeConfig::default(),
                ..Default::default()
            },
        );

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

        match result {
            Ok(output) => {
                if let Some(summary) = agent.rolling_summary() {
                    let _ = self
                        .store
                        .save_summary(output.session.id, "rolling", &summary)
                        .await;
                }
                let _ = self.persist(&output.session, &output).await;
                let logs: Vec<String> = output
                    .tool_results
                    .iter()
                    .map(|t| {
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
                        "{:?} · {} turns · {}ms",
                        output.status, output.turns, output.duration_ms
                    ),
                    error: output.error,
                }
            }
            Err(e) => RunUpdate {
                ok: false,
                session,
                assistant: String::new(),
                logs: Vec::new(),
                status: "failed".into(),
                error: Some(e.to_string()),
            },
        }
    }

    fn make_tool_context(
        &self,
        cancel: CancellationToken,
        yolo: bool,
        session_id: Option<cortex_common::SessionId>,
    ) -> ToolContext {
        let approver: Arc<dyn Approver> = if yolo {
            Arc::new(AlwaysAllow)
        } else {
            Arc::new(AlwaysDeny)
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
}
