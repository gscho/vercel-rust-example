use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// An error response. Every error has a JSON body like `{"error": "monitor not found"}`,
/// which the UI shows to the user.
#[derive(Debug)]
pub enum ApiError {
    BadRequest(&'static str),
    NotFound(&'static str),
    BadGateway(&'static str),
    /// Logged with its cause, and reported to the client without details.
    Internal(Box<dyn std::error::Error + Send + Sync>),
}

impl ApiError {
    pub fn monitor_not_found() -> Self {
        Self::NotFound("monitor not found")
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::BadGateway(message) => (StatusCode::BAD_GATEWAY, message),
            Self::Internal(error) => {
                tracing::error!(%error, "request failed");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
