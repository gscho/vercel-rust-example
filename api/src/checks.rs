//! The check job that the API and the worker exchange through Vercel Queues, and the HTTP
//! probe the worker runs for each job.

use std::error::Error as _;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::queue::{QueueClient, QueueError};
use crate::store::Trigger;

pub const TOPIC: &str = "monitor-checks";
pub const CONSUMER_GROUP: &str = "worker";

const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const USER_AGENT: &str = "vercel-rust-example-uptime/1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub monitor_id: i64,
    pub trigger: Trigger,
}

pub async fn enqueue(queue: &QueueClient, token: &str, job: &Job) -> Result<(), QueueError> {
    let body = serde_json::to_vec(job).expect("a check job always serializes to JSON");
    queue.send(token, TOPIC, body).await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub ok: bool,
    /// `None` when the request failed before a response arrived.
    pub status_code: Option<u16>,
    pub latency: Duration,
    pub error: Option<String>,
}

pub struct Checker {
    client: reqwest::Client,
}

impl Checker {
    pub fn new() -> Result<Self, reqwest::Error> {
        // A fresh connection per check measures DNS, TCP, and TLS time the way a new
        // visitor would experience it, instead of reusing a warm connection.
        let client = reqwest::Client::builder()
            .timeout(CHECK_TIMEOUT)
            .pool_max_idle_per_host(0)
            .user_agent(USER_AGENT)
            .build()?;
        Ok(Self { client })
    }

    /// Requests the URL and treats any response below 400 as up. Latency is measured until
    /// the response headers arrive.
    pub async fn check(&self, url: &str) -> Outcome {
        let started = Instant::now();
        let result = self.client.get(url).send().await;
        let latency = started.elapsed();

        match result {
            Ok(response) => {
                let status = response.status();
                let ok = status.as_u16() < 400;
                Outcome {
                    ok,
                    status_code: Some(status.as_u16()),
                    latency,
                    error: (!ok).then(|| status.to_string()),
                }
            }
            Err(error) => Outcome {
                ok: false,
                status_code: None,
                latency,
                error: Some(describe(&error)),
            },
        }
    }
}

/// reqwest's own message, such as "error sending request for url (...)", leaves out why
/// the request failed, so the causes are appended.
fn describe(error: &reqwest::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}
