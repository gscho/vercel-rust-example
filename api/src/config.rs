use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub queue_region: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("DATABASE_URL is required")]
    MissingDatabaseUrl,
    #[error("PORT must be a port number, got {0:?}")]
    InvalidPort(String),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = non_empty_env("DATABASE_URL").ok_or(ConfigError::MissingDatabaseUrl)?;
        let port = non_empty_env("PORT").unwrap_or_else(|| "3001".to_owned());
        let port = port.parse().map_err(|_| ConfigError::InvalidPort(port))?;
        let queue_region = non_empty_env("QUEUE_REGION").unwrap_or_else(|| "iad1".to_owned());

        Ok(Self {
            database_url,
            port,
            queue_region,
        })
    }
}

/// Reads an environment variable, treating an empty value the same as an unset one.
pub(crate) fn non_empty_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.is_empty())
}
