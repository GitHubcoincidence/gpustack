//! Restricted Worker process orchestration for control-plane validation.

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use gpustack_client::http::{ReqwestTransport, Transport, TransportError};
use gpustack_client::types::{
    ProxyMode, SERVER_BASELINE_GIT_COMMIT, SERVER_BASELINE_VERSION, WorkerCreate,
    WorkerMaintenance, WorkerPublic, WorkerResourceStatus, WorkerStatusReport,
};
use gpustack_client::{ClientError, SecretToken, ServerClient};
use serde::Serialize;
use thiserror::Error;
use tokio::sync::watch;

use crate::config::WorkerConfig;
use crate::control::{ControlLoopConfig, ControlLoopConfigError, WorkerControlPlane};
use crate::health::{HealthServer, LivenessServer};
use crate::identity::{
    CredentialStore, IdentityBootstrap, IdentityError, IdentityRecordStore, IdentitySource,
    worker_credential_path, worker_identity_path,
};
use crate::signal::{ShutdownSignal, wait_for_shutdown};

pub const CONTROL_PLANE_ONLY_MODE: &str = "control-plane-only";
const MAINTENANCE_MESSAGE: &str = "Rust M3-D control-plane validation; scheduling disabled";
const UNASSIGNED_WORKER_UUID: &str = "00000000-0000-4000-8000-000000000000";

#[derive(Debug, Clone)]
struct ControlPlaneOnlyProfile {
    server_url: url::Url,
    worker_name: String,
    worker_hostname: String,
    worker_ip: IpAddr,
    worker_ifname: String,
    worker_port: u16,
    token_file: PathBuf,
    data_dir: PathBuf,
}

impl ControlPlaneOnlyProfile {
    fn from_config(config: &WorkerConfig) -> Result<Self, WorkerRunError> {
        Ok(Self {
            server_url: config
                .server_url
                .clone()
                .ok_or(WorkerRunError::MissingConfiguration("server_url"))?,
            worker_name: config
                .worker_name
                .clone()
                .ok_or(WorkerRunError::MissingConfiguration("worker_name"))?,
            worker_hostname: config
                .worker_hostname
                .clone()
                .ok_or(WorkerRunError::MissingConfiguration("worker_hostname"))?,
            worker_ip: config
                .worker_ip
                .ok_or(WorkerRunError::MissingConfiguration("worker_ip"))?,
            worker_ifname: config
                .worker_ifname
                .clone()
                .ok_or(WorkerRunError::MissingConfiguration("worker_ifname"))?,
            worker_port: config.worker_port,
            token_file: config
                .token_file
                .clone()
                .ok_or(WorkerRunError::MissingConfiguration("token_file"))?,
            data_dir: config.data_dir.clone(),
        })
    }

    fn listen_address(&self) -> SocketAddr {
        SocketAddr::new(self.worker_ip, self.worker_port)
    }

    fn registration_request(&self) -> WorkerCreate {
        let labels = BTreeMap::from([
            ("implementation".to_owned(), "rust".to_owned()),
            ("mode".to_owned(), CONTROL_PLANE_ONLY_MODE.to_owned()),
            ("stage".to_owned(), "m3d".to_owned()),
        ]);
        WorkerCreate {
            advertise_address: None,
            cluster_id: None,
            external_id: None,
            hostname: self.worker_hostname.clone(),
            ifname: self.worker_ifname.clone(),
            ip: self.worker_ip.to_string(),
            labels,
            machine_id: None,
            maintenance: Some(WorkerMaintenance {
                enabled: true,
                message: Some(MAINTENANCE_MESSAGE.to_owned()),
            }),
            metrics_port: None,
            name: self.worker_name.clone(),
            owner_principal_id: None,
            port: self.worker_port,
            proxy_mode: ProxyMode::Worker,
            state_message: Some(MAINTENANCE_MESSAGE.to_owned()),
            status: Some(empty_resource_status()),
            system_reserved: None,
            // v2.2.3 replaces this value on first registration. Recovery uses
            // the persisted Server UUID instead of trusting this placeholder.
            worker_uuid: UNASSIGNED_WORKER_UUID.to_owned(),
            worker_version: SERVER_BASELINE_VERSION.to_owned(),
        }
    }

