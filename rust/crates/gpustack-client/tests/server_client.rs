//! Offline endpoint and transport behavior tests for `ServerClient`.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use futures_util::stream;
use gpustack_client::{
    ClientError, SecretToken, ServerClient,
    http::{HttpRequest, HttpResponse, StreamResponse, Transport, TransportError},
    types::{ModelInstanceUpdate, WorkerCreate, WorkerStatusReport},
    watch::WatchEvent,
};
use serde_json::{Value, json};
use url::Url;

const VERSION: &str = include_str!("../../../fixtures/server-v2.2.3/live-version.response.json");
const WORKER_REGISTER_REQUEST: &str =
    include_str!("../../../fixtures/server-v2.2.3/worker-register.request.json");
const WORKER_REGISTER_RESPONSE: &str =
    include_str!("../../../fixtures/server-v2.2.3/worker-register.response.json");
const WORKERS_ME: &str = include_str!("../../../fixtures/server-v2.2.3/workers-me.response.json");
const WORKER_STATUS: &str =
    include_str!("../../../fixtures/server-v2.2.3/worker-status.request.json");
const MODELS_FULL_SYNC: &str =
    include_str!("../../../fixtures/server-v2.2.3/models-full-sync.response.json");
const MODEL_INSTANCES_FULL_SYNC: &str =
    include_str!("../../../fixtures/server-v2.2.3/model-instances-full-sync.response.json");
const MODEL_INSTANCE_UPDATE: &str =
    include_str!("../../../fixtures/server-v2.2.3/model-instance-update.request.json");
const MODEL_INSTANCE_WATCH: &str =
    include_str!("../../../fixtures/server-v2.2.3/model-instance-watch.ndjson");

#[derive(Clone, Default)]
struct MockTransport {
    state: Arc<Mutex<MockState>>,
}

#[derive(Default)]
struct MockState {
    requests: Vec<HttpRequest>,
    responses: VecDeque<Result<HttpResponse, TransportError>>,
    streams: VecDeque<Result<StreamResponse, TransportError>>,
}

impl MockTransport {
    fn push_response(&self, status: u16, body: impl Into<Vec<u8>>) {
        self.state
            .lock()
            .expect("mock lock")
            .responses
            .push_back(Ok(HttpResponse {
                status,
                content_type: Some("application/json".to_owned()),
                body: body.into(),
            }));
    }

    fn push_stream(&self, chunks: Vec<Vec<u8>>) {
        let chunks = chunks.into_iter().map(Ok);
        self.state
            .lock()
            .expect("mock lock")
            .streams
            .push_back(Ok(StreamResponse {
                status: 200,
                content_type: Some("text/event-stream; charset=utf-8".to_owned()),
                chunks: Box::pin(stream::iter(chunks)),
                error_body: None,
            }));
    }

    fn requests(&self) -> Vec<HttpRequest> {
        self.state.lock().expect("mock lock").requests.clone()
    }
}

impl Transport for MockTransport {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        {
            let mut state = self.state.lock().expect("mock lock");
            state.requests.push(request);
            state
                .responses
                .pop_front()
                .unwrap_or_else(|| Err(TransportError::Request("missing mock response".to_owned())))
        }
    }

    async fn open_stream(&self, request: HttpRequest) -> Result<StreamResponse, TransportError> {
        {
            let mut state = self.state.lock().expect("mock lock");
            state.requests.push(request);
            state
                .streams
                .pop_front()
                .unwrap_or_else(|| Err(TransportError::Request("missing mock stream".to_owned())))
        }
    }
}

fn json_item_from_page(raw: &str) -> Vec<u8> {
    let page: Value = serde_json::from_str(raw).expect("page JSON");
    serde_json::to_vec(&page["items"][0]).expect("item JSON")
}

