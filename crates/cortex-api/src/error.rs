//! API error responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// JSON error body.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    /// Error message.
    pub error: String,
    /// Optional machine code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// API error.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    code: Option<String>,
}

impl ApiError {
    /// Create an error.
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            code: None,
        }
    }

    /// Bad request.
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }

    /// Unauthorized.
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, msg).with_code("unauthorized")
    }

    /// Not found.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, msg).with_code("not_found")
    }

    /// Internal error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, msg).with_code("internal")
    }

    /// Attach machine code.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.message,
            code: self.code,
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(value: anyhow::Error) -> Self {
        ApiError::internal(value.to_string())
    }
}

/// Result alias.
pub type ApiResult<T> = Result<T, ApiError>;
