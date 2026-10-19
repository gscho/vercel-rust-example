use std::fmt::Display;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::Url;

use super::AppState;
use super::error::ApiError;
use super::extract::{JsonBody, MonitorId};
use crate::checks::{self, Job};
use crate::oidc;
use crate::store::{CheckResult, Monitor, MonitorUpdate, NewMonitor, Trigger, WorkerHeartbeat};

const DEFAULT_INTERVAL_SECONDS: i64 = 60;
const MIN_INTERVAL_SECONDS: i64 = 30;
const MAX_INTERVAL_SECONDS: i64 = 3600;
const MAX_NAME_CHARS: usize = 100;
const RECENT_CHECKS_PER_MONITOR: i64 = 30;
const HISTORY_WINDOW: TimeDelta = TimeDelta::hours(24);
/// A worker counts as online if its last heartbeat (sent every 15 seconds) is this recent.
const WORKER_ONLINE_WINDOW: TimeDelta = TimeDelta::seconds(45);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum MonitorStatus {
    Up,
    Down,
    Pending,
    Paused,
}

/// A monitor plus the check history the UI needs to draw it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorView {
    #[serde(flatten)]
    monitor: Monitor,
    status: MonitorStatus,
    #[serde(rename = "uptime24h")]
    uptime_24h: Option<f64>,
    /// Newest first.
    recent_checks: Vec<CheckResult>,
}

impl MonitorView {
    fn new(monitor: Monitor, recent_checks: Vec<CheckResult>, uptime_24h: Option<f64>) -> Self {
        let status = if monitor.paused {
            MonitorStatus::Paused
        } else {
            match recent_checks.first() {
                Some(latest) if latest.ok => MonitorStatus::Up,
                Some(_) => MonitorStatus::Down,
                None => MonitorStatus::Pending,
            }
        };
        Self {
            monitor,
            status,
            uptime_24h,
            recent_checks,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WorkerStatus {
    online: bool,
    instances: Vec<WorkerHeartbeat>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorRequest {
    name: Option<String>,
    url: Option<String>,
    interval_seconds: Option<i64>,
    paused: Option<bool>,
}

pub async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

pub async fn worker_status(State(state): State<AppState>) -> Result<Json<WorkerStatus>, ApiError> {
    let instances = state
        .store
        .active_workers(Utc::now() - WORKER_ONLINE_WINDOW)
        .await?;
    Ok(Json(WorkerStatus {
        online: !instances.is_empty(),
        instances,
    }))
}

pub async fn list_monitors(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let since = Utc::now() - HISTORY_WINDOW;
    let monitors = state.store.list_monitors().await?;
    let mut recent = state
        .store
        .latest_checks(since, RECENT_CHECKS_PER_MONITOR)
        .await?;
    let uptime = state.store.uptime_since(since).await?;

    let views: Vec<MonitorView> = monitors
        .into_iter()
        .map(|monitor| {
            let checks = recent.remove(&monitor.id).unwrap_or_default();
            let monitor_uptime = uptime.get(&monitor.id).copied();
            MonitorView::new(monitor, checks, monitor_uptime)
        })
        .collect();
    Ok(Json(json!({ "monitors": views })))
}

/// Returns the monitor with every check from the last 24 hours.
pub async fn get_monitor(
    State(state): State<AppState>,
    MonitorId(id): MonitorId,
) -> Result<Json<MonitorView>, ApiError> {
    let monitor = state
        .store
        .get_monitor(id)
        .await?
        .ok_or_else(ApiError::monitor_not_found)?;
    let history = state
        .store
        .checks_since(monitor.id, Utc::now() - HISTORY_WINDOW)
        .await?;

    let uptime = (!history.is_empty()).then(|| {
        let succeeded = history.iter().filter(|check| check.ok).count();
        succeeded as f64 / history.len() as f64
    });
    Ok(Json(MonitorView::new(monitor, history, uptime)))
}

pub async fn create_monitor(
    State(state): State<AppState>,
    JsonBody(request): JsonBody<MonitorRequest>,
) -> Result<(StatusCode, Json<MonitorView>), ApiError> {
    let (Some(name), Some(url)) = (request.name, request.url) else {
        return Err(ApiError::BadRequest("name and url are required"));
    };
    let new_monitor = NewMonitor {
        name: validate_name(&name)?,
        url: validate_url(&url)?,
        interval_seconds: validate_interval(
            request.interval_seconds.unwrap_or(DEFAULT_INTERVAL_SECONDS),
        )?,
        paused: request.paused.unwrap_or(false),
    };

    let monitor = state.store.create_monitor(&new_monitor).await?;
    Ok((
        StatusCode::CREATED,
        Json(MonitorView::new(monitor, Vec::new(), None)),
    ))
}

pub async fn update_monitor(
    State(state): State<AppState>,
    MonitorId(id): MonitorId,
    JsonBody(request): JsonBody<MonitorRequest>,
) -> Result<Json<Monitor>, ApiError> {
    let update = MonitorUpdate {
        name: request.name.as_deref().map(validate_name).transpose()?,
        url: request.url.as_deref().map(validate_url).transpose()?,
        interval_seconds: request
            .interval_seconds
            .map(validate_interval)
            .transpose()?,
        paused: request.paused,
    };

    let monitor = state
        .store
        .update_monitor(id, &update)
        .await?
        .ok_or_else(ApiError::monitor_not_found)?;
    Ok(Json(monitor))
}

pub async fn delete_monitor(
    State(state): State<AppState>,
    MonitorId(id): MonitorId,
) -> Result<StatusCode, ApiError> {
    if state.store.delete_monitor(id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::monitor_not_found())
    }
}

/// Queues a one-off check. The worker picks it up from the same topic as scheduled checks.
pub async fn check_now(
    State(state): State<AppState>,
    MonitorId(id): MonitorId,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let monitor = state
        .store
        .get_monitor(id)
        .await?
        .ok_or_else(ApiError::monitor_not_found)?;
    let job = Job {
        monitor_id: monitor.id,
        trigger: Trigger::Manual,
    };

    let token = oidc::token_for_request(&headers)
        .await
        .map_err(could_not_queue)?;
    checks::enqueue(&state.queue, &token, &job)
        .await
        .map_err(could_not_queue)?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "queued": true }))))
}

