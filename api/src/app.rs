//! Startup and shutdown shared by the API and the worker.

use anyhow::Context;
use tokio::signal::unix::{SignalKind, signal};

use crate::config::Config;
use crate::database;
use crate::queue::QueueClient;
use crate::store::Store;

/// The dependencies both binaries use.
pub struct Core {
    pub config: Config,
    pub store: Store,
    pub queue: QueueClient,
}

impl Core {
    /// Connects to Postgres and brings the schema up to date, so the API and the worker
    /// both migrate before they serve requests or process checks.
    pub async fn start() -> anyhow::Result<Self> {
        let config = Config::from_env()?;
        let pool = database::connect(&config.database_url)
            .await
            .context("connect to Postgres")?;
        let store = Store::new(pool);
        store.migrate().await.context("run migrations")?;
        let queue = QueueClient::new(&config.queue_region)?;

        Ok(Self {
            config,
            store,
            queue,
        })
    }
}

/// Resolves with the signal's name on SIGTERM, which Vercel sends before it stops an
/// instance, or on Ctrl+C.
pub async fn shutdown_signal() -> &'static str {
    let mut terminate = signal(SignalKind::terminate()).expect("install the SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => "SIGINT",
        _ = terminate.recv() => "SIGTERM",
    }
}
