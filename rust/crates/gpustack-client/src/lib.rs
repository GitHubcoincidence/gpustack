//! Typed GPUStack v2.2.3 control-plane client and context validation.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

mod auth;
mod client;
mod error;
/// Replaceable HTTP transport boundary used by production and mock clients.
pub mod http;
/// GPUStack v2.2.3 request and response wire types.
pub mod types;
/// JSON-line watch stream events and decoder.
pub mod watch;

pub use auth::{SecretToken, SecretTokenError};
pub use client::ServerClient;
pub use error::ClientError;

/// Named Server connection that references credentials by file path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    /// Operator-facing context name.
    pub name: String,
    /// GPUStack Server base URL without credentials or query parameters.
    pub server_url: Url,
    /// Absolute path to the credential file used by this context.
    pub token_file: PathBuf,
}

impl Context {
    /// Validates context identity, URL safety, and credential path shape.
    pub fn validate(&self) -> Result<(), ContextError> {
        if self.name.trim().is_empty() {
            return Err(ContextError::InvalidName);
        }
        if !matches!(self.server_url.scheme(), "http" | "https") {
            return Err(ContextError::InvalidServerUrl);
        }
        if !self.server_url.username().is_empty()
            || self.server_url.password().is_some()
            || self.server_url.query().is_some()
            || self.server_url.fragment().is_some()
        {
            return Err(ContextError::ServerUrlContainsSensitiveOrUnsupportedData);
        }
        if !self.token_file.is_absolute() {
            return Err(ContextError::TokenFileMustBeAbsolute);
        }
        Ok(())
    }
}

/// Validation failures for a named Server context.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextError {
    /// The context name is empty or whitespace-only.
    #[error("context name must not be empty")]
    InvalidName,
    /// The Server URL is not HTTP or HTTPS.
    #[error("server URL must use http or https")]
    InvalidServerUrl,
    /// The Server URL embeds credentials or unsupported URL components.
    #[error("server URL must not contain user info, query parameters, or a fragment")]
    ServerUrlContainsSensitiveOrUnsupportedData,
    /// The token file is not an absolute path.
    #[error("token file path must be absolute")]
    TokenFileMustBeAbsolute,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_records_a_token_file_reference_only() {
        let context = Context {
            name: "validation".to_owned(),
            server_url: Url::parse("http://127.0.0.1").expect("valid URL"),
            token_file: PathBuf::from("/run/secrets/gpustack-token"),
        };

        context.validate().expect("valid context");
        let serialized = serde_json::to_string(&context).expect("serialize context");
        assert!(serialized.contains("token_file"));
        assert!(!serialized.contains("token_value"));
    }
}
