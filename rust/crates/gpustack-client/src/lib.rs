//! Typed GPUStack v2.2.3 control-plane client and context validation.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

mod auth;
mod client;
mod error;
pub mod http;
pub mod types;
pub mod watch;

pub use auth::{SecretToken, SecretTokenError};
pub use client::ServerClient;
pub use error::ClientError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub name: String,
    pub server_url: Url,
    pub token_file: PathBuf,
}

impl Context {
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextError {
    #[error("context name must not be empty")]
    InvalidName,
    #[error("server URL must use http or https")]
    InvalidServerUrl,
    #[error("server URL must not contain user info, query parameters, or a fragment")]
    ServerUrlContainsSensitiveOrUnsupportedData,
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
