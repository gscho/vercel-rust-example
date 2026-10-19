//! The HTTP API that the UI calls through `/api` rewrites.

mod error;
mod extract;
mod handlers;

use std::any::Any;
use std::time::Duration;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Request};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::queue::QueueClient;
use crate::store::Store;

pub use error::ApiError;

const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub queue: QueueClient,
}

/// Mounts every route under `/api` because Vercel passes the original request path through
/// to the service.
pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/healthz", get(handlers::health))
        .route("/worker", get(handlers::worker_status))
        .route(
            "/monitors",
            get(handlers::list_monitors).post(handlers::create_monitor),
        )
        .route(
            "/monitors/{id}",
            get(handlers::get_monitor)
                .patch(handlers::update_monitor)
                .delete(handlers::delete_monitor),
        )
        .route("/monitors/{id}/check", post(handlers::check_now));

    Router::new()
        .nest("/api", api)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(CatchPanicLayer::custom(panic_response))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request| {
                    tracing::info_span!(
                        "request",
                        method = %request.method(),
                        path = %request.uri().path(),
                    )
                })
                .on_request(())
                .on_response(|response: &Response, duration: Duration, _: &Span| {
                    tracing::info!(
                        status = response.status().as_u16(),
                        duration_ms = duration.as_secs_f64() * 1000.0,
                        "request"
                    );
                })
                // Handlers log their own errors, and the response line above has the status.
                .on_failure(()),
        )
        .with_state(state)
}

fn panic_response(_panic: Box<dyn Any + Send + 'static>) -> Response {
    ApiError::Internal("handler panicked".into()).into_response()
}
