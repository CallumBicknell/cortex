//! Session store: CRUD for sessions, messages, checkpoints, events.

use crate::checkpoint::{Checkpoint, CheckpointState};
use crate::error::{MemoryError, Result};
use chrono::{DateTime, Utc};
use cortex_common::{CheckpointId, SessionId};
use cortex_models::{
    Artifact, ArtifactKind, Message, Session, SessionStatus, ToolCall, ToolResult,
};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// Summary row for listing sessions without full message payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Session id.
    pub id: SessionId,
    /// Workspace path.
    pub workspace: String,
    /// Model string.
    pub model: String,
    /// Status.
    pub status: SessionStatus,
    /// Message count.
    pub message_count: u32,
    /// Created at.
    pub created_at: DateTime<Utc>,
    /// Updated at.
    pub updated_at: DateTime<Utc>,
}

/// Durable store backed by SQLite.
#[derive(Clone)]
pub struct SessionStore {
    pool: SqlitePool,
}

impl SessionStore {
    /// Wrap an existing pool (already migrated).
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Insert or update a session header and replace its messages.
    pub async fn save_session(&self, session: &Session) -> Result<()> {
        let status = session_status_str(session.status);
        let created = session.created_at.to_rfc3339();
        let updated = session.updated_at.to_rfc3339();
        let id = session.id.to_string();

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO sessions (id, workspace, model, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                workspace = excluded.workspace,
                model = excluded.model,
                status = excluded.status,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&id)
        .bind(&session.workspace)
        .bind(&session.model)
        .bind(status)
        .bind(&created)
        .bind(&updated)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM messages WHERE session_id = ?")
            .bind(&id)
            .execute(&mut *tx)
            .await?;

        for (seq, msg) in session.messages.iter().enumerate() {
            let payload = serde_json::to_string(msg)?;
            let mid = msg.id.to_string();
            let created_at = msg.created_at.to_rfc3339();
            sqlx::query(
                r#"
                INSERT INTO messages (id, session_id, seq, payload_json, created_at)
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(&mid)
            .bind(&id)
            .bind(seq as i64)
            .bind(&payload)
            .bind(&created_at)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Load a session with all messages.
    pub async fn load_session(&self, id: SessionId) -> Result<Session> {
        let id_str = id.to_string();
        let row = sqlx::query(
            r#"
            SELECT id, workspace, model, status, created_at, updated_at
            FROM sessions WHERE id = ?
            "#,
        )
        .bind(&id_str)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| MemoryError::NotFound(format!("session {id}")))?;

        let mut session = session_from_row(&row)?;
        let msg_rows = sqlx::query(
            r#"
            SELECT payload_json FROM messages
            WHERE session_id = ?
            ORDER BY seq ASC
            "#,
        )
        .bind(&id_str)
        .fetch_all(&self.pool)
        .await?;

        for m in msg_rows {
            let payload: String = m.try_get("payload_json")?;
            let message: Message = serde_json::from_str(&payload)?;
            session.messages.push(message);
        }
        Ok(session)
    }

    /// List sessions newest-first.
    pub async fn list_sessions(&self, limit: u32) -> Result<Vec<SessionSummary>> {
        let limit = limit.clamp(1, 500) as i64;
        let rows = sqlx::query(
            r#"
            SELECT s.id, s.workspace, s.model, s.status, s.created_at, s.updated_at,
                   (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) AS message_count
            FROM sessions s
            ORDER BY s.updated_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(summary_from_row).collect()
    }

    /// Soft-archive a session.
    pub async fn archive_session(&self, id: SessionId) -> Result<()> {
        let n = sqlx::query("UPDATE sessions SET status = 'archived', updated_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        if n == 0 {
            return Err(MemoryError::NotFound(format!("session {id}")));
        }
        Ok(())
    }

    /// Persist a tool call + result pair for audit.
    pub async fn save_tool_trace(
        &self,
        session_id: SessionId,
        call: &ToolCall,
        result: &ToolResult,
    ) -> Result<()> {
        let sid = session_id.to_string();
        let cid = call.id.to_string();
        let created = Utc::now().to_rfc3339();
        let input = serde_json::to_string(&call.arguments)?;
        let status = if result.is_error { "error" } else { "ok" };

        sqlx::query(
            r#"
            INSERT INTO tool_calls (id, session_id, message_id, name, input_json, status, created_at)
            VALUES (?, ?, NULL, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET status = excluded.status
            "#,
        )
        .bind(&cid)
        .bind(&sid)
        .bind(&call.name)
        .bind(&input)
        .bind(status)
        .bind(&created)
        .execute(&self.pool)
        .await?;

        let rid = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO tool_results (id, tool_call_id, session_id, output, is_error, duration_ms, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&rid)
        .bind(&cid)
        .bind(&sid)
        .bind(&result.output)
        .bind(if result.is_error { 1i64 } else { 0 })
        .bind(result.duration_ms.map(|v| v as i64))
        .bind(result.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Append a generic event row.
    pub async fn append_event(
        &self,
        session_id: Option<SessionId>,
        kind: &str,
        payload: &serde_json::Value,
        correlation_id: Option<Uuid>,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let payload_json = serde_json::to_string(payload)?;
        sqlx::query(
            r#"
            INSERT INTO events (id, session_id, kind, payload_json, correlation_id, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(session_id.map(|s| s.to_string()))
        .bind(kind)
        .bind(&payload_json)
        .bind(correlation_id.map(|c| c.to_string()))
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List recent events for a session.
    pub async fn list_events(
        &self,
        session_id: SessionId,
        limit: u32,
    ) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query(
            r#"
            SELECT id, kind, payload_json, correlation_id, created_at
            FROM events WHERE session_id = ?
            ORDER BY created_at ASC
            LIMIT ?
            "#,
        )
        .bind(session_id.to_string())
        .bind(limit.clamp(1, 10_000) as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::new();
        for r in rows {
            out.push(serde_json::json!({
                "id": r.try_get::<String, _>("id")?,
                "kind": r.try_get::<String, _>("kind")?,
                "payload": serde_json::from_str::<serde_json::Value>(
                    &r.try_get::<String, _>("payload_json")?
                )?,
                "correlation_id": r.try_get::<Option<String>, _>("correlation_id")?,
                "created_at": r.try_get::<String, _>("created_at")?,
            }));
        }
        Ok(out)
    }

    /// Save a checkpoint.
    pub async fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        let state = serde_json::to_string(&checkpoint.state)?;
        sqlx::query(
            r#"
            INSERT INTO checkpoints (id, session_id, label, loop_state_json, message_count, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(checkpoint.id.to_string())
        .bind(checkpoint.session_id.to_string())
        .bind(&checkpoint.label)
        .bind(&state)
        .bind(checkpoint.message_count as i64)
        .bind(checkpoint.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Latest checkpoint for a session, if any.
    pub async fn latest_checkpoint(&self, session_id: SessionId) -> Result<Option<Checkpoint>> {
        let row = sqlx::query(
            r#"
            SELECT id, session_id, label, loop_state_json, message_count, created_at
            FROM checkpoints WHERE session_id = ?
            ORDER BY created_at DESC LIMIT 1
            "#,
        )
        .bind(session_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| checkpoint_from_row(&r)).transpose()
    }

    /// Load checkpoint by id.
    pub async fn load_checkpoint(&self, id: CheckpointId) -> Result<Checkpoint> {
        let row = sqlx::query(
            r#"
            SELECT id, session_id, label, loop_state_json, message_count, created_at
            FROM checkpoints WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| MemoryError::NotFound(format!("checkpoint {id}")))?;
        checkpoint_from_row(&row)
    }

    /// Insert an artifact metadata row.
    pub async fn save_artifact(&self, artifact: &Artifact) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO artifacts (id, session_id, kind, name, sha256, size_bytes, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(artifact.id.to_string())
        .bind(artifact.session_id.to_string())
        .bind(artifact_kind_str(artifact.kind))
        .bind(&artifact.name)
        .bind(&artifact.sha256)
        .bind(artifact.size_bytes.map(|v| v as i64))
        .bind(artifact.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Persist a permissions / approval audit row.
    pub async fn save_permission_audit(
        &self,
        session_id: Option<SessionId>,
        tool_name: &str,
        decision: &str,
        detail: &serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO permissions_audit (id, session_id, tool_name, decision, detail_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(session_id.map(|s| s.to_string()))
        .bind(tool_name)
        .bind(decision)
        .bind(serde_json::to_string(detail)?)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Save a summary row (schema ready; generation is later).
    pub async fn save_summary(
        &self,
        session_id: SessionId,
        scope: &str,
        content: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO summaries (id, session_id, scope, content, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(session_id.to_string())
        .bind(scope)
        .bind(content)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Export a session as a JSON value (session + checkpoints + events).
    pub async fn export_session(&self, id: SessionId) -> Result<serde_json::Value> {
        let session = self.load_session(id).await?;
        let events = self.list_events(id, 5000).await?;
        let checkpoint = self.latest_checkpoint(id).await?;
        Ok(serde_json::json!({
            "session": session,
            "latest_checkpoint": checkpoint,
            "events": events,
        }))
    }

    /// Convenience: save session + checkpoint after a run.
    pub async fn persist_run(
        &self,
        session: &Session,
        state: CheckpointState,
        label: Option<String>,
    ) -> Result<Checkpoint> {
        self.save_session(session).await?;
        let cp = Checkpoint::new(session.id, state, session.message_count() as u32, label);
        self.save_checkpoint(&cp).await?;
        Ok(cp)
    }
}

fn session_status_str(s: SessionStatus) -> &'static str {
    match s {
        SessionStatus::Active => "active",
        SessionStatus::Paused => "paused",
        SessionStatus::Completed => "completed",
        SessionStatus::Failed => "failed",
        SessionStatus::Archived => "archived",
    }
}

fn parse_session_status(s: &str) -> Result<SessionStatus> {
    match s {
        "active" => Ok(SessionStatus::Active),
        "paused" => Ok(SessionStatus::Paused),
        "completed" => Ok(SessionStatus::Completed),
        "failed" => Ok(SessionStatus::Failed),
        "archived" => Ok(SessionStatus::Archived),
        other => Err(MemoryError::Invalid(format!(
            "unknown session status: {other}"
        ))),
    }
}

fn artifact_kind_str(k: ArtifactKind) -> &'static str {
    match k {
        ArtifactKind::File => "file",
        ArtifactKind::Diff => "diff",
        ArtifactKind::Log => "log",
        ArtifactKind::Other => "other",
    }
}

fn parse_rfc3339(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| MemoryError::Invalid(format!("bad timestamp {s}: {e}")))
}

fn session_from_row(row: &SqliteRow) -> Result<Session> {
    let id = SessionId::from_uuid(
        Uuid::parse_str(&row.try_get::<String, _>("id")?)
            .map_err(|e| MemoryError::Invalid(e.to_string()))?,
    );
    Ok(Session {
        id,
        workspace: row.try_get("workspace")?,
        model: row.try_get("model")?,
        status: parse_session_status(&row.try_get::<String, _>("status")?)?,
        messages: Vec::new(),
        created_at: parse_rfc3339(&row.try_get::<String, _>("created_at")?)?,
        updated_at: parse_rfc3339(&row.try_get::<String, _>("updated_at")?)?,
    })
}

fn summary_from_row(row: &SqliteRow) -> Result<SessionSummary> {
    let id = SessionId::from_uuid(
        Uuid::parse_str(&row.try_get::<String, _>("id")?)
            .map_err(|e| MemoryError::Invalid(e.to_string()))?,
    );
    Ok(SessionSummary {
        id,
        workspace: row.try_get("workspace")?,
        model: row.try_get("model")?,
        status: parse_session_status(&row.try_get::<String, _>("status")?)?,
        message_count: row.try_get::<i64, _>("message_count")? as u32,
        created_at: parse_rfc3339(&row.try_get::<String, _>("created_at")?)?,
        updated_at: parse_rfc3339(&row.try_get::<String, _>("updated_at")?)?,
    })
}

fn checkpoint_from_row(row: &SqliteRow) -> Result<Checkpoint> {
    let id = CheckpointId::from_uuid(
        Uuid::parse_str(&row.try_get::<String, _>("id")?)
            .map_err(|e| MemoryError::Invalid(e.to_string()))?,
    );
    let session_id = SessionId::from_uuid(
        Uuid::parse_str(&row.try_get::<String, _>("session_id")?)
            .map_err(|e| MemoryError::Invalid(e.to_string()))?,
    );
    let state: CheckpointState =
        serde_json::from_str(&row.try_get::<String, _>("loop_state_json")?)?;
    Ok(Checkpoint {
        id,
        session_id,
        label: row.try_get("label")?,
        state,
        message_count: row.try_get::<i64, _>("message_count")? as u32,
        created_at: parse_rfc3339(&row.try_get::<String, _>("created_at")?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sqlite;
    use cortex_models::Message;
    use tempfile::tempdir;

    #[tokio::test]
    async fn save_load_list_checkpoint() {
        let dir = tempdir().unwrap();
        let pool = open_sqlite(dir.path().join("t.db")).await.unwrap();
        let store = SessionStore::new(pool);

        let mut session = Session::new("/tmp/ws", "mock/default");
        session.push_message(Message::user("hi"));
        session.push_message(Message::assistant("hello"));
        store.save_session(&session).await.unwrap();

        let loaded = store.load_session(session.id).await.unwrap();
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].content, "hi");

        let list = store.list_sessions(10).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].message_count, 2);

        let cp = store
            .persist_run(
                &loaded,
                CheckpointState {
                    run_id: None,
                    phase: "done".into(),
                    turns: 1,
                    note: Some("test".into()),
                },
                Some("after-run".into()),
            )
            .await
            .unwrap();

        let latest = store.latest_checkpoint(session.id).await.unwrap().unwrap();
        assert_eq!(latest.id, cp.id);
        assert_eq!(latest.state.phase, "done");

        let export = store.export_session(session.id).await.unwrap();
        assert!(export["session"]["messages"].as_array().unwrap().len() == 2);
    }

    #[tokio::test]
    async fn tool_trace_and_events() {
        let dir = tempdir().unwrap();
        let store = SessionStore::new(open_sqlite(dir.path().join("t.db")).await.unwrap());
        let session = Session::new("/ws", "m");
        store.save_session(&session).await.unwrap();

        let call = ToolCall::new("read_file", serde_json::json!({"path": "a"}));
        let result = ToolResult::success(call.id, "read_file", "ok");
        store
            .save_tool_trace(session.id, &call, &result)
            .await
            .unwrap();
        store
            .append_event(
                Some(session.id),
                "test.event",
                &serde_json::json!({"x": 1}),
                None,
            )
            .await
            .unwrap();
        let events = store.list_events(session.id, 10).await.unwrap();
        assert_eq!(events.len(), 1);
    }
}
