use thiserror::Error;

use crate::http::TransportError;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("invalid Server base URL: {0}")]
    InvalidBaseUrl(&'static str),
    #[error("HTTP transport failed: {0}")]
    Transport(#[from] TransportError),
    #[error("Server returned HTTP {status}: {reason}: {message}")]
    Api {
        status: u16,
        code: u16,
        reason: String,
        message: String,
    },
    #[error("Server returned an unrecognized HTTP {status} response: {body_preview}")]
    UnknownResponse { status: u16, body_preview: String },
    #[error("failed to encode {context}: {source}")]
    Encode {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to decode {context}: {source}")]
    Decode {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("Server contract violation: {0}")]
    Contract(String),
    #[error("watch stream is invalid: {0}")]
    Watch(String),
}
