//! Minimal HTTP abstraction and the production rustls-backed transport.

use std::{future::Future, pin::Pin, time::Duration};

use futures_util::{Stream, StreamExt};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use thiserror::Error;
use url::Url;

use crate::auth::SecretToken;

const MAX_BUFFERED_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

/// Asynchronous stream of raw response chunks used by watch connections.
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>, TransportError>> + Send + 'static>>;

/// HTTP methods required by the R1 Worker protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Read an endpoint or open a watch stream.
    Get,
    /// Create a resource or report Worker liveness/status.
    Post,
    /// Replace an existing ModelInstance resource.
    Put,
}

/// Transport-neutral request assembled by `ServerClient`.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// HTTP method.
    pub method: Method,
    /// Fully resolved request URL.
    pub url: Url,
    /// Optional bearer token kept in a redacting wrapper.
    pub authorization: Option<SecretToken>,
    /// Expected response media type.
    pub accept: &'static str,
    /// Optional JSON request body.
    pub body: Option<Vec<u8>>,
    /// Per-request deadline; watch streams deliberately have no total deadline.
    pub timeout: Option<Duration>,
}

/// Fully buffered HTTP response for finite control-plane operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP response status.
    pub status: u16,
    /// Response `Content-Type`, when valid UTF-8 and present.
    pub content_type: Option<String>,
    /// Response bytes bounded by the transport buffer limit.
    pub body: Vec<u8>,
}

/// Open watch response with either streaming chunks or a buffered error body.
pub struct StreamResponse {
    /// HTTP response status.
    pub status: u16,
    /// Response `Content-Type`, when valid UTF-8 and present.
    pub content_type: Option<String>,
    /// Success response byte stream; empty for non-success responses.
    pub chunks: ByteStream,
    /// Bounded non-success body used for normal API error decoding.
    pub error_body: Option<Vec<u8>>,
}

/// Replaceable HTTP boundary used to keep protocol tests offline.
pub trait Transport: Send + Sync {
    /// Executes a finite request and buffers its bounded response.
    fn execute(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<HttpResponse, TransportError>> + Send;

    /// Opens a streaming response without buffering successful watch data.
    fn open_stream(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<StreamResponse, TransportError>> + Send;
}

/// Production HTTP transport backed by reqwest and rustls.
#[derive(Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// Builds the production client with a bounded connection timeout.
    pub fn new() -> Result<Self, TransportError> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .user_agent("gpustack-rs-worker/0.1")
            .build()
            .map_err(|error| TransportError::Build(error.to_string()))?;
        Ok(Self { client })
    }

    fn request_builder(&self, request: &HttpRequest) -> reqwest::RequestBuilder {
        let method = match request.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
        };
        let mut builder = self
            .client
            .request(method, request.url.clone())
            .header(ACCEPT, request.accept);
        if let Some(token) = &request.authorization {
            builder = builder.bearer_auth(token.expose());
        }
        if let Some(body) = &request.body {
            builder = builder
                .header(CONTENT_TYPE, "application/json")
                .body(body.clone());
        }
        if let Some(timeout) = request.timeout {
            builder = builder.timeout(timeout);
        }
        builder
    }
}

impl Transport for ReqwestTransport {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let response = self
            .request_builder(&request)
            .send()
            .await
            .map_err(|error| TransportError::Request(error.to_string()))?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if response
            .content_length()
            .is_some_and(|length| length > MAX_BUFFERED_RESPONSE_BYTES as u64)
        {
            return Err(TransportError::ResponseTooLarge);
        }
        let body = response
            .bytes()
            .await
            .map_err(|error| TransportError::Request(error.to_string()))?
            .to_vec();
        if body.len() > MAX_BUFFERED_RESPONSE_BYTES {
            return Err(TransportError::ResponseTooLarge);
        }
        Ok(HttpResponse {
            status,
            content_type,
            body,
        })
    }

    async fn open_stream(&self, request: HttpRequest) -> Result<StreamResponse, TransportError> {
        let response = self
            .request_builder(&request)
            .send()
            .await
            .map_err(|error| TransportError::Request(error.to_string()))?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if !(200..300).contains(&status) {
            // Non-success watch responses are finite API errors rather than
            // event streams, so buffer them under the same size limit as execute().
            if response
                .content_length()
                .is_some_and(|length| length > MAX_BUFFERED_RESPONSE_BYTES as u64)
            {
                return Err(TransportError::ResponseTooLarge);
            }
            let body = response
                .bytes()
                .await
                .map_err(|error| TransportError::Request(error.to_string()))?
                .to_vec();
            if body.len() > MAX_BUFFERED_RESPONSE_BYTES {
                return Err(TransportError::ResponseTooLarge);
            }
            return Ok(StreamResponse {
                status,
                content_type,
                chunks: Box::pin(futures_util::stream::empty()),
                error_body: Some(body),
            });
        }
        let chunks = response.bytes_stream().map(|result| {
            result
                .map(|bytes| bytes.to_vec())
                .map_err(|error| TransportError::Request(error.to_string()))
        });
        Ok(StreamResponse {
            status,
            content_type,
            chunks: Box::pin(chunks),
            error_body: None,
        })
    }
}

/// Failures raised by the transport before typed protocol decoding.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// The underlying HTTP client could not be constructed.
    #[error("failed to build HTTP client: {0}")]
    Build(String),
    /// Sending a request or reading its body failed.
    #[error("HTTP request failed: {0}")]
    Request(String),
    /// A finite response exceeded the 32 MiB safety limit.
    #[error("HTTP response exceeded the buffered response limit")]
    ResponseTooLarge,
}
