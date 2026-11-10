//! Unified Worker startup, lifecycle supervision, and operational events.

use std::collections::BTreeMap;
use std::fmt;
use std::future::IntoFuture;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpustack_client::http::{ReqwestTransport, TransportError};
use gpustack_client::types::{
    ProxyMode, SERVER_BASELINE_GIT_COMMIT, SERVER_BASELINE_VERSION, SystemReserved, WorkerCreate,
    WorkerPublic, WorkerStatusReport,
};
use gpustack_client::{ClientError, ServerClient};
use serde::Serialize;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};

use crate::config::WorkerConfig;
use crate::control::{ControlLoopConfig, ControlLoopConfigError, WorkerControlPlane};
use crate::identity::{
    CredentialStore, IdentityBootstrap, IdentityError, IdentityRecordStore, IdentitySource,
    WorkerIdentity, worker_credential_path, worker_identity_path,
};
use crate::managed::{
    LocalModelResolver, ManagedCoordinator, ManagedOutcome, ManagedPortAllocator, ManagedResult,
    ManagedRuntimeError, ManagedRuntimeOutcome, ManagedRuntimeRegistry, ManagedRuntimeResult,
    ManagedTarget, ResolveError, run_production_managed_runtime,
};
use crate::metrics::{MetricIdentity, MetricsState, router as metrics_router};
use crate::readiness::{ReadinessGate, WorkerReadiness};
use crate::resources::{
    CollectedResources, NvmlGpuProbe, ResourceCollector, ResourceError, SysinfoProbe,
};
use crate::signal::{ShutdownSignal, wait_for_shutdown};
use crate::supervisor::{SupervisedTaskExit, WorkerSupervisor};
use crate::worker_api::{WorkerApiBuildError, WorkerApiState, router as worker_api_router};

const PRODUCTION_NOT_READY_MESSAGE: &str = "Rust Worker initialization is incomplete";
const UNASSIGNED_WORKER_UUID: &str = "00000000-0000-4000-8000-000000000000";
const RESOURCE_REFRESH_INTERVAL: Duration = Duration::from_secs(10);
const WORKER_SHUTDOWN_GRACE: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
struct WorkerProfile {
    server_url: url::Url,
    worker_name: String,
    worker_hostname: String,
    worker_ip: IpAddr,
    worker_ifname: String,
    worker_port: u16,
    metrics_port: u16,
    data_dir: PathBuf,
}

impl WorkerProfile {
    fn from_config(config: &WorkerConfig) -> Result<Self, WorkerRunError> {
        if config.model_roots.is_empty() {
            return Err(WorkerRunError::MissingConfiguration("model_roots"));
        }
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
            metrics_port: config.metrics_port,
            data_dir: config.data_dir.clone(),
        })
    }

    fn worker_address(&self) -> SocketAddr {
        SocketAddr::new(self.worker_ip, self.worker_port)
    }

    fn metrics_address(&self) -> SocketAddr {
        SocketAddr::new(self.worker_ip, self.metrics_port)
    }

    fn registration_request(&self, resources: &CollectedResources) -> WorkerCreate {
        WorkerCreate {
            advertise_address: None,
            cluster_id: None,
            external_id: None,
            hostname: self.worker_hostname.clone(),
            ifname: self.worker_ifname.clone(),
            ip: self.worker_ip.to_string(),
            labels: BTreeMap::from([("implementation".to_owned(), "rust".to_owned())]),
            machine_id: resources.machine_id.clone(),
            maintenance: None,
            metrics_port: Some(self.metrics_port),
            name: self.worker_name.clone(),
            owner_principal_id: None,
            port: self.worker_port,
            proxy_mode: ProxyMode::Worker,
            state_message: Some(PRODUCTION_NOT_READY_MESSAGE.to_owned()),
            status: Some(resources.status.clone()),
            system_reserved: Some(SystemReserved::default()),
            worker_uuid: UNASSIGNED_WORKER_UUID.to_owned(),
            worker_version: SERVER_BASELINE_VERSION.to_owned(),
        }
    }

    fn status_report(
        &self,
        worker_uuid: String,
        resources: &CollectedResources,
        ready: bool,
    ) -> WorkerStatusReport {
        WorkerStatusReport {
            advertise_address: None,
            gateway_endpoint: None,
            hostname: self.worker_hostname.clone(),
            ifname: self.worker_ifname.clone(),
            ip: self.worker_ip.to_string(),
            machine_id: resources.machine_id.clone(),
            metrics_port: Some(self.metrics_port),
            port: self.worker_port,
            proxy_mode: ProxyMode::Worker,
            state_message: (!ready).then(|| PRODUCTION_NOT_READY_MESSAGE.to_owned()),
            status: Some(resources.status.clone()),
            system_reserved: Some(SystemReserved::default()),
            worker_uuid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerEvent {
    Listening {
        worker_address: SocketAddr,
        metrics_address: SocketAddr,
    },
    IdentityEstablished {
        worker_id: i64,
        worker_name: String,
        identity_source: &'static str,
    },
    Running {
        gpu_count: usize,
    },
    FileReconciled(ManagedResult),
    RuntimeReconciled(ManagedRuntimeResult),
    Stopping {
        signal: &'static str,
    },
}

impl fmt::Display for WorkerEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Listening {
                worker_address,
                metrics_address,
            } => write!(
                formatter,
                "state=listening worker_api={worker_address} metrics={metrics_address}"
            ),
            Self::IdentityEstablished {
                worker_id,
                worker_name,
                identity_source,
            } => write!(
                formatter,
                "state=identity_established worker_id={worker_id} worker_name={worker_name:?} identity_source={identity_source}"
            ),
            Self::Running { gpu_count } => {
                write!(formatter, "state=running gpu_devices={gpu_count}")
            }
            Self::FileReconciled(result) => render_managed_result(formatter, result),
            Self::RuntimeReconciled(result) => render_managed_runtime_result(formatter, result),
            Self::Stopping { signal } => write!(formatter, "state=stopping signal={signal}"),
        }
    }
}

