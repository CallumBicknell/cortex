//! Server-Sent Events streaming for agent runs.

use crate::dto::RunRequest;
use crate::error::ApiError;
use crate::routes::{build_context, make_tool_context};
use crate::state::SharedState;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use cortex_common::SessionId;
use cortex_memory::CheckpointState;
use cortex_models::{Session, SessionStatus, TaskStatus};
use cortex_runtime::{AgentLoop, AgentLoopConfig, RunInput, SummarizeConfig};
use futures::stream::{self, Stream};
use serde_json::json;
use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

fn check_auth(state: &SharedState, headers: &HeaderMap) -> Result<(), ApiError> {
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

/// POST /v1/runs/stream — SSE progress + final result.
pub async fn create_run_stream(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<RunRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    check_auth(&state, &headers)?;
    if req.prompt.trim().is_empty() {
        return Err(ApiError::bad_request("prompt must not be empty"));
    }

    let (tx, rx) = mpsc::channel::<String>(32);
    let state2 = Arc::clone(&state);
    let prompt = req.prompt.clone();

    tokio::spawn(async move {
        let send = |tx: &mpsc::Sender<String>, event: &str, data: serde_json::Value| {
            let tx = tx.clone();
            let payload = json!({"event": event, "data": data}).to_string();
            async move {
                let _ = tx.send(payload).await;
            }
        };

        send(&tx, "started", json!({"prompt_chars": prompt.len()})).await;

        let resolved = match state2.registry.resolve(req.model.as_deref()) {
            Ok(r) => r,
            Err(e) => {
                send(&tx, "error", json!({"error": e.to_string()})).await;
                return;
            }
        };

        let mut session = if let Some(id) = &req.session_id {
            match SessionId::from_str(id) {
                Ok(sid) => match state2.store.load_session(sid).await {
                    Ok(s) => s,
                    Err(e) => {
                        send(&tx, "error", json!({"error": e.to_string()})).await;
                        return;
                    }
                },
                Err(e) => {
                    send(&tx, "error", json!({"error": e.to_string()})).await;
                    return;
                }
            }
        } else {
            Session::new(
                state2.workspace.to_string_lossy(),
                format!("{}/{}", resolved.provider_id, resolved.model),
            )
        };

        send(
            &tx,
            "session",
            json!({"session_id": session.id.to_string()}),
        )
        .await;

        let yolo = req.yolo.unwrap_or(state2.default_yolo);
        let max_turns = req.max_turns.unwrap_or(state2.default_max_turns).max(1);
        let context = build_context(&state2.workspace, &prompt, &req.skills);
        let cancel = CancellationToken::new();
        let tool_ctx = make_tool_context(&state2.workspace, cancel.clone(), yolo, Some(session.id));

        let agent = AgentLoop::new(
            Arc::clone(&resolved.provider),
            resolved.model.clone(),
            state2.tools.clone(),
            AgentLoopConfig {
                max_turns,
                context,
                summarize: SummarizeConfig::default(),
                ..Default::default()
            },
        );

        if let Ok(Some((_, s))) = state2
            .store
            .latest_summary(session.id, Some("rolling"))
            .await
        {
            agent.set_rolling_summary(Some(s));
        }

        send(
            &tx,
            "running",
            json!({"model": resolved.model, "max_turns": max_turns}),
        )
        .await;

        info!(session = %session.id, "api stream run started");
        let output = match agent
            .run(RunInput {
                session: session.clone(),
                prompt,
                cancel,
                tool_ctx,
            })
            .await
        {
            Ok(o) => o,
            Err(e) => {
                send(&tx, "error", json!({"error": e.to_string()})).await;
                return;
            }
        };

        if let Some(summary) = agent.rolling_summary() {
            let _ = state2
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
        let _ = state2
            .store
            .persist_run(
                &session,
                CheckpointState {
                    run_id: Some(output.run_id),
                    phase: format!("{:?}", output.phase).to_ascii_lowercase(),
                    turns: output.turns,
                    note: output.error.clone(),
                },
                Some("api-stream".into()),
            )
            .await;

        for t in &output.tool_results {
            send(
                &tx,
                "tool",
                json!({
                    "name": t.name,
                    "is_error": t.is_error,
                    "output": t.output.chars().take(400).collect::<String>(),
                }),
            )
            .await;
        }

        send(
            &tx,
            "done",
            json!({
                "session_id": output.session.id.to_string(),
                "run_id": output.run_id.to_string(),
                "status": format!("{:?}", output.status).to_ascii_lowercase(),
                "turns": output.turns,
                "final_message": output.final_message,
                "duration_ms": output.duration_ms,
                "error": output.error,
            }),
        )
        .await;
    });

    let stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Some(payload) => {
                let event = Event::default().data(payload);
                Some((Ok::<_, Infallible>(event), rx))
            }
            None => None,
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
