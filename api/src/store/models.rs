use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub interval_seconds: i32,
    pub paused: bool,
    pub next_check_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewMonitor {
    pub name: String,
    pub url: String,
    pub interval_seconds: i32,
    pub paused: bool,
}

/// The fields to change on a monitor. `None` leaves a field as it is.
#[derive(Debug, Clone, Default)]
pub struct MonitorUpdate {
    pub name: Option<String>,
    pub url: Option<String>,
    pub interval_seconds: Option<i32>,
    pub paused: Option<bool>,
}

impl MonitorUpdate {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.url.is_none()
            && self.interval_seconds.is_none()
            && self.paused.is_none()
    }
}

/// What started a check: the worker's schedule or the "Check now" button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Trigger {
    Scheduled,
    Manual,
}

impl Trigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown check trigger {0:?}")]
pub struct UnknownTrigger(String);

impl TryFrom<String> for Trigger {
    type Error = UnknownTrigger;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "scheduled" => Ok(Self::Scheduled),
            "manual" => Ok(Self::Manual),
            _ => Err(UnknownTrigger(value)),
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub id: i64,
    pub monitor_id: i64,
    #[serde(skip)]
    pub message_id: String,
    #[sqlx(try_from = "String")]
    pub trigger: Trigger,
    pub checked_at: DateTime<Utc>,
    pub ok: bool,
    pub status_code: Option<i32>,
    pub latency_ms: i32,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewCheckResult {
    pub monitor_id: i64,
    /// The Vercel Queues message that triggered the check.
    pub message_id: String,
    pub trigger: Trigger,
    pub checked_at: DateTime<Utc>,
    pub ok: bool,
    pub status_code: Option<i32>,
    pub latency_ms: i32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WorkerHeartbeat {
    pub instance_id: String,
    pub deployment_id: String,
    pub started_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}