fn render_managed_result(
    formatter: &mut fmt::Formatter<'_>,
    result: &ManagedResult,
) -> fmt::Result {
    let (resource, id) = match result.target {
        ManagedTarget::ModelInstance(id) => ("model_instance", id),
        ManagedTarget::ModelFile(id) => ("model_file", id),
    };
    write!(
        formatter,
        "component=managed_file resource={resource} id={id} observed_state={:?}",
        result.observed_state.as_deref().unwrap_or("unknown")
    )?;
    match &result.outcome {
        ManagedOutcome::InstanceInitialized { port, reused } => {
            write!(
                formatter,
                " outcome=initialized port={port} reused={reused}"
            )
        }
        ManagedOutcome::InstanceErrored { message } => {
            write!(formatter, " outcome=error message={message:?}")
        }
        ManagedOutcome::ModelFileReady { path_count, size } => write!(
            formatter,
            " outcome=ready path_count={path_count} size_bytes={size}"
        ),
        ManagedOutcome::ModelFileErrored { message } => {
            write!(formatter, " outcome=error message={message:?}")
        }
        ManagedOutcome::Noop { reason } => {
            write!(formatter, " outcome=noop reason={reason:?}")
        }
        ManagedOutcome::Failed { message } => {
            write!(formatter, " outcome=failed message={message:?}")
        }
    }
}