    fn status_report(&self, worker_uuid: String) -> WorkerStatusReport {
        WorkerStatusReport {
            advertise_address: None,
            gateway_endpoint: None,
            hostname: self.worker_hostname.clone(),
            ifname: self.worker_ifname.clone(),
            ip: self.worker_ip.to_string(),
            machine_id: None,
            metrics_port: None,
            port: self.worker_port,
            proxy_mode: ProxyMode::Worker,
            state_message: Some(MAINTENANCE_MESSAGE.to_owned()),
            status: Some(empty_resource_status()),
            system_reserved: None,
            worker_uuid,
        }
    }
}

fn empty_resource_status() -> WorkerResourceStatus {
    WorkerResourceStatus {
        cpu: None,
        filesystem: None,
        gpu_devices: Some(Vec::new()),
        kernel: None,
        memory: None,
        os: None,
        rpc_servers: None,
        swap: None,
        uptime: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlPlaneOnlyEvent {
    Listening {
        address: SocketAddr,
    },
    IdentityEstablished {
        worker_id: i64,
        worker_name: String,
        identity_source: &'static str,
    },
    Running,
    Stopping {
        signal: &'static str,
    },
}

impl fmt::Display for ControlPlaneOnlyEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Listening { address } => write!(
                formatter,
                "mode={CONTROL_PLANE_ONLY_MODE} health_listen={address}"
            ),
            Self::IdentityEstablished {
                worker_id,
                worker_name,
                identity_source,
            } => write!(
                formatter,
                "worker_id={worker_id} worker_name={worker_name} identity_source={identity_source}"
            ),
            Self::Running => write!(
                formatter,
                "mode={CONTROL_PLANE_ONLY_MODE} state=running scheduling=disabled gpu_devices=0"
            ),
            Self::Stopping { signal } => {
                write!(formatter, "state=stopping signal={signal}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ControlPlaneOnlySummary {
    pub mode: String,
    pub server_version: String,
    pub server_git_commit: String,
    pub worker_id: i64,
    pub worker_name: String,
    pub worker_uuid: String,
    pub identity_source: String,
    pub health_address: String,
    pub shutdown_signal: String,
    pub desired_models: usize,
    pub desired_model_instances: usize,
    pub desired_model_files: usize,
    pub desired_revision: u64,
    pub heartbeat_ok: bool,
    pub status_ok: bool,
    pub full_sync_ok: bool,
    pub watch_connected: bool,
    pub reconnect_attempts: u64,
}

#[derive(Debug, Error)]
pub enum WorkerRunError {
    #[error("control-plane-only mode requires configuration field {0}")]
    MissingConfiguration(&'static str),
    #[error("failed to bind Worker liveness endpoint {address}: {source}")]
    HealthBind {
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("Worker liveness endpoint failed: {0}")]
    HealthServe(#[source] io::Error),
    #[error("Worker liveness endpoint stopped unexpectedly")]
    HealthStopped,
    #[error("Worker control plane stopped unexpectedly")]
    ControlPlaneStopped,
    #[error("control-plane-only safety invariant failed: {0}")]
    SafetyInvariant(&'static str),
    #[error("failed to wait for shutdown signal: {0}")]
    ShutdownSignal(#[source] io::Error),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    ControlConfiguration(#[from] ControlLoopConfigError),
}

/// Runs the restricted Worker until SIGINT or SIGTERM without enabling runtime work.
pub async fn run_control_plane_only<F>(
    config: &WorkerConfig,
    mut observe: F,
) -> Result<ControlPlaneOnlySummary, WorkerRunError>
where
    F: FnMut(&ControlPlaneOnlyEvent),
{
    let profile = ControlPlaneOnlyProfile::from_config(config)?;
    let address = profile.listen_address();
    // Bind before credential or Server work so a port collision cannot create a
    // registered Worker that this process is unable to expose for probing.
    let health = HealthServer::bind(address)
        .await
        .map_err(|source| WorkerRunError::HealthBind { address, source })?;
    observe(&ControlPlaneOnlyEvent::Listening {
        address: health.local_addr(),
    });

    let registration_token = CredentialStore::new(&profile.token_file).load()?;
    let transport = ReqwestTransport::new()?;
    let client = ServerClient::new(profile.server_url.clone(), transport)?;
    run_control_plane_only_session(
        profile,
        client,
        health,
        registration_token,
        wait_for_shutdown(),
        &mut observe,
    )
    .await
}

async fn run_control_plane_only_session<T, L, S, F>(
    profile: ControlPlaneOnlyProfile,
    client: ServerClient<T>,
    health: L,
    registration_token: SecretToken,
    shutdown_signal: S,
    observe: &mut F,
) -> Result<ControlPlaneOnlySummary, WorkerRunError>
where
    T: Transport + 'static,
    L: LivenessServer,
    S: Future<Output = io::Result<ShutdownSignal>>,
    F: FnMut(&ControlPlaneOnlyEvent),
{
    let health_address = health.local_addr();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let health_future = health.serve(shutdown_rx.clone());
    tokio::pin!(health_future);

    let registration_request = profile.registration_request();
    let identity_result = {
        let bootstrap = IdentityBootstrap::new(
            &client,
            CredentialStore::new(worker_credential_path(&profile.data_dir)),
            IdentityRecordStore::new(worker_identity_path(&profile.data_dir)),
        );
        let establish = bootstrap.establish(Some(&registration_token), &registration_request);
        tokio::pin!(establish);
        tokio::select! {
            result = &mut health_future => {
                return Err(match result {
                    Ok(()) => WorkerRunError::HealthStopped,
                    Err(error) => WorkerRunError::HealthServe(error),
                });
            }
            result = &mut establish => result,
        }
    };
    let identity = match identity_result {
        Ok(identity) => identity,
        Err(error) => {
            shutdown_tx.send_replace(true);
            let _ = (&mut health_future).await;
            return Err(error.into());
        }
    };
    let current_worker_result = {
        let current_worker = client.me(&identity.token);
        tokio::pin!(current_worker);
        tokio::select! {
            result = &mut health_future => {
                return Err(match result {
                    Ok(()) => WorkerRunError::HealthStopped,
                    Err(error) => WorkerRunError::HealthServe(error),
                });
            }
            result = &mut current_worker => result,
        }
    };
    let current_worker = match current_worker_result {
        Ok(worker) => worker,
        Err(error) => {
            shutdown_tx.send_replace(true);
            let _ = (&mut health_future).await;
            return Err(error.into());
        }
    };
    if let Err(error) = verify_control_plane_only_worker(&current_worker) {
        shutdown_tx.send_replace(true);
        let _ = (&mut health_future).await;
        return Err(error);
    }
    let identity_source = identity_source_name(&identity.source);
    observe(&ControlPlaneOnlyEvent::IdentityEstablished {
        worker_id: identity.id,
        worker_name: identity.name.clone(),
        identity_source,
    });

    let status = profile.status_report(identity.worker_uuid.clone());
    let (_status_tx, status_rx) = watch::channel(status);
    let (control, handle) = match WorkerControlPlane::new(
        client,
        identity.token.clone(),
        identity.id,
        ControlLoopConfig::default(),
    ) {
        Ok(value) => value,
        Err(error) => {
            shutdown_tx.send_replace(true);
            let _ = (&mut health_future).await;
            return Err(error.into());
        }
    };
    let desired_state = handle.desired_state();
    let control_health = handle.health();
    let control_future = control.run(status_rx, shutdown_rx);
    tokio::pin!(control_future);
    tokio::pin!(shutdown_signal);
    observe(&ControlPlaneOnlyEvent::Running);

    enum Termination {
        Signal(io::Result<ShutdownSignal>),
        Health(io::Result<()>),
        Control,
    }

    let termination = tokio::select! {
        signal = &mut shutdown_signal => Termination::Signal(signal),
        result = &mut health_future => Termination::Health(result),
        () = &mut control_future => Termination::Control,
    };

    let signal = match termination {
        Termination::Signal(signal) => {
            let signal = signal.map_err(WorkerRunError::ShutdownSignal)?;
            observe(&ControlPlaneOnlyEvent::Stopping {
                signal: shutdown_signal_name(signal),
            });
            shutdown_tx.send_replace(true);
            let (health_result, ()) = tokio::join!(&mut health_future, &mut control_future);
            health_result.map_err(WorkerRunError::HealthServe)?;
            signal
        }
        Termination::Health(result) => {
            shutdown_tx.send_replace(true);
            (&mut control_future).await;
            return Err(match result {
                Ok(()) => WorkerRunError::HealthStopped,
                Err(error) => WorkerRunError::HealthServe(error),
            });
        }
        Termination::Control => {
            shutdown_tx.send_replace(true);
            (&mut health_future)
                .await
                .map_err(WorkerRunError::HealthServe)?;
            return Err(WorkerRunError::ControlPlaneStopped);
        }
    };

    let desired = desired_state.borrow().clone();
    let health = control_health.borrow().clone();
    Ok(ControlPlaneOnlySummary {
        mode: CONTROL_PLANE_ONLY_MODE.to_owned(),
        server_version: SERVER_BASELINE_VERSION.to_owned(),
        server_git_commit: SERVER_BASELINE_GIT_COMMIT.to_owned(),
        worker_id: identity.id,
        worker_name: identity.name,
        worker_uuid: identity.worker_uuid,
        identity_source: identity_source.to_owned(),
        health_address: health_address.to_string(),
        shutdown_signal: shutdown_signal_name(signal).to_owned(),
        desired_models: desired.models.len(),
        desired_model_instances: desired.model_instances.len(),
        desired_model_files: desired.model_files.len(),
        desired_revision: desired.revision,
        heartbeat_ok: health.heartbeat_ok,
        status_ok: health.status_ok,
        full_sync_ok: health.full_sync_ok,
        watch_connected: health.watch_connected,
        reconnect_attempts: health.reconnect_attempts,
    })
}

fn identity_source_name(source: &IdentitySource) -> &'static str {
    match source {
        IdentitySource::Persisted => "persisted",
        IdentitySource::Registered => "registered",
        IdentitySource::Recovered => "recovered",
    }
}

fn verify_control_plane_only_worker(worker: &WorkerPublic) -> Result<(), WorkerRunError> {
    if !worker
        .maintenance
        .as_ref()
        .is_some_and(|maintenance| maintenance.enabled)
    {
        return Err(WorkerRunError::SafetyInvariant(
            "Server Worker maintenance must remain enabled",
        ));
    }
    Ok(())
}

fn shutdown_signal_name(signal: ShutdownSignal) -> &'static str {
    match signal {
        ShutdownSignal::Interrupt => "interrupt",
        ShutdownSignal::Terminate => "terminate",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use futures_util::stream;
    use gpustack_client::http::{
        HttpRequest, HttpResponse, Method, StreamResponse, TransportError,
    };
    use serde_json::Value;
    use tokio::sync::Notify;
    use url::Url;

    use super::*;

    #[derive(Clone)]
    struct MockTransport {
        requests: Arc<Mutex<Vec<HttpRequest>>>,
        stream_count: Arc<AtomicUsize>,
        heartbeat_seen: Arc<AtomicBool>,
        status_seen: Arc<AtomicBool>,
        control_plane_ready: Arc<Notify>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                requests: Arc::new(Mutex::new(Vec::new())),
                stream_count: Arc::new(AtomicUsize::new(0)),
                heartbeat_seen: Arc::new(AtomicBool::new(false)),
                status_seen: Arc::new(AtomicBool::new(false)),
                control_plane_ready: Arc::new(Notify::new()),
            }
        }

        fn requests(&self) -> Vec<HttpRequest> {
            self.requests.lock().expect("request lock").clone()
        }

        fn notify_if_ready(&self) {
            if self.stream_count.load(Ordering::SeqCst) >= 3
                && self.heartbeat_seen.load(Ordering::SeqCst)
                && self.status_seen.load(Ordering::SeqCst)
            {
                self.control_plane_ready.notify_one();
            }
        }
    }

    impl Transport for MockTransport {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            let path = request.url.path().to_owned();
            let method = request.method;
            self.requests.lock().expect("request lock").push(request);
            if path == "/v2/worker-heartbeat" {
                self.heartbeat_seen.store(true, Ordering::SeqCst);
            }
            if path == "/v2/worker-status" {
                self.status_seen.store(true, Ordering::SeqCst);
            }
            self.notify_if_ready();
            let response = match (method, path.as_str()) {
                (Method::Get, "/version") => json_response(include_bytes!(
                    "../../../fixtures/server-v2.2.3/live-version.response.json"
                )),
                (Method::Get, "/v2/workers") => json_response(include_bytes!(
                    "../../../fixtures/server-v2.2.3/worker-maintenance-me.response.json"
                )),
                (Method::Post, "/v2/workers") => json_response(include_bytes!(
                    "../../../fixtures/server-v2.2.3/worker-register.response.json"
                )),
                (Method::Post, "/v2/worker-heartbeat" | "/v2/worker-status") => HttpResponse {
                    status: 204,
                    content_type: None,
                    body: Vec::new(),
                },
                (Method::Get, "/v2/models") => json_response(include_bytes!(
                    "../../../fixtures/server-v2.2.3/models-full-sync.response.json"
                )),
                (Method::Get, "/v2/model-instances") => json_response(include_bytes!(
                    "../../../fixtures/server-v2.2.3/model-instances-full-sync.response.json"
                )),
                (Method::Get, "/v2/model-files") => json_response(include_bytes!(
                    "../../../fixtures/server-v2.2.3/model-files-full-sync.response.json"
                )),
                _ => {
                    return Err(TransportError::Request(format!(
                        "unexpected mock request {method:?} {path}"
                    )));
                }
            };
            Ok(response)
        }

        async fn open_stream(
            &self,
            request: HttpRequest,
        ) -> Result<StreamResponse, TransportError> {
            self.requests.lock().expect("request lock").push(request);
            self.stream_count.fetch_add(1, Ordering::SeqCst);
            self.notify_if_ready();
            Ok(StreamResponse {
                status: 200,
                content_type: Some("text/event-stream".to_owned()),
                chunks: Box::pin(stream::pending()),
                error_body: None,
            })
        }
    }

    struct MockLivenessServer {
        address: SocketAddr,
        started: Arc<AtomicBool>,
        stopped: Arc<AtomicBool>,
    }

    impl LivenessServer for MockLivenessServer {
        fn local_addr(&self) -> SocketAddr {
            self.address
        }

        fn serve(self, mut shutdown: watch::Receiver<bool>) -> crate::health::LivenessFuture {
            Box::pin(async move {
                self.started.store(true, Ordering::SeqCst);
                while !*shutdown.borrow() {
                    if shutdown.changed().await.is_err() {
                        break;
                    }
                }
                self.stopped.store(true, Ordering::SeqCst);
                Ok(())
            })
        }
    }

    fn json_response(body: &[u8]) -> HttpResponse {
        HttpResponse {
            status: 200,
            content_type: Some("application/json".to_owned()),
            body: body.to_vec(),
        }
    }

    fn test_config(data_dir: PathBuf) -> WorkerConfig {
        WorkerConfig {
            server_url: Some(Url::parse("http://127.0.0.1").expect("test URL")),
            worker_name: Some("rust-m3d-r760xa".to_owned()),
            worker_hostname: Some("r760xa".to_owned()),
            worker_ip: Some("127.0.0.1".parse().expect("test IP")),
            worker_ifname: Some("lo".to_owned()),
            worker_port: 20150,
            token_file: Some(data_dir.join("registration-token")),
            data_dir,
            log_dir: PathBuf::from("/tmp/gpustack-rust-test-logs"),
            model_roots: Vec::new(),
            docker_socket: PathBuf::from("/var/run/docker.sock"),
        }
    }

    #[test]
    fn profile_requires_every_control_plane_identity_field() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut config = test_config(directory.path().to_path_buf());
        config.worker_ifname = None;

        let error = ControlPlaneOnlyProfile::from_config(&config).expect_err("missing interface");

        assert!(matches!(
            error,
            WorkerRunError::MissingConfiguration("worker_ifname")
        ));
    }

    #[test]
    fn profile_rejects_a_server_worker_without_maintenance() {
        let page: gpustack_client::types::Paginated<WorkerPublic> = serde_json::from_slice(
            include_bytes!("../../../fixtures/server-v2.2.3/workers-me.response.json"),
        )
        .expect("Worker fixture");

        let error = verify_control_plane_only_worker(&page.items[0])
            .expect_err("maintenance is a scheduling safety invariant");

        assert!(matches!(error, WorkerRunError::SafetyInvariant(_)));
    }

    #[tokio::test]
    async fn session_is_maintenance_only_and_stops_cleanly() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let profile =
            ControlPlaneOnlyProfile::from_config(&test_config(directory.path().to_path_buf()))
                .expect("complete profile");
        let transport = MockTransport::new();
        let requests = transport.clone();
        let control_plane_ready = Arc::clone(&transport.control_plane_ready);
        let client = ServerClient::new(profile.server_url.clone(), transport).expect("client");
        let started = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let health = MockLivenessServer {
            address: profile.listen_address(),
            started: Arc::clone(&started),
            stopped: Arc::clone(&stopped),
        };
        let registration_token = SecretToken::new("registration-token").expect("secret token");
        let mut events = Vec::new();
        let shutdown = async move {
            control_plane_ready.notified().await;
            // The third stream becomes visible inside the mock before
            // WorkerControlPlane publishes its connected health snapshot.
            tokio::task::yield_now().await;
            Ok(ShutdownSignal::Interrupt)
        };

        let summary = run_control_plane_only_session(
            profile.clone(),
            client,
            health,
            registration_token,
            shutdown,
            &mut |event| events.push(event.clone()),
        )
        .await
        .expect("control-plane-only session");

        assert!(started.load(Ordering::SeqCst));
        assert!(stopped.load(Ordering::SeqCst));
        assert_eq!(summary.mode, CONTROL_PLANE_ONLY_MODE);
        assert_eq!(summary.identity_source, "registered");
        assert_eq!(summary.shutdown_signal, "interrupt");
        assert!(summary.heartbeat_ok);
        assert!(summary.status_ok);
        assert!(summary.full_sync_ok);
        assert!(summary.watch_connected);
        assert!(summary.desired_models > 0);
        assert!(worker_identity_path(directory.path()).exists());
        assert!(worker_credential_path(directory.path()).exists());
        assert!(events.contains(&ControlPlaneOnlyEvent::Running));

        let recorded = requests.requests();
        let registration: Value = serde_json::from_slice(
            recorded
                .iter()
                .find(|request| {
                    request.method == Method::Post && request.url.path() == "/v2/workers"
                })
                .and_then(|request| request.body.as_deref())
                .expect("registration body"),
        )
        .expect("registration JSON");
        assert_eq!(registration["maintenance"]["enabled"], true);
        assert_eq!(registration["metrics_port"], Value::Null);
        assert_eq!(registration["status"]["gpu_devices"], serde_json::json!([]));
        assert_eq!(registration["labels"]["mode"], CONTROL_PLANE_ONLY_MODE);

        let status: Value = serde_json::from_slice(
            recorded
                .iter()
                .find(|request| {
                    request.method == Method::Post && request.url.path() == "/v2/worker-status"
                })
                .and_then(|request| request.body.as_deref())
                .expect("status body"),
        )
        .expect("status JSON");
        assert_eq!(
            status["worker_uuid"],
            "00000000-0000-4000-8000-000000000002"
        );
        assert_eq!(status["status"]["gpu_devices"], serde_json::json!([]));
        assert!(!recorded.iter().any(|request| request.method == Method::Put));

        let restore_transport = MockTransport::new();
        let restore_requests = restore_transport.clone();
        let restore_ready = Arc::clone(&restore_transport.control_plane_ready);
        let restore_client =
            ServerClient::new(profile.server_url.clone(), restore_transport).expect("client");
        let restore_started = Arc::new(AtomicBool::new(false));
        let restore_stopped = Arc::new(AtomicBool::new(false));
        let restore_health = MockLivenessServer {
            address: profile.listen_address(),
            started: Arc::clone(&restore_started),
            stopped: Arc::clone(&restore_stopped),
        };
        let restore_shutdown = async move {
            restore_ready.notified().await;
            tokio::task::yield_now().await;
            Ok(ShutdownSignal::Terminate)
        };

        let restored = run_control_plane_only_session(
            profile,
            restore_client,
            restore_health,
            SecretToken::new("unused-registration-token").expect("secret token"),
            restore_shutdown,
            &mut |_| {},
        )
        .await
        .expect("restored control-plane-only session");

        assert!(restore_started.load(Ordering::SeqCst));
        assert!(restore_stopped.load(Ordering::SeqCst));
        assert_eq!(restored.identity_source, "persisted");
        assert_eq!(restored.worker_id, summary.worker_id);
        assert_eq!(restored.worker_uuid, summary.worker_uuid);
        assert_eq!(restored.shutdown_signal, "terminate");
        let restored_requests = restore_requests.requests();
        assert!(restored_requests.iter().any(|request| {
            request.method == Method::Get
                && request.url.path() == "/v2/workers"
                && request.url.query() == Some("me=true")
        }));
        assert!(!restored_requests.iter().any(|request| {
            request.method == Method::Post && request.url.path() == "/v2/workers"
        }));
        assert!(
            !restored_requests
                .iter()
                .any(|request| request.method == Method::Put)
        );
    }
}
