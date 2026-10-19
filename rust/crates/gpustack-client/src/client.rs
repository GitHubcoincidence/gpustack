//! Endpoint orchestration for the GPUStack v2.2.3 Worker control plane.

use std::time::Duration;

use serde::{Serialize, de::DeserializeOwned};
use url::Url;

use crate::{
    ClientError, SecretToken,
    http::{HttpRequest, HttpResponse, Method, Transport},
    types::{
        ErrorResponse, ModelInstancePublic, ModelInstanceUpdate, ModelPublic, Paginated,
        ServerVersion, WorkerCreate, WorkerPublic, WorkerRegistrationPublic, WorkerStatusReport,
    },
    watch::WatchConnection,
};

const JSON_CONTENT_TYPE: &str = "application/json";
const EVENT_STREAM_CONTENT_TYPE: &str = "text/event-stream";
const MAX_ERROR_PREVIEW_CHARS: usize = 1024;
const BUFFERED_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Typed GPUStack client with a replaceable transport for offline contract tests.
pub struct ServerClient<T> {
    base_url: Url,
    transport: T,
}

impl<T: Transport> ServerClient<T> {
    pub fn new(base_url: Url, transport: T) -> Result<Self, ClientError> {
        validate_base_url(&base_url)?;
        Ok(Self {
            base_url,
            transport,
        })
    }

    pub async fn version(&self) -> Result<ServerVersion, ClientError> {
        let response = self
            .transport
            .execute(self.request(Method::Get, "version", None, None, JSON_CONTENT_TYPE)?)
            .await?;
        self.decode_json(response, "Server version", None)
    }

    pub async fn register(
        &self,
        registration_token: &SecretToken,
        worker: &WorkerCreate,
    ) -> Result<WorkerRegistrationPublic, ClientError> {
        let body = encode_json(worker, "Worker registration request")?;
        let response = self
            .transport
            .execute(self.request(
                Method::Post,
                "v2/workers",
                Some(registration_token),
                Some(body),
                JSON_CONTENT_TYPE,
            )?)
            .await?;
        self.decode_json(
            response,
            "Worker registration response",
            Some(registration_token),
        )
    }

    pub async fn me(&self, worker_token: &SecretToken) -> Result<WorkerPublic, ClientError> {
        let response = self
            .transport
            .execute(self.request(
                Method::Get,
                "v2/workers?me=true",
                Some(worker_token),
                None,
                JSON_CONTENT_TYPE,
            )?)
            .await?;
        let mut workers: Paginated<WorkerPublic> =
            self.decode_json(response, "current Worker response", Some(worker_token))?;
        if workers.items.len() != 1 {
            return Err(ClientError::Contract(format!(
                "expected exactly one current Worker, got {}",
                workers.items.len()
            )));
        }
        Ok(workers.items.remove(0))
    }

    pub async fn heartbeat(&self, worker_token: &SecretToken) -> Result<(), ClientError> {
        let body = b"{}".to_vec();
        let response = self
            .transport
            .execute(self.request(
                Method::Post,
                "v2/worker-heartbeat",
                Some(worker_token),
                Some(body),
                JSON_CONTENT_TYPE,
            )?)
            .await?;
        self.expect_no_content(response, "Worker heartbeat", worker_token)
    }

    pub async fn report_status(
        &self,
        worker_token: &SecretToken,
        status: &WorkerStatusReport,
    ) -> Result<(), ClientError> {
        let body = encode_json(status, "Worker status request")?;
        let response = self
            .transport
            .execute(self.request(
                Method::Post,
                "v2/worker-status",
                Some(worker_token),
                Some(body),
                JSON_CONTENT_TYPE,
            )?)
            .await?;
        self.expect_no_content(response, "Worker status", worker_token)
    }

    pub async fn list_models(
        &self,
        worker_token: &SecretToken,
    ) -> Result<Paginated<ModelPublic>, ClientError> {
        let response = self
            .transport
            .execute(self.request(
                Method::Get,
                "v2/models?page=-1",
                Some(worker_token),
                None,
                JSON_CONTENT_TYPE,
            )?)
            .await?;
        self.decode_json(response, "Model full sync", Some(worker_token))
    }

    pub async fn list_model_instances(
        &self,
        worker_token: &SecretToken,
    ) -> Result<Paginated<ModelInstancePublic>, ClientError> {
        let response = self
            .transport
            .execute(self.request(
                Method::Get,
                "v2/model-instances?page=-1",
                Some(worker_token),
                None,
                JSON_CONTENT_TYPE,
            )?)
            .await?;
        self.decode_json(response, "ModelInstance full sync", Some(worker_token))
    }

    /// Performs a full update only after checking Worker ownership and R1 scope.
    pub async fn update_assigned_model_instance(
        &self,
        worker_token: &SecretToken,
        worker_id: i64,
        instance_id: i64,
        update: &ModelInstanceUpdate,
    ) -> Result<ModelInstancePublic, ClientError> {
        update.validate_for_worker(worker_id)?;
        let body = encode_json(update, "ModelInstance update request")?;
        let response = self
            .transport
            .execute(self.request(
                Method::Put,
                &format!("v2/model-instances/{instance_id}"),
                Some(worker_token),
                Some(body),
                JSON_CONTENT_TYPE,
            )?)
            .await?;
        self.decode_json(
            response,
            "ModelInstance update response",
            Some(worker_token),
        )
    }

