use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::future::join_all;
use tokio::time::{Interval, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::checks::{self, CONSUMER_GROUP, Checker, Job, TOPIC};
use crate::config::non_empty_env;
use crate::oidc;
use crate::queue::{Message, QueueClient};
use crate::store::{NewCheckResult, Store, Trigger, WorkerHeartbeat};

const SCHEDULE_EVERY: Duration = Duration::from_secs(5);
const HEARTBEAT_EVERY: Duration = Duration::from_secs(15);
const MIN_IDLE_POLL: Duration = Duration::from_secs(1);
const MAX_IDLE_POLL: Duration = Duration::from_secs(5);
const CLAIM_LIMIT: i64 = 100;
const BATCH_SIZE: u32 = 10;
/// Must outlast a check (10 second timeout) plus recording the result.
const VISIBILITY_TIMEOUT: Duration = Duration::from_secs(60);

/// Runs three loops: one sends a check job for each monitor that is due, one receives jobs
/// from Vercel Queues and runs them, and one writes a heartbeat so the UI can show whether
/// a worker is running.
pub struct Worker {
    store: Store,
    queue: QueueClient,
    checker: Checker,
    instance_id: String,
    deployment_id: String,
    started_at: DateTime<Utc>,
}

impl Worker {
    pub fn new(store: Store, queue: QueueClient, checker: Checker) -> Self {
        Self {
            store,
            queue,
            checker,
            instance_id: instance_id(),
            deployment_id: non_empty_env("VERCEL_DEPLOYMENT_ID").unwrap_or_default(),
            started_at: Utc::now(),
        }
    }

    /// Runs until `shutdown` is cancelled, which happens when Vercel sends SIGTERM. The loops
    /// then stop taking new work, checks already running finish and are acknowledged, and
    /// the heartbeat is removed.
    pub async fn run(&self, shutdown: &CancellationToken) -> Result<(), sqlx::Error> {
        let span = tracing::info_span!("worker", instance = %self.instance_id);
        async {
            tracing::info!("worker started");
            tokio::join!(
                self.heartbeat_loop(shutdown),
                self.schedule_loop(shutdown),
                self.consume_loop(shutdown),
            );
            tracing::info!("worker stopped");
            self.store.delete_heartbeat(&self.instance_id).await
        }
        .instrument(span)
        .await
    }

    async fn heartbeat_loop(&self, shutdown: &CancellationToken) {
        let mut ticker = ticker(HEARTBEAT_EVERY);
        loop {
            tokio::select! {
                () = shutdown.cancelled() => return,
                _ = ticker.tick() => {}
            }

            let heartbeat = WorkerHeartbeat {
                instance_id: self.instance_id.clone(),
                deployment_id: self.deployment_id.clone(),
                started_at: self.started_at,
                last_seen_at: Utc::now(),
            };
            if let Err(error) = self.store.record_heartbeat(&heartbeat).await {
                tracing::error!(%error, "record heartbeat");
            }
        }
    }

    async fn schedule_loop(&self, shutdown: &CancellationToken) {
        let mut ticker = ticker(SCHEDULE_EVERY);
        loop {
            tokio::select! {
                () = shutdown.cancelled() => return,
                _ = ticker.tick() => {}
            }
            self.schedule_due_monitors().await;
        }
    }

    async fn schedule_due_monitors(&self) {
        let monitor_ids = match self.store.claim_due_monitors(CLAIM_LIMIT).await {
            Ok(ids) if ids.is_empty() => return,
            Ok(ids) => ids,
            Err(error) => {
                tracing::error!(%error, "claim due monitors");
                return;
            }
        };

        let token = match oidc::token_from_environment().await {
            Ok(token) => token,
            Err(error) => {
                tracing::error!(%error, "enqueue scheduled checks");
                return;
            }
        };
        for monitor_id in monitor_ids {
            let job = Job {
                monitor_id,
                trigger: Trigger::Scheduled,
            };
            if let Err(error) = checks::enqueue(&self.queue, &token, &job).await {
                tracing::error!(%error, monitor = monitor_id, "enqueue scheduled check");
            }
        }
    }

    /// Polls the queue, backing off while it is empty because `receive` returns right away
    /// instead of waiting for a message.
    async fn consume_loop(&self, shutdown: &CancellationToken) {
        let mut idle = MIN_IDLE_POLL;
        while !shutdown.is_cancelled() {
            if self.receive_and_process().await > 0 {
                idle = MIN_IDLE_POLL;
                continue;
            }

            tokio::select! {
                () = shutdown.cancelled() => return,
                () = tokio::time::sleep(idle) => {}
            }
            idle = (idle * 2).min(MAX_IDLE_POLL);
        }
    }

    /// Receives one batch and runs its checks at the same time. Returns how many messages
    /// arrived. Shutdown doesn't interrupt the batch: a check that is already running
    /// should finish and be recorded rather than show up as a failure.
    async fn receive_and_process(&self) -> usize {
        let token = match oidc::token_from_environment().await {
            Ok(token) => token,
            Err(error) => {
                tracing::error!(%error, "receive checks");
                return 0;
            }
        };
        let messages = match self
            .queue
            .receive(
                &token,
                TOPIC,
                CONSUMER_GROUP,
                BATCH_SIZE,
                VISIBILITY_TIMEOUT,
            )
            .await
        {
            Ok(messages) => messages,
            Err(error) => {
                tracing::error!(%error, "receive checks");
                return 0;
            }
        };

        join_all(messages.iter().map(|message| {
            let span = tracing::info_span!(
                "message",
                message_id = %message.id,
                delivery = message.delivery_count,
            );
            self.process(&token, message).instrument(span)
        }))
        .await;
        messages.len()
    }

    async fn process(&self, token: &str, message: &Message) {
        let job: Job = match serde_json::from_slice(&message.body) {
            Ok(job) => job,
            Err(error) => {
                tracing::error!(%error, "drop malformed check job");
                self.ack(token, message).await;
                return;
            }
        };

        let monitor = match self.store.get_monitor(job.monitor_id).await {
            Ok(Some(monitor)) => monitor,
            // The monitor was deleted after the job was sent.
            Ok(None) => {
                self.ack(token, message).await;
                return;
            }
            Err(error) => {
                // Leave the message unacknowledged so Vercel Queues redelivers it.
                tracing::error!(%error, "load monitor");
                return;
            }
        };

        let outcome = self.checker.check(&monitor.url).await;
        let check = NewCheckResult {
            monitor_id: monitor.id,
            message_id: message.id.clone(),
            trigger: job.trigger,
            checked_at: Utc::now(),
            ok: outcome.ok,
            status_code: outcome.status_code.map(i32::from),
            latency_ms: i32::try_from(outcome.latency.as_millis()).unwrap_or(i32::MAX),
            error: outcome.error,
        };
        if let Err(error) = self.store.record_check(&check).await {
            tracing::error!(%error, "record check");
            return;
        }

        self.ack(token, message).await;
        tracing::info!(
            monitor = monitor.id,
            trigger = job.trigger.as_str(),
            ok = check.ok,
            latency_ms = check.latency_ms,
            "check complete"
        );
    }

    async fn ack(&self, token: &str, message: &Message) {
        let result = self
            .queue
            .ack(token, TOPIC, CONSUMER_GROUP, &message.receipt_handle)
            .await;
        if let Err(error) = result {
            tracing::error!(%error, "ack check job");
        }
    }
}

/// The container's hostname, which is unique per daemon instance.
fn instance_id() -> String {
    gethostname::gethostname()
        .into_string()
        .ok()
        .filter(|hostname| !hostname.is_empty())
        .unwrap_or_else(|| format!("worker-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]))
}

/// Fires right away and then every `period`, skipping ticks that a slow iteration missed
/// instead of firing them in a burst.
fn ticker(period: Duration) -> Interval {
    let mut ticker = tokio::time::interval(period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker
}
