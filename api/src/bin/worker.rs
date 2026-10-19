//! The background worker that runs as a Vercel Daemon. It is built from api/Dockerfile.

use std::process::ExitCode;
use std::time::Duration;

use anyhow::Context;
use tokio_util::sync::CancellationToken;
use uptime::app::{self, Core};
use uptime::checks::Checker;
use uptime::logging;
use uptime::worker::Worker;

/// Leaves time to finish running checks within the daemon's 30 second
/// `terminationGracePeriodSeconds` after SIGTERM.
const STOP_TIMEOUT: Duration = Duration::from_secs(25);

#[tokio::main]
async fn main() -> ExitCode {
    logging::init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %format!("{error:#}"), "worker failed");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let Core { store, queue, .. } = Core::start().await?;
    let worker = Worker::new(store.clone(), queue, Checker::new()?);

    let shutdown = CancellationToken::new();
    let running = tokio::spawn({
        let shutdown = shutdown.clone();
        async move { worker.run(&shutdown).await }
    });

    let signal = app::shutdown_signal().await;
    tracing::info!(signal, "stopping");
    shutdown.cancel();

    let stopped = tokio::time::timeout(STOP_TIMEOUT, running)
        .await
        .context("the worker didn't stop in time")?;
    stopped
        .context("the worker task panicked")?
        .context("remove the worker heartbeat")?;

    store.close().await;
    tracing::info!("stopped");
    Ok(())
}
