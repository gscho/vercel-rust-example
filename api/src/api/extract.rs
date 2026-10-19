//! Extractors that reject bad input with the API's JSON error format instead of axum's
//! plain-text rejections.

use axum::body::Bytes;
use axum::extract::{FromRequest, FromRequestParts, Path, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use super::ApiError;

/// The `{id}` path parameter of a monitor route.
pub struct MonitorId(pub i64);

impl<S: Send + Sync> FromRequestParts<S> for MonitorId {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(id) = Path::<String>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::BadRequest("invalid monitor id"))?;
        id.parse::<i64>()
            .ok()
            .filter(|id| *id >= 0)
            .map(MonitorId)
            .ok_or(ApiError::BadRequest("invalid monitor id"))
    }
}

/// A JSON request body of at most the router's body limit. Unlike axum's `Json`, it doesn't
/// require a `Content-Type: application/json` header.
pub struct JsonBody<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for JsonBody<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let body = Bytes::from_request(request, state)
            .await
            .map_err(|_| ApiError::BadRequest("invalid JSON body"))?;
        serde_json::from_slice(&body)
            .map(JsonBody)
            .map_err(|_| ApiError::BadRequest("invalid JSON body"))
    }
}
