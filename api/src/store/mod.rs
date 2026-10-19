//! Postgres queries for monitors, check results, and worker heartbeats.

mod models;

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::migrate::MigrateError;

pub use models::{
    CheckResult, Monitor, MonitorUpdate, NewCheckResult, NewMonitor, Trigger, UnknownTrigger,
    WorkerHeartbeat,
};

/// An arbitrary advisory lock key shared by every process that migrates.
const MIGRATION_LOCK_ID: i64 = 727001;

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Runs the migrations in `migrations/` inside one transaction that holds an advisory
    /// lock, so API instances and the worker that start at the same time migrate one after
    /// another instead of racing to create tables.
    ///
    /// sqlx's own migration lock is session-level, which Neon's pooled connections
    /// (PgBouncer in transaction mode) don't support, so it's replaced by a
    /// transaction-level lock.
    pub async fn migrate(&self) -> Result<(), MigrateError> {
        let mut migrator = sqlx::migrate!();
        migrator.set_locking(false);

        let mut transaction = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(MIGRATION_LOCK_ID)
            .execute(&mut *transaction)
            .await?;
        migrator.run(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn list_monitors(&self) -> Result<Vec<Monitor>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM monitors ORDER BY created_at")
            .fetch_all(&self.pool)
            .await
    }

    pub async fn get_monitor(&self, id: i64) -> Result<Option<Monitor>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM monitors WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Creates a monitor that is due right away, so the worker checks it on its next pass.
    pub async fn create_monitor(&self, monitor: &NewMonitor) -> Result<Monitor, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO monitors (name, url, interval_seconds, paused, next_check_at)
             VALUES ($1, $2, $3, $4, now())
             RETURNING *",
        )
        .bind(&monitor.name)
        .bind(&monitor.url)
        .bind(monitor.interval_seconds)
        .bind(monitor.paused)
        .fetch_one(&self.pool)
        .await
    }

    /// Applies the update and returns the monitor, or `None` when it doesn't exist.
    /// Resuming a monitor makes it due right away.
    pub async fn update_monitor(
        &self,
        id: i64,
        update: &MonitorUpdate,
    ) -> Result<Option<Monitor>, sqlx::Error> {
        if update.is_empty() {
            return self.get_monitor(id).await;
        }

        sqlx::query_as(
            "UPDATE monitors SET
                 name = COALESCE($2, name),
                 url = COALESCE($3, url),
                 interval_seconds = COALESCE($4, interval_seconds),
                 paused = COALESCE($5, paused),
                 next_check_at = CASE WHEN $5 = FALSE THEN now() ELSE next_check_at END,
                 updated_at = now()
             WHERE id = $1
             RETURNING *",
        )
        .bind(id)
        .bind(&update.name)
        .bind(&update.url)
        .bind(update.interval_seconds)
        .bind(update.paused)
        .fetch_optional(&self.pool)
        .await
    }

    /// Deletes the monitor and its checks. Returns `false` when it doesn't exist.
    pub async fn delete_monitor(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM monitors WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Moves each due monitor's next check forward by its interval and returns the claimed
    /// IDs. SKIP LOCKED lets several worker instances claim at once without picking the
    /// same monitor.
    pub async fn claim_due_monitors(&self, limit: i64) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query_scalar(
            "UPDATE monitors
             SET next_check_at = now() + make_interval(secs => interval_seconds)
             WHERE id IN (
                 SELECT id FROM monitors
                 WHERE NOT paused AND next_check_at <= now()
                 ORDER BY next_check_at
                 LIMIT $1
                 FOR UPDATE SKIP LOCKED
             )
             RETURNING id",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Stores a check result. A result for a message that was already recorded is ignored.
    pub async fn record_check(&self, check: &NewCheckResult) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO check_results
                 (monitor_id, message_id, trigger, checked_at, ok, status_code, latency_ms, error)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (message_id) DO NOTHING",
        )
        .bind(check.monitor_id)
        .bind(&check.message_id)
        .bind(check.trigger.as_str())
        .bind(check.checked_at)
        .bind(check.ok)
        .bind(check.status_code)
        .bind(check.latency_ms)
        .bind(&check.error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns a monitor's checks newer than `since`, newest first.
    pub async fn checks_since(
        &self,
        monitor_id: i64,
        since: DateTime<Utc>,
    ) -> Result<Vec<CheckResult>, sqlx::Error> {
        sqlx::query_as(
            "SELECT * FROM check_results
             WHERE monitor_id = $1 AND checked_at > $2
             ORDER BY checked_at DESC",
        )
        .bind(monitor_id)
        .bind(since)
        .fetch_all(&self.pool)
        .await
    }

    /// Returns up to `per_monitor` of each monitor's newest checks since the given time,
    /// newest first, keyed by monitor ID.
    pub async fn latest_checks(
        &self,
        since: DateTime<Utc>,
        per_monitor: i64,
    ) -> Result<HashMap<i64, Vec<CheckResult>>, sqlx::Error> {
        let checks: Vec<CheckResult> = sqlx::query_as(
            "SELECT id, monitor_id, message_id, trigger, checked_at, ok, status_code, latency_ms, error
             FROM (
                 SELECT *, row_number() OVER (PARTITION BY monitor_id ORDER BY checked_at DESC) AS position
                 FROM check_results
                 WHERE checked_at > $1
             ) ranked
             WHERE position <= $2
             ORDER BY monitor_id, checked_at DESC",
        )
        .bind(since)
        .bind(per_monitor)
        .fetch_all(&self.pool)
        .await?;

        let mut by_monitor: HashMap<i64, Vec<CheckResult>> = HashMap::new();
        for check in checks {
            by_monitor.entry(check.monitor_id).or_default().push(check);
        }
        Ok(by_monitor)
    }

    /// Returns the share of successful checks for each monitor, from 0 to 1.
    pub async fn uptime_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<HashMap<i64, f64>, sqlx::Error> {
        let rows: Vec<(i64, f64)> = sqlx::query_as(
            "SELECT monitor_id, avg(CASE WHEN ok THEN 1 ELSE 0 END)::float8 AS uptime
             FROM check_results
             WHERE checked_at > $1
             GROUP BY monitor_id",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn record_heartbeat(&self, heartbeat: &WorkerHeartbeat) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO worker_heartbeats (instance_id, deployment_id, started_at, last_seen_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (instance_id) DO UPDATE SET last_seen_at = EXCLUDED.last_seen_at",
        )
        .bind(&heartbeat.instance_id)
        .bind(&heartbeat.deployment_id)
        .bind(heartbeat.started_at)
        .bind(heartbeat.last_seen_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_heartbeat(&self, instance_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM worker_heartbeats WHERE instance_id = $1")
            .bind(instance_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn active_workers(
        &self,
        seen_since: DateTime<Utc>,
    ) -> Result<Vec<WorkerHeartbeat>, sqlx::Error> {
        sqlx::query_as(
            "SELECT * FROM worker_heartbeats WHERE last_seen_at > $1 ORDER BY started_at",
        )
        .bind(seen_since)
        .fetch_all(&self.pool)
        .await
    }
}