    pub async fn watch_models(
        &self,
        worker_token: &SecretToken,
    ) -> Result<WatchConnection<ModelPublic>, ClientError> {
        self.open_watch("v2/models?watch=true", worker_token).await
    }

    pub async fn watch_model_instances(
        &self,
        worker_token: &SecretToken,
    ) -> Result<WatchConnection<ModelInstancePublic>, ClientError> {
        self.open_watch("v2/model-instances?watch=true", worker_token)
            .await
    }

    async fn open_watch<R: DeserializeOwned>(
        &self,
        path_and_query: &str,
        token: &SecretToken,
    ) -> Result<WatchConnection<R>, ClientError> {
        let response = self
            .transport
            .open_stream(self.request(
                Method::Get,
                path_and_query,
                Some(token),
                None,
                EVENT_STREAM_CONTENT_TYPE,
            )?)
            .await?;
        if !(200..300).contains(&response.status) {
            // Streaming error responses are buffered by the transport so they
            // pass through the same redaction and API-error mapping as normal calls.
            return Err(map_error_response(
                HttpResponse {
                    status: response.status,
                    content_type: response.content_type,
                    body: response.error_body.unwrap_or_default(),
                },
                Some(token),
            ));
        }
        if !response
            .content_type
            .as_deref()
            .is_some_and(|value| value.starts_with(EVENT_STREAM_CONTENT_TYPE))
        {
            return Err(ClientError::Contract(format!(
                "watch response must use {EVENT_STREAM_CONTENT_TYPE}"
            )));
        }
        Ok(WatchConnection::new(response.chunks))
    }

    fn request(
        &self,
        method: Method,
        path_and_query: &str,
        token: Option<&SecretToken>,
        body: Option<Vec<u8>>,
        accept: &'static str,
    ) -> Result<HttpRequest, ClientError> {
        let url = append_path_and_query(&self.base_url, path_and_query)?;
        Ok(HttpRequest {
            method,
            url,
            authorization: token.cloned(),
            accept,
            body,
            timeout: (accept != EVENT_STREAM_CONTENT_TYPE).then_some(BUFFERED_REQUEST_TIMEOUT),
        })
    }

    fn decode_json<R: DeserializeOwned>(
        &self,
        response: HttpResponse,
        context: &'static str,
        token: Option<&SecretToken>,
    ) -> Result<R, ClientError> {
        if !(200..300).contains(&response.status) {
            return Err(map_error_response(response, token));
        }
        serde_json::from_slice(&response.body)
            .map_err(|source| ClientError::Decode { context, source })
    }

    fn expect_no_content(
        &self,
        response: HttpResponse,
        context: &'static str,
        token: &SecretToken,
    ) -> Result<(), ClientError> {
        if response.status != 204 {
            return Err(map_error_response(response, Some(token)));
        }
        if !response.body.is_empty() {
            return Err(ClientError::Contract(format!(
                "{context} returned a body with HTTP 204"
            )));
        }
        Ok(())
    }
}

fn validate_base_url(url: &Url) -> Result<(), ClientError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ClientError::InvalidBaseUrl("scheme must be http or https"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ClientError::InvalidBaseUrl("user info is forbidden"));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(ClientError::InvalidBaseUrl(
            "query parameters and fragments are forbidden",
        ));
    }
    Ok(())
}

fn append_path_and_query(base: &Url, path_and_query: &str) -> Result<Url, ClientError> {
    // URL::join treats a leading slash as an instruction to discard any base
    // path. Explicit assembly preserves deployments hosted below a path prefix.
    let (path, query) = path_and_query
        .split_once('?')
        .map_or((path_and_query, None), |(path, query)| (path, Some(query)));
    let mut url = base.clone();
    let base_path = base.path().trim_end_matches('/');
    url.set_path(&format!("{base_path}/{}", path.trim_start_matches('/')));
    url.set_query(query);
    Ok(url)
}

fn encode_json<T: Serialize>(value: &T, context: &'static str) -> Result<Vec<u8>, ClientError> {
    serde_json::to_vec(value).map_err(|source| ClientError::Encode { context, source })
}

fn map_error_response(response: HttpResponse, token: Option<&SecretToken>) -> ClientError {
    if let Ok(error) = serde_json::from_slice::<ErrorResponse>(&response.body) {
        let message = token.map_or(error.message.clone(), |token| token.redact(&error.message));
        let reason = token.map_or(error.reason.clone(), |token| token.redact(&error.reason));
        return ClientError::Api {
            status: response.status,
            code: error.code,
            reason,
            message,
        };
    }

    // Unknown bodies are diagnostic only. Bound them before inclusion in an
    // error and redact the active token even when the body is not valid JSON.
    let preview = String::from_utf8_lossy(&response.body);
    let preview: String = preview.chars().take(MAX_ERROR_PREVIEW_CHARS).collect();
    let body_preview = token.map_or(preview.clone(), |token| token.redact(&preview));
    ClientError::UnknownResponse {
        status: response.status,
        body_preview,
    }
}
