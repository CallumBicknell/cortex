//! HTTP route handlers.

use crate::dto::*;
use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use cortex_common::SessionId;
use cortex_memory::CheckpointState;
use cortex_models::{Session, SessionStatus, TaskStatus};
use cortex_prompts::PromptCatalog;
use cortex_runtime::{AgentLoop, AgentLoopConfig, ContextBuilder, RunInput, SummarizeConfig};
use cortex_skills::{select_skills, SkillRegistry};
use cortex_tools::{AlwaysAllow, AlwaysDeny, Approver, PermissionPolicy, ToolContext};
use cortex_workspace::RepoMap;
use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::info;

fn check_auth(state: &SharedState, headers: &HeaderMap) -> ApiResult<()> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .or_else(|| headers.get("x-api-key").and_then(|v| v.to_str().ok()));
    if state.authorize(auth) {
        Ok(())
    } else {
        Err(ApiError::unauthorized(
            "missing or invalid Authorization bearer token / x-api-key",
        ))
    }
}

/// GET /health
pub async fn health(State(state): State<SharedState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: state.version.clone(),
    })
}

/// GET /v1/info
pub async fn info(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> ApiResult<Json<InfoResponse>> {
    check_auth(&state, &headers)?;
    Ok(Json(InfoResponse {
        version: state.version.clone(),
        workspace: state.workspace.display().to_string(),
        database: state.database.display().to_string(),
        models_config: state.models_config.display().to_string(),
        auth_required: state.api_token.is_some(),
        default_yolo: state.default_yolo,
        default_max_turns: state.default_max_turns,
    }))
}

/// GET /v1/models
pub async fn list_models(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<ModelInfo>>> {
    check_auth(&state, &headers)?;
    let mut out = Vec::new();
    for alias in state.registry.alias_names() {
        if let Ok(m) = state.registry.resolve(Some(&alias)) {
            out.push(ModelInfo {
                alias,
                provider_id: m.provider_id,
                model: m.model,
            });
        }
    }
    Ok(Json(out))
}

/// GET /v1/tools
pub async fn list_tools(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<ToolInfo>>> {
    check_auth(&state, &headers)?;
    let tools = state
        .tools
        .registry()
        .specs()
        .into_iter()
        .map(|s| ToolInfo {
            name: s.name,
            description: s.description,
        })
        .collect();
    Ok(Json(tools))
}

#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    20
}

/// GET /v1/sessions
pub async fn list_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<ListSessionsQuery>,
) -> ApiResult<Json<Vec<SessionInfo>>> {
    check_auth(&state, &headers)?;
    let rows = state
        .store
        .list_sessions(q.limit.clamp(1, 200))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let out = rows
        .into_iter()
        .map(|s| SessionInfo {
            id: s.id.to_string(),
            workspace: s.workspace,
            model: s.model,
            status: format!("{:?}", s.status).to_ascii_lowercase(),
            message_count: s.message_count,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        })
        .collect();
    Ok(Json(out))
}

/// GET /v1/sessions/:id
pub async fn get_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<SessionDetail>> {
    check_auth(&state, &headers)?;
    let sid = SessionId::from_str(&id).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let session = state
        .store
        .load_session(sid)
        .await
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    let message_count = session.messages.len() as u32;
    let messages = session
        .messages
        .iter()
        .filter_map(|m| serde_json::to_value(m).ok())
        .collect();
    Ok(Json(SessionDetail {
        info: SessionInfo {
            id: session.id.to_string(),
            workspace: session.workspace,
            model: session.model,
            status: format!("{:?}", session.status).to_ascii_lowercase(),
            message_count,
            created_at: session.created_at.to_rfc3339(),
            updated_at: session.updated_at.to_rfc3339(),
        },
        messages,
    }))
}

/// POST /v1/runs
pub async fn create_run(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<RunRequest>,
) -> ApiResult<Json<RunResponse>> {
    check_auth(&state, &headers)?;
    if req.prompt.trim().is_empty() {
        return Err(ApiError::bad_request("prompt must not be empty"));
    }

    let resolved = state
        .registry
        .resolve(req.model.as_deref())
        .map_err(|e| ApiError::bad_request(format!("model: {e}")))?;

    let mut session = if let Some(id) = &req.session_id {
        let sid = SessionId::from_str(id).map_err(|e| ApiError::bad_request(e.to_string()))?;
        state
            .store
            .load_session(sid)
            .await
            .map_err(|e| ApiError::not_found(e.to_string()))?
    } else {
        Session::new(
            state.workspace.to_string_lossy(),
            format!("{}/{}", resolved.provider_id, resolved.model),
        )
    };

    let yolo = req.yolo.unwrap_or(state.default_yolo);
    let max_turns = req.max_turns.unwrap_or(state.default_max_turns).max(1);
    let context = build_context(&state.workspace, &req.prompt, &req.skills);
    let cancel = CancellationToken::new();
    let tool_ctx = make_tool_context(&state.workspace, cancel.clone(), yolo, Some(session.id));

    let agent = AgentLoop::new(
        Arc::clone(&resolved.provider),
        resolved.model.clone(),
        state.tools.clone(),
        AgentLoopConfig {
            max_turns,
            context,
            summarize: SummarizeConfig::default(),
            ..Default::default()
        },
    );

    if let Ok(Some((_, s))) = state
        .store
        .latest_summary(session.id, Some("rolling"))
        .await
    {
        agent.set_rolling_summary(Some(s));
    }

    info!(
        session = %session.id,
        model = %resolved.model,
        "api run started"
    );

    let output = agent
        .run(RunInput {
            session: session.clone(),
            prompt: req.prompt,
            cancel,
            tool_ctx,
        })
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if let Some(summary) = agent.rolling_summary() {
        let _ = state
            .store
            .save_summary(output.session.id, "rolling", &summary)
            .await;
    }

    session = output.session.clone();
    session.status = match output.status {
        TaskStatus::Succeeded => SessionStatus::Completed,
        TaskStatus::Failed => SessionStatus::Failed,
        TaskStatus::Cancelled => SessionStatus::Paused,
        TaskStatus::Pending | TaskStatus::Running => SessionStatus::Active,
    };
    session.updated_at = chrono::Utc::now();

    let _ = state
        .store
        .persist_run(
            &session,
            CheckpointState {
                run_id: Some(output.run_id),
                phase: format!("{:?}", output.phase).to_ascii_lowercase(),
                turns: output.turns,
                note: output.error.clone(),
            },
            Some("api-run".into()),
        )
        .await;

    let tool_results = output
        .tool_results
        .iter()
        .map(|t| {
            let mut out = t.output.clone();
            if out.len() > 500 {
                out.truncate(500);
                out.push('…');
            }
            ToolResultInfo {
                name: t.name.clone(),
                is_error: t.is_error,
                output: out,
            }
        })
        .collect();

    Ok(Json(RunResponse {
        session_id: output.session.id.to_string(),
        run_id: output.run_id.to_string(),
        status: format!("{:?}", output.status).to_ascii_lowercase(),
        turns: output.turns,
        final_message: output.final_message,
        duration_ms: output.duration_ms,
        error: output.error,
        tool_results,
    }))
}

/// Build agent context (shared with streaming).
pub fn build_context(
    workspace: &std::path::Path,
    prompt: &str,
    skills: &[String],
) -> ContextBuilder {
    let prompts = PromptCatalog::with_builtins();
    let system = prompts
        .render("system", &Default::default())
        .unwrap_or_else(|_| cortex_runtime::DEFAULT_SYSTEM_PROMPT.to_string());
    let mut context = ContextBuilder::new(system);
    if let Some(instr) = cortex_workspace::load_project_instructions(workspace) {
        context = context.with_project_instructions(instr.to_prompt_section());
    }
    if let Ok(map) = RepoMap::build(workspace) {
        context = context.with_repo_map(&map);
        let reg = SkillRegistry::with_builtins();
        let selection = select_skills(&reg, prompt, Some(&map.project), skills);
        let mut skill_body = String::from("## Active skills\n");
        for id in &selection.skill_ids {
            skill_body.push_str(&format!("- {id}\n"));
        }
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

/// Tool context for runs (shared with streaming).
pub fn make_tool_context(
    workspace: &std::path::Path,
    cancel: CancellationToken,
    yolo: bool,
    session_id: Option<SessionId>,
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
        workspace_root: workspace.to_path_buf(),
        session_id,
        cancel,
        permissions: Arc::new(permissions),
        approver,
        default_timeout: Duration::from_secs(60),
    }
}
