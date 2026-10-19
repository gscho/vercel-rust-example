use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, PgPool};

/// Opens a small connection pool. Neon's pooled connection string works: it goes through
/// PgBouncer in transaction mode, which supports the protocol-level prepared statements
/// sqlx caches on each connection.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let options: PgConnectOptions = database_url.parse()?;
    let options = options
        // PgBouncer can reject startup parameters it doesn't track. Postgres 12 and later
        // already return floats at full precision, so sqlx doesn't need to send this one.
        .extra_float_digits(None)
        .log_slow_statements(log::LevelFilter::Warn, Duration::from_millis(500));

    PgPoolOptions::new()
        .max_connections(10)
        .idle_timeout(Duration::from_secs(5 * 60))
        .connect_with(options)
        .await
}
