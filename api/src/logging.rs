use tracing_subscriber::EnvFilter;

/// Postgres reports routine notices, such as "relation already exists, skipping" on every
/// startup migration check, at the info level.
const DEFAULT_FILTER: &str = "info,sqlx::postgres::notice=warn";

/// Writes JSON logs, which Vercel's log viewer can parse. Set `LOG_FORMAT=console` for
/// readable output during local development, and `RUST_LOG` to change the level.
pub fn init() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let subscriber = tracing_subscriber::fmt().with_env_filter(filter);

    if std::env::var("LOG_FORMAT").as_deref() == Ok("console") {
        subscriber.init();
    } else {
        subscriber
            .json()
            .flatten_event(true)
            .with_span_list(false)
            .init();
    }
}