fn could_not_queue(error: impl Display) -> ApiError {
    tracing::error!(%error, "enqueue manual check");
    ApiError::BadGateway("could not queue the check")
}

fn validate_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(ApiError::BadRequest(
            "name must be between 1 and 100 characters",
        ));
    }
    Ok(name.to_owned())
}

/// Returns the URL in normalized form, for example with a lowercase host.
fn validate_url(url: &str) -> Result<String, ApiError> {
    Url::parse(url.trim())
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https") && url.host().is_some())
        .map(String::from)
        .ok_or(ApiError::BadRequest(
            "url must be an absolute http or https URL",
        ))
}

fn validate_interval(seconds: i64) -> Result<i32, ApiError> {
    if !(MIN_INTERVAL_SECONDS..=MAX_INTERVAL_SECONDS).contains(&seconds) {
        return Err(ApiError::BadRequest(
            "intervalSeconds must be between 30 and 3600",
        ));
    }
    Ok(i32::try_from(seconds).expect("the interval range fits in an i32"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_trimmed_and_limited_to_100_characters() {
        assert_eq!(validate_name("  Docs  ").unwrap(), "Docs");
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"é".repeat(100)).is_ok());
        assert!(validate_name(&"é".repeat(101)).is_err());
    }

    #[test]
    fn url_must_be_absolute_http_or_https() {
        assert_eq!(
            validate_url(" https://Example.com/status ").unwrap(),
            "https://example.com/status"
        );
        assert!(validate_url("http://localhost:3000").is_ok());
        assert!(validate_url("ftp://example.com").is_err());
        assert!(validate_url("/status").is_err());
        assert!(validate_url("not a url").is_err());
    }

    #[test]
    fn interval_must_be_between_30_and_3600_seconds() {
        assert_eq!(validate_interval(30).unwrap(), 30);
        assert_eq!(validate_interval(3600).unwrap(), 3600);
        assert!(validate_interval(29).is_err());
        assert!(validate_interval(3601).is_err());
        assert!(validate_interval(i64::MAX).is_err());
    }
}
