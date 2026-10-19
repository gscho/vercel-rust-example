//! The HTTP API. Vercel's Rust preset builds this binary as the api service (see the
//! entrypoint in vercel.json) and routes requests to the port in `PORT`.

use std::net::SocketAddr;
use std::process::ExitCode;

use anyhow::Context;
use tokio::net::TcpListener;
use uptime::api::{self, AppState};
use uptime::app::{self, Core};
use uptime::logging;

#[tokio::main]
async fn main() -> ExitCode {
    logging::init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %format!("{error:#}"), "api failed");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let Core {
        config,
        store,
        queue,
    } = Core::start().await?;

    let address = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("listen on {address}"))?;
    tracing::info!(%address, "api listening");

    let router = api::router(AppState {
        store: store.clone(),
        queue,
    });
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let signal = app::shutdown_signal().await;
            tracing::info!(signal, "stopping");
        })
        .await?;

    store.close().await;
    tracing::info!("stopped");
    Ok(())
}
