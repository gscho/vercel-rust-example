//! Finds the Vercel OIDC token used to authenticate with Vercel Queues.
//!
//! The sources are checked in the same order as `@vercel/oidc`:
//!   - the `x-vercel-oidc-token` request header that Vercel sends to functions
//!   - the file named by `VERCEL_OIDC_TOKEN_FILE`, which Vercel refreshes for daemons
//!   - `VERCEL_OIDC_TOKEN`, which `vercel env pull` writes for local development

use axum::http::HeaderMap;

use crate::config::non_empty_env;

pub const REQUEST_HEADER: &str = "x-vercel-oidc-token";

#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    #[error("read OIDC token file {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },
    #[error("OIDC token file {0} is empty")]
    EmptyFile(String),
    #[error("no Vercel OIDC token found")]
    Missing,
}

/// Returns the token Vercel sent with the request, or the environment's token when the
/// request has none, such as under `vercel dev`.
pub async fn token_for_request(headers: &HeaderMap) -> Result<String, OidcError> {
    let request_token = headers
        .get(REQUEST_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|token| !token.is_empty());

    match request_token {
        Some(token) => Ok(token.to_owned()),
        None => token_from_environment().await,
    }
}

/// Returns the token for code that isn't handling a request, such as the worker. The file
/// is read on every call because Vercel rotates its contents.
pub async fn token_from_environment() -> Result<String, OidcError> {
    if let Some(path) = non_empty_env("VERCEL_OIDC_TOKEN_FILE") {
        let contents =
            tokio::fs::read_to_string(&path)
                .await
                .map_err(|source| OidcError::ReadFile {
                    path: path.clone(),
                    source,
                })?;
        let token = contents.trim();
        if token.is_empty() {
            return Err(OidcError::EmptyFile(path));
        }
        return Ok(token.to_owned());
    }

    non_empty_env("VERCEL_OIDC_TOKEN").ok_or(OidcError::Missing)
}
