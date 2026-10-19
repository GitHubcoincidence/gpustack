//! Minimal HTTP abstraction and the production rustls-backed transport.

use std::{future::Future, pin::Pin, time::Duration};

use futures_util::{Stream, StreamExt};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use thiserror::Error;
use url::Url;

use crate::auth::SecretToken;

const MAX_BUFFERED_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>, TransportError>> + Send + 'static>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: Method,
    pub url: Url,
    pub authorization: Option<SecretToken>,
    pub accept: &'static str,
    pub body: Option<Vec<u8>>,
    pub timeout: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub struct StreamResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub chunks: ByteStream,
    pub error_body: Option<Vec<u8>>,
}

/// HTTP boundary that keeps protocol tests independent of the real network.
pub trait Transport: Send + Sync {
    /// Executes a finite request and returns a size-bounded response body.
    fn execute(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<HttpResponse, TransportError>> + Send;

    /// Opens a successful stream without buffering its complete response.
    fn open_stream(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<StreamResponse, TransportError>> + Send;
}

#[derive(Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
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

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportError {
    #[error("failed to build HTTP client: {0}")]
    Build(String),
    #[error("HTTP request failed: {0}")]
    Request(String),
    #[error("HTTP response exceeded the buffered response limit")]
    ResponseTooLarge,
}
