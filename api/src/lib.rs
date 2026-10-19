//! An uptime monitor, shared by the `api` binary that serves the UI and the `worker`
//! binary that runs as a Vercel Daemon.

pub mod api;
pub mod app;
pub mod checks;
pub mod config;
pub mod database;
pub mod logging;
pub mod oidc;
pub mod queue;
pub mod store;
pub mod worker;