#[tokio::test]
async fn client_uses_the_frozen_control_plane_contract() {
    let transport = MockTransport::default();
    transport.push_response(200, VERSION);
    transport.push_response(200, WORKER_REGISTER_RESPONSE);
    transport.push_response(200, WORKERS_ME);
    transport.push_response(204, Vec::new());
    transport.push_response(204, Vec::new());
    transport.push_response(200, MODELS_FULL_SYNC);
    transport.push_response(200, MODEL_INSTANCES_FULL_SYNC);
    transport.push_response(200, json_item_from_page(MODEL_INSTANCES_FULL_SYNC));
    transport.push_stream(vec![
        MODEL_INSTANCE_WATCH.as_bytes()[..47].to_vec(),
        MODEL_INSTANCE_WATCH.as_bytes()[47..].to_vec(),
    ]);

    let client = ServerClient::new(
        Url::parse("http://127.0.0.1/base/").expect("base URL"),
        transport.clone(),
    )
    .expect("client");
    let registration_token = SecretToken::new("registration-secret").expect("token");
    let worker_token = SecretToken::new("worker-secret").expect("token");
    let worker: WorkerCreate = serde_json::from_str(WORKER_REGISTER_REQUEST).expect("WorkerCreate");
    let status: WorkerStatusReport =
        serde_json::from_str(WORKER_STATUS).expect("WorkerStatusReport");
    let update: ModelInstanceUpdate =
        serde_json::from_str(MODEL_INSTANCE_UPDATE).expect("ModelInstanceUpdate");

    client
        .version()
        .await
        .expect("version")
        .verify_baseline()
        .expect("baseline");
    let registration = client
        .register(&registration_token, &worker)
        .await
        .expect("register");
    assert_ne!(registration.worker_uuid, worker.worker_uuid);
    assert_eq!(
        registration.worker_uuid,
        "00000000-0000-4000-8000-000000000002"
    );
    assert_eq!(client.me(&worker_token).await.expect("me").id, 101);
    client.heartbeat(&worker_token).await.expect("heartbeat");
    client
        .report_status(&worker_token, &status)
        .await
        .expect("status");
    assert_eq!(
        client
            .list_models(&worker_token)
            .await
            .expect("models")
            .pagination
            .page,
        -1
    );
    assert_eq!(
        client
            .list_model_instances(&worker_token)
            .await
            .expect("model instances")
            .items[0]
            .id,
        301
    );
    client
        .update_assigned_model_instance(&worker_token, 101, 301, &update)
        .await
        .expect("update assigned ModelInstance");

    let mut watch = client
        .watch_model_instances(&worker_token)
        .await
        .expect("watch");
    assert!(matches!(
        watch.next_event().await.expect("created event"),
        Some(WatchEvent::Created(instance)) if instance.id == 301
    ));
    assert_eq!(
        watch.next_event().await.expect("deleted event"),
        Some(WatchEvent::Deleted { id: 301 })
    );
    assert_eq!(watch.next_event().await.expect("end of stream"), None);

    let requests = transport.requests();
    let paths_and_queries: Vec<String> = requests
        .iter()
        .map(|request| {
            let query = request
                .url
                .query()
                .map(|query| format!("?{query}"))
                .unwrap_or_default();
            format!("{}{query}", request.url.path())
        })
        .collect();
    assert_eq!(
        paths_and_queries,
        [
            "/base/version",
            "/base/v2/workers",
            "/base/v2/workers?me=true",
            "/base/v2/worker-heartbeat",
            "/base/v2/worker-status",
            "/base/v2/models?page=-1",
            "/base/v2/model-instances?page=-1",
            "/base/v2/model-instances/301",
            "/base/v2/model-instances?watch=true",
        ]
    );
    assert!(requests[0].authorization.is_none());
    assert!(
        requests[1..]
            .iter()
            .all(|request| request.authorization.is_some())
    );
    assert!(
        requests[..8]
            .iter()
            .all(|request| request.timeout.is_some())
    );
    assert!(requests[8].timeout.is_none());
    assert_eq!(
        serde_json::from_slice::<Value>(requests[3].body.as_deref().expect("heartbeat body"))
            .expect("heartbeat JSON"),
        json!({})
    );
    let request_debug = format!("{requests:?}");
    assert!(!request_debug.contains("registration-secret"));
    assert!(!request_debug.contains("worker-secret"));
}

#[tokio::test]
async fn errors_are_bounded_and_redact_the_active_token() {
    let transport = MockTransport::default();
    let token = SecretToken::new("do-not-print-this").expect("token");
    transport.push_response(
        401,
        serde_json::to_vec(&json!({
            "code": 401,
            "reason": "Unauthorized",
            "message": "rejected do-not-print-this"
        }))
        .expect("error JSON"),
    );
    let client = ServerClient::new(Url::parse("http://127.0.0.1").expect("base URL"), transport)
        .expect("client");

    let error = client.heartbeat(&token).await.expect_err("401 response");
    assert!(matches!(error, ClientError::Api { status: 401, .. }));
    let rendered = error.to_string();
    assert!(!rendered.contains("do-not-print-this"));
    assert!(rendered.contains("<redacted-token>"));
}

#[tokio::test]
async fn update_refuses_an_instance_assigned_to_another_worker() {
    let transport = MockTransport::default();
    let client = ServerClient::new(
        Url::parse("http://127.0.0.1").expect("base URL"),
        transport.clone(),
    )
    .expect("client");
    let token = SecretToken::new("worker-secret").expect("token");
    let update: ModelInstanceUpdate =
        serde_json::from_str(MODEL_INSTANCE_UPDATE).expect("ModelInstanceUpdate");

    let error = client
        .update_assigned_model_instance(&token, 999, 301, &update)
        .await
        .expect_err("assignment mismatch");
    assert!(matches!(error, ClientError::Contract(_)));
    assert!(transport.requests().is_empty());
}
