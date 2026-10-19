//! Stable failures exposed by the typed Server client.

use thiserror::Error;

use crate::http::TransportError;

/// Errors produced while validating, sending, or decoding Server operations.
#[derive(Debug, Error)]
pub enum ClientError {
    /// The configured Server base URL violates the client safety contract.
    #[error("invalid Server base URL: {0}")]
    InvalidBaseUrl(&'static str),
    /// The HTTP transport failed before a valid Server response was decoded.
    #[error("HTTP transport failed: {0}")]
    Transport(#[from] TransportError),
    /// The Server returned its standard structured error response.
    #[error("Server returned HTTP {status}: {reason}: {message}")]
    Api {
        /// HTTP response status.
        status: u16,
        /// GPUStack application error code.
        code: u16,
        /// GPUStack error category.
        reason: String,
        /// Redacted operator-facing error message.
        message: String,
    },
    /// A non-success response did not match the standard error schema.
    #[error("Server returned an unrecognized HTTP {status} response: {body_preview}")]
    UnknownResponse {
        /// HTTP response status.
        status: u16,
        /// Redacted and length-limited response preview.
        body_preview: String,
    },
    /// A typed request could not be serialized.
    #[error("failed to encode {context}: {source}")]
    Encode {
        /// Static operation name that does not contain user data.
        context: &'static str,
        /// JSON serialization failure.
        #[source]
        source: serde_json::Error,
    },
    /// A Server response did not match its accepted wire schema.
    #[error("failed to decode {context}: {source}")]
    Decode {
        /// Static operation name that does not contain response data.
        context: &'static str,
        /// JSON deserialization failure.
        #[source]
        source: serde_json::Error,
    },
    /// A response or requested operation violates the frozen v2.2.3 contract.
    #[error("Server contract violation: {0}")]
    Contract(String),
    /// A watch stream is malformed, unsupported, or idle beyond its deadline.
    #[error("watch stream is invalid: {0}")]
    Watch(String),
}