fn render_managed_runtime_result(
    formatter: &mut fmt::Formatter<'_>,
    result: &ManagedRuntimeResult,
) -> fmt::Result {
    write!(
        formatter,
        "component=managed_runtime resource=model_instance id={} ",
        result.instance_id
    )?;
    match &result.outcome {
        ManagedRuntimeOutcome::Starting {
            container_id,
            created,
            adopted,
        } => write!(
            formatter,
            "outcome=starting container_id={container_id:?} created={created} adopted={adopted}"
        ),
        ManagedRuntimeOutcome::Running {
            container_id,
            health_ready_at_unix_nanos,
        } => {
            write!(
                formatter,
                "outcome=running container_id={container_id:?} health_ready_at_unix_nanos={health_ready_at_unix_nanos}"
            )
        }
        ManagedRuntimeOutcome::Errored { message } => {
            write!(formatter, "outcome=error message={message:?}")
        }
        ManagedRuntimeOutcome::RestartPending {
            delay_secs,
            remaining_secs,
        } => write!(
            formatter,
            "outcome=restart_pending delay_secs={delay_secs} remaining_secs={remaining_secs}"
        ),
        ManagedRuntimeOutcome::RestartScheduled { restart_count } => write!(
            formatter,
            "outcome=restart_scheduled restart_count={restart_count}"
        ),
        ManagedRuntimeOutcome::Deleted { container_removed } => write!(
            formatter,
            "outcome=deleted container_removed={container_removed}"
        ),
        ManagedRuntimeOutcome::Noop { reason } => {
            write!(formatter, "outcome=noop reason={reason:?}")
        }
        ManagedRuntimeOutcome::Failed { message } => {
            write!(formatter, "outcome=failed message={message:?}")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkerSummary {
    pub server_version: String,
    pub server_git_commit: String,
    pub worker_id: i64,
    pub worker_name: String,
    pub worker_uuid: String,
    pub identity_source: String,
    pub worker_address: String,
    pub metrics_address: String,
    pub gpu_count: usize,
    pub shutdown_signal: String,
    pub desired_models: usize,
    pub desired_model_instances: usize,
    pub desired_model_files: usize,
    pub desired_revision: u64,
    pub file_reconciliations: u64,
    pub initialized_instances: u64,
    pub errored_instances: u64,
    pub ready_model_files: u64,
    pub errored_model_files: u64,
    pub file_noops: u64,
    pub file_failures: u64,
    pub runtime_reconciliations: u64,
    pub runtime_starting: u64,
    pub runtime_running: u64,
    pub runtime_errored: u64,
    pub runtime_deleted: u64,
    pub runtime_noops: u64,
    pub runtime_failures: u64,
    pub heartbeat_ok: bool,
    pub status_ok: bool,
    pub full_sync_ok: bool,
    pub watch_connected: bool,
    pub reconnect_attempts: u64,
}

#[derive(Debug, Error)]
pub enum WorkerRunError {
    #[error("Worker requires configuration field {0}")]
    MissingConfiguration(&'static str),
    #[error("failed to bind {service} endpoint {address}: {source}")]
    Bind {
        service: &'static str,
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("{0} endpoint stopped unexpectedly")]
    EndpointStopped(&'static str),
    #[error("{0} endpoint failed: {1}")]
    EndpointServe(&'static str, #[source] io::Error),
    #[error("Worker control plane stopped unexpectedly")]
    ControlPlaneStopped,
    #[error("Managed coordinator stopped unexpectedly")]
    ManagedCoordinatorStopped,
    #[error("Managed runtime coordinator stopped unexpectedly")]
    ManagedRuntimeCoordinatorStopped,
    #[error("Worker safety invariant failed: {0}")]
    SafetyInvariant(&'static str),
    #[error("failed to wait for shutdown signal: {0}")]
    ShutdownSignal(#[source] io::Error),
    #[error("Worker tasks did not stop within {0:?} after shutdown")]
    ShutdownTimeout(Duration),
    #[error(transparent)]
    Resource(#[from] ResourceError),
    #[error(transparent)]
    WorkerApi(#[from] WorkerApiBuildError),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    ControlConfiguration(#[from] ControlLoopConfigError),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    ManagedRuntime(#[from] ManagedRuntimeError),
}

#[derive(Debug, Default)]
struct ManagedStats {
    reconciliations: u64,
    initialized_instances: u64,
    errored_instances: u64,
    ready_model_files: u64,
    errored_model_files: u64,
    noops: u64,
    failures: u64,
}

#[derive(Debug, Default)]
struct ManagedRuntimeStats {
    reconciliations: u64,
    starting: u64,
    running: u64,
    errored: u64,
    deleted: u64,
    noops: u64,
    failures: u64,
}

impl ManagedRuntimeStats {
    fn record(&mut self, outcome: &ManagedRuntimeOutcome) {
        self.reconciliations = self.reconciliations.saturating_add(1);
        match outcome {
            ManagedRuntimeOutcome::Starting { .. } => {
                self.starting = self.starting.saturating_add(1);
            }
            ManagedRuntimeOutcome::Running { .. } => {
                self.running = self.running.saturating_add(1);
            }
            ManagedRuntimeOutcome::Errored { .. } => {
                self.errored = self.errored.saturating_add(1);
            }
            ManagedRuntimeOutcome::RestartPending { .. } => {
                self.noops = self.noops.saturating_add(1);
            }
            ManagedRuntimeOutcome::RestartScheduled { .. } => {
                self.starting = self.starting.saturating_add(1);
            }
            ManagedRuntimeOutcome::Deleted { .. } => {
                self.deleted = self.deleted.saturating_add(1);
            }
            ManagedRuntimeOutcome::Noop { .. } => {
                self.noops = self.noops.saturating_add(1);
            }
            ManagedRuntimeOutcome::Failed { .. } => {
                self.failures = self.failures.saturating_add(1);
            }
        }
    }
}

impl ManagedStats {
    fn record(&mut self, outcome: &ManagedOutcome) {
        self.reconciliations = self.reconciliations.saturating_add(1);
        match outcome {
            ManagedOutcome::InstanceInitialized { .. } => {
                self.initialized_instances = self.initialized_instances.saturating_add(1);
            }
            ManagedOutcome::InstanceErrored { .. } => {
                self.errored_instances = self.errored_instances.saturating_add(1);
            }
            ManagedOutcome::ModelFileReady { .. } => {
                self.ready_model_files = self.ready_model_files.saturating_add(1);
            }
            ManagedOutcome::ModelFileErrored { .. } => {
                self.errored_model_files = self.errored_model_files.saturating_add(1);
            }
            ManagedOutcome::Noop { .. } => {
                self.noops = self.noops.saturating_add(1);
            }
            ManagedOutcome::Failed { .. } => {
                self.failures = self.failures.saturating_add(1);
            }
        }
    }
}

enum WorkerTermination {
    Signal(ShutdownSignal),
    Failure(WorkerRunError),
}

/// Runs the complete Worker through the unified production lifecycle.
pub async fn run_worker<F>(
    config: &WorkerConfig,
    observe: F,
) -> Result<WorkerSummary, WorkerRunError>
where
    F: FnMut(&WorkerEvent),
{
    run_worker_inner(config, Arc::new(WorkerReadiness::default()), observe).await
}

async fn run_worker_inner<F>(
    config: &WorkerConfig,
    readiness: Arc<WorkerReadiness>,
    mut observe: F,
) -> Result<WorkerSummary, WorkerRunError>
where
    F: FnMut(&WorkerEvent),
{
    let profile = WorkerProfile::from_config(config)?;
    let token_file = config
        .token_file
        .as_ref()
        .ok_or(WorkerRunError::MissingConfiguration("token_file"))?;
    let registration_token = CredentialStore::new(token_file).load()?;
    // Canonicalizing roots before network setup keeps unsafe model paths fail-closed.
    let resolver = LocalModelResolver::new(config.model_roots.clone())?;
    let mut collector = ResourceCollector::new(SysinfoProbe::new(), NvmlGpuProbe::new()?);
    let initial_resources = collector.collect()?;
    verify_gpu_inventory(&initial_resources)?;
    verify_model_root_mounts(config, &initial_resources)?;
    set_readiness(&readiness, ReadinessGate::Resources, true);

    // Both listeners are reserved before contacting Server so a partial bind
    // cannot publish an unreachable Worker status.
    let worker_listener = TcpListener::bind(profile.worker_address())
        .await
        .map_err(|source| WorkerRunError::Bind {
            service: "Worker API",
            address: profile.worker_address(),
            source,
        })?;
    let metrics_listener =
        TcpListener::bind(profile.metrics_address())
            .await
            .map_err(|source| WorkerRunError::Bind {
                service: "metrics",
                address: profile.metrics_address(),
                source,
            })?;
    observe(&WorkerEvent::Listening {
        worker_address: worker_listener
            .local_addr()
            .map_err(|source| WorkerRunError::Bind {
                service: "Worker API",
                address: profile.worker_address(),
                source,
            })?,
        metrics_address: metrics_listener
            .local_addr()
            .map_err(|source| WorkerRunError::Bind {
                service: "metrics",
                address: profile.metrics_address(),
                source,
            })?,
    });
    set_readiness(&readiness, ReadinessGate::WorkerApi, true);
    set_readiness(&readiness, ReadinessGate::Metrics, true);

    let transport = ReqwestTransport::new()?;
    let client = ServerClient::new(profile.server_url.clone(), transport.clone())?;
    let managed_client = ServerClient::new(profile.server_url.clone(), transport.clone())?;
    let runtime_client = ServerClient::new(profile.server_url.clone(), transport)?;
    let bootstrap = IdentityBootstrap::new(
        &client,
        CredentialStore::new(worker_credential_path(&profile.data_dir)),
        IdentityRecordStore::new(worker_identity_path(&profile.data_dir)),
    );
    let identity = bootstrap
        .establish(
            Some(&registration_token),
            &profile.registration_request(&initial_resources),
        )
        .await?;
    let current_worker = client.me(&identity.token).await?;
    verify_production_worker(&current_worker, &identity)?;
    let identity_source = identity_source_name(&identity.source);
    observe(&WorkerEvent::IdentityEstablished {
        worker_id: identity.id,
        worker_name: identity.name.clone(),
        identity_source,
    });
    set_readiness(&readiness, ReadinessGate::Identity, true);

    let runtime_registry = Arc::new(ManagedRuntimeRegistry::default());
    let api_state = WorkerApiState::new_worker(
        config,
        identity.token.clone(),
        registration_token,
        Arc::clone(&runtime_registry),
        Arc::clone(&readiness),
    )?;
    let initial_status =
        profile.status_report(identity.worker_uuid.clone(), &initial_resources, false);
    // Publish the scheduling barrier before full sync can expose desired work.
    client
        .report_status(&identity.token, &initial_status)
        .await?;
    let (resources_tx, resources_rx) = watch::channel(initial_resources.clone());
    let (status_tx, status_rx) = watch::channel(initial_status);
    let mut supervisor = WorkerSupervisor::new();
    let (control, control_handle) = WorkerControlPlane::new(
        client,
        identity.token.clone(),
        identity.id,
        ControlLoopConfig::default(),
    )?;
    let desired_summary = control_handle.desired_state();
    let desired_managed = control_handle.desired_state();
    let desired_runtime = control_handle.desired_state();
    let mut control_health = control_handle.health();
    let control_shutdown = supervisor.subscribe();
    supervisor.add("control_plane", async move {
        control.run(status_rx, control_shutdown).await;
        Ok::<(), WorkerRunError>(())
    });
    let api_shutdown = supervisor.subscribe();
    supervisor.add("worker_api", async move {
        axum::serve(worker_listener, worker_api_router(api_state))
            .with_graceful_shutdown(shutdown_requested(api_shutdown))
            .into_future()
            .await
            .map_err(|error| WorkerRunError::EndpointServe("Worker API", error))
    });
    let metrics_mode = "worker";
    let metric_identity = MetricIdentity {
        instance: profile.worker_ip.to_string(),
        worker_id: identity.id,
        worker_name: identity.name.clone(),
    };
    let metrics_control = control_health.clone();
    let metrics_readiness = Some(Arc::clone(&readiness));
    let metrics_shutdown = supervisor.subscribe();
    supervisor.add("metrics", async move {
        axum::serve(
            metrics_listener,
            metrics_router(MetricsState {
                mode: metrics_mode,
                identity: metric_identity,
                resources: resources_rx,
                control: metrics_control,
                readiness: metrics_readiness,
            }),
        )
        .with_graceful_shutdown(shutdown_requested(metrics_shutdown))
        .into_future()
        .await
        .map_err(|error| WorkerRunError::EndpointServe("metrics", error))
    });
    let coordinator = ManagedCoordinator::new(
        managed_client,
        identity.token.clone(),
        identity.id,
        profile.worker_ip,
        resolver,
        ManagedPortAllocator::production(),
    );
    let (managed_tx, mut managed_rx) = mpsc::unbounded_channel();
    let managed_shutdown = supervisor.subscribe();
    supervisor.add("managed_files", async move {
        coordinator
            .run(desired_managed, managed_shutdown, |result| {
                let _ = managed_tx.send(result);
            })
            .await;
        Ok(())
    });
    let (runtime_tx, mut runtime_rx) = mpsc::unbounded_channel();
    let runtime_shutdown = supervisor.subscribe();
    let runtime_config = config.clone();
    let runtime_token = identity.token.clone();
    let runtime_worker_uuid = identity.worker_uuid.clone();
    let runtime_worker_ip = profile.worker_ip;
    let runtime_worker_id = identity.id;
    let runtime_registry_for_task = Arc::clone(&runtime_registry);
    supervisor.add("managed_runtime", async move {
        run_production_managed_runtime(
            runtime_client,
            runtime_token,
            runtime_worker_id,
            runtime_worker_uuid,
            runtime_worker_ip,
            runtime_config,
            desired_runtime,
            runtime_shutdown,
            runtime_registry_for_task,
            |result| {
                let _ = runtime_tx.send(result);
            },
        )
        .await?;
        Ok(())
    });
    let mut runtime_available = runtime_registry.subscribe_available();
    let mut runtime_coordinated = runtime_registry.subscribe_coordinated();
    let shutdown_signal = wait_for_shutdown();
    tokio::pin!(shutdown_signal);
    let mut refresh = tokio::time::interval(RESOURCE_REFRESH_INTERVAL);
    refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    refresh.tick().await;
    let gpu_count = initial_resources
        .status
        .gpu_devices
        .as_ref()
        .map_or(0, Vec::len);
    observe(&WorkerEvent::Running { gpu_count });
    let mut managed_stats = ManagedStats::default();
    let mut runtime_stats = ManagedRuntimeStats::default();

    let termination = loop {
        tokio::select! {
            signal = &mut shutdown_signal => {
                break match signal {
                    Ok(signal) => WorkerTermination::Signal(signal),
                    Err(error) => WorkerTermination::Failure(
                        WorkerRunError::ShutdownSignal(error),
                    ),
                };
            }
            exit = supervisor.next_exit() => {
                break WorkerTermination::Failure(unexpected_task_exit(
                    exit.expect("Worker supervisor always owns critical tasks"),
                ));
            }
            Some(result) = managed_rx.recv() => {
                managed_stats.record(&result.outcome);
                observe(&WorkerEvent::FileReconciled(result));
            }
            Some(result) = runtime_rx.recv() => {
                runtime_stats.record(&result.outcome);
                observe(&WorkerEvent::RuntimeReconciled(result));
            }
            changed = control_health.changed() => {
                if changed.is_err() {
                    break WorkerTermination::Failure(
                        WorkerRunError::ControlPlaneStopped,
                    );
                }
                set_readiness(
                    &readiness,
                    ReadinessGate::FullSync,
                    control_health.borrow().full_sync_ok,
                );
                publish_status_report(
                    &profile,
                    &identity.worker_uuid,
                    &resources_tx,
                    &status_tx,
                    &readiness,
                );
            }
            changed = runtime_available.changed() => {
                if changed.is_err() {
                    break WorkerTermination::Failure(
                        WorkerRunError::ManagedRuntimeCoordinatorStopped,
                    );
                }
                set_readiness(
                    &readiness,
                    ReadinessGate::Docker,
                    *runtime_available.borrow(),
                );
                publish_status_report(
                    &profile,
                    &identity.worker_uuid,
                    &resources_tx,
                    &status_tx,
                    &readiness,
                );
            }
            changed = runtime_coordinated.changed() => {
                if changed.is_err() {
                    break WorkerTermination::Failure(
                        WorkerRunError::ManagedRuntimeCoordinatorStopped,
                    );
                }
                set_readiness(
                    &readiness,
                    ReadinessGate::Runtime,
                    *runtime_coordinated.borrow(),
                );
                publish_status_report(
                    &profile,
                    &identity.worker_uuid,
                    &resources_tx,
                    &status_tx,
                    &readiness,
                );
            }
            _ = refresh.tick() => {
                let resources = match collector
                    .collect()
                    .map_err(WorkerRunError::from)
                    .and_then(|resources| {
                        verify_gpu_inventory(&resources)?;
                        verify_model_root_mounts(config, &resources)?;
                        Ok(resources)
                    })
                {
                    Ok(resources) => resources,
                    Err(error) => {
                        break WorkerTermination::Failure(error);
                    }
                };
                set_readiness(&readiness, ReadinessGate::Resources, true);
                resources_tx.send_replace(resources.clone());
                status_tx.send_replace(profile.status_report(
                    identity.worker_uuid.clone(),
                    &resources,
                    readiness.is_ready(),
                ));
            }
        }
    };

    let signal = match termination {
        WorkerTermination::Signal(signal) => {
            observe(&WorkerEvent::Stopping {
                signal: shutdown_signal_name(signal),
            });
            Some(signal)
        }
        WorkerTermination::Failure(error) => {
            reset_readiness(&readiness);
            supervisor.request_shutdown();
            supervisor
                .shutdown(WORKER_SHUTDOWN_GRACE)
                .await
                .map_err(|_| WorkerRunError::ShutdownTimeout(WORKER_SHUTDOWN_GRACE))?;
            return Err(error);
        }
    };
    reset_readiness(&readiness);
    supervisor.request_shutdown();
    let exits = supervisor
        .shutdown(WORKER_SHUTDOWN_GRACE)
        .await
        .map_err(|_| WorkerRunError::ShutdownTimeout(WORKER_SHUTDOWN_GRACE))?;
    if let Some(error) = first_task_error(exits) {
        return Err(error);
    }
    let signal = signal.expect("successful Worker termination requires a signal");
    let health = control_health.borrow().clone();
    let desired = desired_summary.borrow().clone();
    Ok(WorkerSummary {
        server_version: SERVER_BASELINE_VERSION.to_owned(),
        server_git_commit: SERVER_BASELINE_GIT_COMMIT.to_owned(),
        worker_id: identity.id,
        worker_name: identity.name,
        worker_uuid: identity.worker_uuid,
        identity_source: identity_source.to_owned(),
        worker_address: profile.worker_address().to_string(),
        metrics_address: profile.metrics_address().to_string(),
        gpu_count,
        shutdown_signal: shutdown_signal_name(signal).to_owned(),
        desired_models: desired.models.len(),
        desired_model_instances: desired.model_instances.len(),
        desired_model_files: desired.model_files.len(),
        desired_revision: desired.revision,
        file_reconciliations: managed_stats.reconciliations,
        initialized_instances: managed_stats.initialized_instances,
        errored_instances: managed_stats.errored_instances,
        ready_model_files: managed_stats.ready_model_files,
        errored_model_files: managed_stats.errored_model_files,
        file_noops: managed_stats.noops,
        file_failures: managed_stats.failures,
        runtime_reconciliations: runtime_stats.reconciliations,
        runtime_starting: runtime_stats.starting,
        runtime_running: runtime_stats.running,
        runtime_errored: runtime_stats.errored,
        runtime_deleted: runtime_stats.deleted,
        runtime_noops: runtime_stats.noops,
        runtime_failures: runtime_stats.failures,
        heartbeat_ok: health.heartbeat_ok,
        status_ok: health.status_ok,
        full_sync_ok: health.full_sync_ok,
        watch_connected: health.watch_connected,
        reconnect_attempts: health.reconnect_attempts,
    })
}

async fn shutdown_requested(mut shutdown: watch::Receiver<bool>) {
    loop {
        if *shutdown.borrow() || shutdown.changed().await.is_err() {
            return;
        }
    }
}

fn set_readiness(readiness: &WorkerReadiness, gate: ReadinessGate, ready: bool) {
    readiness.set(gate, ready);
}

fn publish_status_report(
    profile: &WorkerProfile,
    worker_uuid: &str,
    resources: &watch::Sender<CollectedResources>,
    status: &watch::Sender<WorkerStatusReport>,
    readiness: &WorkerReadiness,
) {
    let report = profile.status_report(
        worker_uuid.to_owned(),
        &resources.borrow(),
        readiness.is_ready(),
    );
    status.send_if_modified(|current| {
        if *current == report {
            false
        } else {
            *current = report;
            true
        }
    });
}

fn reset_readiness(readiness: &WorkerReadiness) {
    readiness.reset();
}

fn unexpected_task_exit(exit: SupervisedTaskExit<WorkerRunError>) -> WorkerRunError {
    match exit.result {
        Err(error) => error,
        Ok(()) => match exit.name {
            "worker_api" => WorkerRunError::EndpointStopped("Worker API"),
            "metrics" => WorkerRunError::EndpointStopped("metrics"),
            "control_plane" => WorkerRunError::ControlPlaneStopped,
            "managed_files" => WorkerRunError::ManagedCoordinatorStopped,
            "managed_runtime" => WorkerRunError::ManagedRuntimeCoordinatorStopped,
            _ => WorkerRunError::SafetyInvariant("unknown critical Worker task stopped"),
        },
    }
}

fn first_task_error(exits: Vec<SupervisedTaskExit<WorkerRunError>>) -> Option<WorkerRunError> {
    exits.into_iter().find_map(|exit| exit.result.err())
}

fn verify_gpu_inventory(resources: &CollectedResources) -> Result<(), WorkerRunError> {
    if resources.machine_id.as_deref().is_none_or(str::is_empty) {
        return Err(WorkerRunError::SafetyInvariant(
            "resource snapshot must contain a machine ID",
        ));
    }
    let gpus = resources
        .status
        .gpu_devices
        .as_deref()
        .ok_or(WorkerRunError::SafetyInvariant(
            "resource snapshot must contain GPU devices",
        ))?;
    if gpus.is_empty() {
        return Err(WorkerRunError::SafetyInvariant(
            "resource snapshot must contain at least one GPU",
        ));
    }
    if gpus.iter().any(|gpu| {
        gpu.name.is_empty()
            || gpu.uuid.as_deref().is_none_or(str::is_empty)
            || gpu
                .memory
                .as_ref()
                .and_then(|memory| memory.total)
                .is_none()
            || gpu.compute_capability.as_deref().is_none_or(str::is_empty)
    }) {
        return Err(WorkerRunError::SafetyInvariant(
            "GPU identity fields must be complete",
        ));
    }
    Ok(())
}

fn verify_model_root_mounts(
    config: &WorkerConfig,
    resources: &CollectedResources,
) -> Result<(), WorkerRunError> {
    let filesystems = resources.status.filesystem.as_deref().unwrap_or_default();
    let all_roots_covered = config.model_roots.iter().all(|root| {
        filesystems.iter().any(|filesystem| {
            let mount = std::path::Path::new(&filesystem.mount_point);
            root.starts_with(mount)
        })
    });
    if !all_roots_covered {
        return Err(WorkerRunError::SafetyInvariant(
            "resource snapshot must cover every configured model root mount",
        ));
    }
    Ok(())
}

fn verify_production_worker(
    worker: &WorkerPublic,
    identity: &WorkerIdentity,
) -> Result<(), WorkerRunError> {
    if worker.me != Some(true) || worker.id != identity.id || worker.name != identity.name {
        return Err(WorkerRunError::SafetyInvariant(
            "Server current Worker differs from the established identity",
        ));
    }
    Ok(())
}

fn identity_source_name(source: &IdentitySource) -> &'static str {
    match source {
        IdentitySource::Persisted => "persisted",
        IdentitySource::Registered => "registered",
        IdentitySource::Recovered => "recovered",
    }
}

fn shutdown_signal_name(signal: ShutdownSignal) -> &'static str {
    match signal {
        ShutdownSignal::Interrupt => "interrupt",
        ShutdownSignal::Terminate => "terminate",
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use gpustack_client::SecretToken;
    use gpustack_client::types::{Paginated, WorkerResourceStatus};

    use super::*;

    fn config(directory: PathBuf) -> WorkerConfig {
        WorkerConfig {
            server_url: Some("http://127.0.0.1/".parse().expect("server URL")),
            worker_name: Some("rust-worker".to_owned()),
            worker_hostname: Some("contract-host".to_owned()),
            worker_ip: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            worker_ifname: Some("lo".to_owned()),
            worker_port: 20150,
            metrics_port: 20151,
            token_file: None,
            data_dir: directory.clone(),
            log_dir: directory.join("logs"),
            model_roots: vec![directory.join("models")],
            docker_socket: PathBuf::from("/var/run/docker.sock"),
        }
    }

    #[test]
    fn profile_reports_typed_resources_and_readiness_without_stage_labels() {
        let directory = tempfile::tempdir().expect("temp directory");
        let profile = WorkerProfile::from_config(&config(directory.path().to_path_buf()))
            .expect("Worker profile");
        let status: WorkerResourceStatus = serde_json::from_slice(include_bytes!(
            "../../../fixtures/server-v2.2.3/worker-status-resource.request.json"
        ))
        .map(|value: serde_json::Value| serde_json::from_value(value["status"].clone()))
        .expect("fixture envelope")
        .expect("fixture status");
        let resources = CollectedResources {
            machine_id: Some("contract-machine-id".to_owned()),
            status,
        };

        verify_gpu_inventory(&resources).expect("valid GPU fixture");
        let create = profile.registration_request(&resources);
        let initializing = profile.status_report("contract-uuid".to_owned(), &resources, false);
        let ready = profile.status_report("contract-uuid".to_owned(), &resources, true);

        assert_eq!(
            create.labels,
            BTreeMap::from([("implementation".to_owned(), "rust".to_owned())])
        );
        assert_eq!(create.maintenance, None);
        assert_eq!(create.metrics_port, Some(20151));
        assert_eq!(initializing.metrics_port, Some(20151));
        assert_eq!(initializing.status, Some(resources.status));
        assert_eq!(
            create.state_message.as_deref(),
            Some(PRODUCTION_NOT_READY_MESSAGE)
        );
        assert_eq!(
            initializing.state_message.as_deref(),
            Some(PRODUCTION_NOT_READY_MESSAGE)
        );
        assert_eq!(ready.state_message, None);
        assert_eq!(create.system_reserved, Some(SystemReserved::default()));
        assert_eq!(
            initializing.system_reserved,
            Some(SystemReserved::default())
        );
    }

    #[test]
    fn production_identity_accepts_server_authoritative_maintenance() {
        let mut worker = serde_json::from_slice::<Paginated<WorkerPublic>>(include_bytes!(
            "../../../fixtures/server-v2.2.3/worker-maintenance-me.response.json"
        ))
        .expect("maintenance Worker fixture")
        .items
        .remove(0);
        let identity = WorkerIdentity {
            id: worker.id,
            name: worker.name.clone(),
            worker_uuid: "contract-uuid".to_owned(),
            token: SecretToken::new("worker-secret").expect("valid token"),
            source: IdentitySource::Persisted,
        };

        verify_production_worker(&worker, &identity).expect("maintenance is Server-owned");
        worker.maintenance = None;
        verify_production_worker(&worker, &identity).expect("non-maintenance Worker");
        worker.name = "different-worker".to_owned();
        assert!(verify_production_worker(&worker, &identity).is_err());
    }

    #[test]
    fn managed_event_reports_starting_as_a_structured_noop() {
        let event = WorkerEvent::FileReconciled(ManagedResult {
            target: ManagedTarget::ModelInstance(301),
            observed_state: Some("starting".to_owned()),
            outcome: ManagedOutcome::Noop {
                reason: "ModelInstance state is not scheduled",
            },
        });

        let line = event.to_string();

        assert!(line.contains("resource=model_instance"));
        assert!(line.contains("id=301"));
        assert!(line.contains("observed_state=\"starting\""));
        assert!(line.contains("outcome=noop"));
        assert!(!line.contains("token"));
    }

    #[test]
    fn file_events_report_the_component_without_a_stage_mode() {
        let event = WorkerEvent::FileReconciled(ManagedResult {
            target: ManagedTarget::ModelInstance(301),
            observed_state: Some("starting".to_owned()),
            outcome: ManagedOutcome::Noop {
                reason: "ModelInstance state is not scheduled",
            },
        });

        let line = event.to_string();

        assert!(line.contains("component=managed_file"));
        assert!(!line.contains("mode="));
        assert!(!line.contains("stage="));
    }

    #[test]
    fn managed_runtime_running_event_carries_the_pre_writeback_health_time() {
        let event = WorkerEvent::RuntimeReconciled(ManagedRuntimeResult {
            instance_id: 301,
            outcome: ManagedRuntimeOutcome::Running {
                container_id: "container-1".to_owned(),
                health_ready_at_unix_nanos: 1_793_190_027_650_000_000,
            },
        });

        let line = event.to_string();

        assert!(line.contains("outcome=running"));
        assert!(line.contains("container_id=\"container-1\""));
        assert!(line.contains("health_ready_at_unix_nanos=1793190027650000000"));
    }

    #[test]
    fn managed_summary_counters_classify_every_outcome() {
        let outcomes = [
            ManagedOutcome::InstanceInitialized {
                port: 40_000,
                reused: false,
            },
            ManagedOutcome::InstanceErrored {
                message: "port unavailable".to_owned(),
            },
            ManagedOutcome::ModelFileReady {
                path_count: 1,
                size: 7,
            },
            ManagedOutcome::ModelFileErrored {
                message: "invalid model".to_owned(),
            },
            ManagedOutcome::Noop { reason: "starting" },
            ManagedOutcome::Failed {
                message: "request failed".to_owned(),
            },
        ];
        let mut stats = ManagedStats::default();

        for outcome in &outcomes {
            stats.record(outcome);
        }

        assert_eq!(stats.reconciliations, 6);
        assert_eq!(stats.initialized_instances, 1);
        assert_eq!(stats.errored_instances, 1);
        assert_eq!(stats.ready_model_files, 1);
        assert_eq!(stats.errored_model_files, 1);
        assert_eq!(stats.noops, 1);
        assert_eq!(stats.failures, 1);
    }

    #[test]
    fn a_single_complete_gpu_is_a_valid_worker_inventory() {
        let status: WorkerResourceStatus = serde_json::from_slice(include_bytes!(
            "../../../fixtures/server-v2.2.3/worker-status-resource.request.json"
        ))
        .map(|value: serde_json::Value| serde_json::from_value(value["status"].clone()))
        .expect("fixture envelope")
        .expect("fixture status");
        let mut resources = CollectedResources {
            machine_id: Some("contract-machine-id".to_owned()),
            status,
        };
        let gpus = resources.status.gpu_devices.as_mut().expect("GPU devices");
        gpus.truncate(1);

        verify_gpu_inventory(&resources).expect("one complete GPU is supported");

        resources
            .status
            .gpu_devices
            .as_mut()
            .expect("GPU devices")
            .clear();
        assert!(verify_gpu_inventory(&resources).is_err());
    }

    #[test]
    fn missing_model_roots_fails_before_any_runtime_or_network_work() {
        let directory = tempfile::tempdir().expect("temp directory");
        let mut config = config(directory.path().to_path_buf());
        config.model_roots.clear();

        assert!(matches!(
            WorkerProfile::from_config(&config),
            Err(WorkerRunError::MissingConfiguration("model_roots"))
        ));
    }

    #[test]
    fn identity_source_names_are_stable() {
        assert_eq!(
            identity_source_name(&IdentitySource::Persisted),
            "persisted"
        );
        assert_eq!(
            identity_source_name(&IdentitySource::Registered),
            "registered"
        );
        assert_eq!(
            identity_source_name(&IdentitySource::Recovered),
            "recovered"
        );
    }

    #[test]
    fn every_model_root_must_have_a_reported_mount() {
        let directory = tempfile::tempdir().expect("temp directory");
        let config = config(directory.path().to_path_buf());
        let status: WorkerResourceStatus = serde_json::from_slice(include_bytes!(
            "../../../fixtures/server-v2.2.3/worker-status-resource.request.json"
        ))
        .map(|value: serde_json::Value| serde_json::from_value(value["status"].clone()))
        .expect("fixture envelope")
        .expect("fixture status");
        let mut resources = CollectedResources {
            machine_id: Some("contract-machine-id".to_owned()),
            status,
        };

        assert!(verify_model_root_mounts(&config, &resources).is_err());
        resources.status.filesystem.as_mut().expect("filesystems")[0].mount_point = "/".to_owned();
        verify_model_root_mounts(&config, &resources).expect("root mount covers model path");
    }
}
