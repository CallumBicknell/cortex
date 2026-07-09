//! Request / response DTOs.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `GET /health` body.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Always "ok" when serving.
    pub status: &'static str,
    /// Package version.
    pub version: String,
}

/// `GET /v1/info` body.
#[derive(Debug, Serialize)]
pub struct InfoResponse {
    /// Version.
    pub version: String,
    /// Workspace path.
    pub workspace: String,
    /// Database path.
    pub database: String,
    /// Models config path.
    pub models_config: String,
    /// Whether API token auth is enabled.
    pub auth_required: bool,
    /// Default yolo.
    pub default_yolo: bool,
    /// Default max turns.
    pub default_max_turns: u32,
}

/// Model alias entry.
#[derive(Debug, Serialize)]
pub struct ModelInfo {
    /// Alias name.
    pub alias: String,
    /// Provider id.
    pub provider_id: String,
    /// Model id.
    pub model: String,
}

/// Tool entry.
#[derive(Debug, Serialize)]
pub struct ToolInfo {
    /// Tool name.
    pub name: String,
    /// Description.
    pub description: String,
}

/// Session list row.
#[derive(Debug, Serialize)]
pub struct SessionInfo {
    /// Session id.
    pub id: String,
    /// Workspace.
    pub workspace: String,
    /// Model.
    pub model: String,
    /// Status.
    pub status: String,
    /// Message count.
    pub message_count: u32,
    /// Created at (RFC3339).
    pub created_at: String,
    /// Updated at (RFC3339).
    pub updated_at: String,
}

/// Full session payload.
#[derive(Debug, Serialize)]
pub struct SessionDetail {
    /// Header fields.
    #[serde(flatten)]
    pub info: SessionInfo,
    /// Messages as JSON values.
    pub messages: Vec<Value>,
}

/// `POST /v1/runs` request.
#[derive(Debug, Deserialize)]
pub struct RunRequest {
    /// User prompt.
    pub prompt: String,
    /// Model alias (optional).
    #[serde(default)]
    pub model: Option<String>,
    /// Resume session id.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Auto-approve tools.
    #[serde(default)]
    pub yolo: Option<bool>,
    /// Max LLM turns.
    #[serde(default)]
    pub max_turns: Option<u32>,
    /// Explicit skill ids.
    #[serde(default)]
    pub skills: Vec<String>,
}

/// Tool result summary.
#[derive(Debug, Serialize)]
pub struct ToolResultInfo {
    /// Tool name.
    pub name: String,
    /// Whether the tool failed.
    pub is_error: bool,
    /// Truncated output.
    pub output: String,
}

/// `POST /v1/runs` response.
#[derive(Debug, Serialize)]
pub struct RunResponse {
    /// Session id.
    pub session_id: String,
    /// Run id.
    pub run_id: String,
    /// Status string.
    pub status: String,
    /// Turns used.
    pub turns: u32,
    /// Final assistant message if any.
    pub final_message: Option<String>,
    /// Duration ms.
    pub duration_ms: u64,
    /// Error if failed.
    pub error: Option<String>,
    /// Tool results this run.
    pub tool_results: Vec<ToolResultInfo>,
}
