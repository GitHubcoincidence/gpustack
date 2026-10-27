//! Resource-only Worker lifecycle with no workload or container execution.

use std::collections::BTreeMap;
use std::fmt;
use std::future::IntoFuture;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use gpustack_client::http::{ReqwestTransport, TransportError};
use gpustack_client::types::{
    ProxyMode, SERVER_BASELINE_GIT_COMMIT, SERVER_BASELINE_VERSION, WorkerCreate,
    WorkerMaintenance, WorkerPublic, WorkerStatusReport,
};
use gpustack_client::{ClientError, ServerClient};
use serde::Serialize;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};

use crate::config::WorkerConfig;
use crate::control::{ControlLoopConfig, ControlLoopConfigError, DesiredState, WorkerControlPlane};
use crate::identity::{
    CredentialStore, IdentityBootstrap, IdentityError, IdentityRecordStore, IdentitySource,
    worker_credential_path, worker_identity_path,
};
use crate::managed::{
    LocalModelResolver, ManagedCoordinator, ManagedOutcome, ManagedPortAllocator, ManagedResult,
    ManagedTarget, ResolveError,
};
use crate::metrics::{MetricIdentity, MetricsState, router as metrics_router};
use crate::resources::{
    CollectedResources, NvmlGpuProbe, ResourceCollector, ResourceError, SysinfoProbe,
};
use crate::signal::{ShutdownSignal, wait_for_shutdown};
use crate::worker_api::{WorkerApiBuildError, WorkerApiState, router as worker_api_router};

pub const RESOURCE_API_ONLY_MODE: &str = "resource-api-only";
pub const MANAGED_FILE_ONLY_MODE: &str = "managed-file-only";
const MAINTENANCE_MESSAGE: &str = "Rust M4 resource and API validation; scheduling disabled";
const MANAGED_MAINTENANCE_MESSAGE: &str =
    "Rust M6-A managed file preparation; runtime execution disabled";
const UNASSIGNED_WORKER_UUID: &str = "00000000-0000-4000-8000-000000000000";
const RESOURCE_REFRESH_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerSurfaceMode {
    ResourceApiOnly,
    ManagedFileOnly,
}

impl WorkerSurfaceMode {
    const fn name(self) -> &'static str {
        match self {
            Self::ResourceApiOnly => RESOURCE_API_ONLY_MODE,
            Self::ManagedFileOnly => MANAGED_FILE_ONLY_MODE,
        }
    }

    const fn stage(self) -> &'static str {
        match self {
            Self::ResourceApiOnly => "m4",
            Self::ManagedFileOnly => "m6-a",
        }
    }

    const fn metric_name(self) -> &'static str {
        match self {
            Self::ResourceApiOnly => "resource_api_only",
            Self::ManagedFileOnly => "managed_file_only",
        }
    }

    const fn maintenance_message(self) -> &'static str {
        match self {
            Self::ResourceApiOnly => MAINTENANCE_MESSAGE,
            Self::ManagedFileOnly => MANAGED_MAINTENANCE_MESSAGE,
        }
    }

    const fn status_message(self) -> Option<&'static str> {
        match self {
            Self::ResourceApiOnly => Some(MAINTENANCE_MESSAGE),
            // Server treats a non-empty message as an explicit NOT_READY reason after
            // operator-controlled maintenance is disabled.
            Self::ManagedFileOnly => None,
        }
    }

    const fn reconciles_managed_files(self) -> bool {
        matches!(self, Self::ManagedFileOnly)
    }
}

#[derive(Debug, Clone)]
struct ResourceApiProfile {
    mode: WorkerSurfaceMode,
    server_url: url::Url,
    worker_name: String,
    worker_hostname: String,
    worker_ip: IpAddr,
    worker_ifname: String,
    worker_port: u16,
    metrics_port: u16,
    data_dir: PathBuf,
}

impl ResourceApiProfile {
    fn from_config(
        config: &WorkerConfig,
        mode: WorkerSurfaceMode,
    ) -> Result<Self, ResourceApiRunError> {
        if config.model_roots.is_empty() {
            return Err(ResourceApiRunError::MissingConfiguration("model_roots"));
        }
        Ok(Self {
            mode,
            server_url: config
                .server_url
                .clone()
                .ok_or(ResourceApiRunError::MissingConfiguration("server_url"))?,
            worker_name: config
                .worker_name
                .clone()
                .ok_or(ResourceApiRunError::MissingConfiguration("worker_name"))?,
            worker_hostname: config
                .worker_hostname
                .clone()
                .ok_or(ResourceApiRunError::MissingConfiguration("worker_hostname"))?,
            worker_ip: config
                .worker_ip
                .ok_or(ResourceApiRunError::MissingConfiguration("worker_ip"))?,
            worker_ifname: config
                .worker_ifname
                .clone()
                .ok_or(ResourceApiRunError::MissingConfiguration("worker_ifname"))?,
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
            labels: BTreeMap::from([
                ("implementation".to_owned(), "rust".to_owned()),
                ("mode".to_owned(), self.mode.name().to_owned()),
                ("stage".to_owned(), self.mode.stage().to_owned()),
            ]),
            machine_id: resources.machine_id.clone(),
            maintenance: Some(WorkerMaintenance {
                enabled: true,
                message: Some(self.mode.maintenance_message().to_owned()),
            }),
            metrics_port: Some(self.metrics_port),
            name: self.worker_name.clone(),
            owner_principal_id: None,
            port: self.worker_port,
            proxy_mode: ProxyMode::Worker,
            state_message: Some(self.mode.maintenance_message().to_owned()),
            status: Some(resources.status.clone()),
            system_reserved: None,
            worker_uuid: UNASSIGNED_WORKER_UUID.to_owned(),
            worker_version: SERVER_BASELINE_VERSION.to_owned(),
        }
    }

    fn status_report(
        &self,
        worker_uuid: String,
        resources: &CollectedResources,
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
            state_message: self.mode.status_message().map(str::to_owned),
            status: Some(resources.status.clone()),
            system_reserved: None,
            worker_uuid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceApiEvent {
    Listening {
        worker_address: SocketAddr,
        metrics_address: SocketAddr,
    },
    IdentityEstablished {
        worker_id: i64,
        worker_name: String,
    },
    Running {
        gpu_count: usize,
    },
    Stopping {
        signal: &'static str,
    },
}

impl fmt::Display for ResourceApiEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Listening {
                worker_address,
                metrics_address,
            } => write!(
                formatter,
                "mode={RESOURCE_API_ONLY_MODE} worker_api={worker_address} metrics={metrics_address}"
            ),
            Self::IdentityEstablished {
                worker_id,
                worker_name,
            } => write!(
                formatter,
                "worker_id={worker_id} worker_name={worker_name} identity_source=persisted"
            ),
            Self::Running { gpu_count } => write!(
                formatter,
                "mode={RESOURCE_API_ONLY_MODE} state=running scheduling=disabled gpu_devices={gpu_count}"
            ),
            Self::Stopping { signal } => write!(formatter, "state=stopping signal={signal}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceApiSummary {
    pub mode: String,
    pub server_version: String,
    pub server_git_commit: String,
    pub worker_id: i64,
    pub worker_name: String,
    pub worker_uuid: String,
    pub worker_address: String,
    pub metrics_address: String,
    pub gpu_count: usize,
    pub shutdown_signal: String,
    pub heartbeat_ok: bool,
    pub status_ok: bool,
    pub full_sync_ok: bool,
    pub watch_connected: bool,
    pub reconnect_attempts: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedFileEvent {
    Listening {
        worker_address: SocketAddr,
        metrics_address: SocketAddr,
    },
    IdentityEstablished {
        worker_id: i64,
        worker_name: String,
    },
    Running {
        gpu_count: usize,
    },
    Reconciled(ManagedResult),
    Stopping {
        signal: &'static str,
    },
}

impl fmt::Display for ManagedFileEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Listening {
                worker_address,
                metrics_address,
            } => write!(
                formatter,
                "mode={MANAGED_FILE_ONLY_MODE} worker_api={worker_address} metrics={metrics_address}"
            ),
            Self::IdentityEstablished {
                worker_id,
                worker_name,
            } => write!(
                formatter,
                "mode={MANAGED_FILE_ONLY_MODE} worker_id={worker_id} worker_name={worker_name:?} identity_source=persisted"
            ),
            Self::Running { gpu_count } => write!(
                formatter,
                "mode={MANAGED_FILE_ONLY_MODE} state=running scheduling=maintenance-required runtime=disabled gpu_devices={gpu_count}"
            ),
            Self::Reconciled(result) => render_managed_result(formatter, result),
            Self::Stopping { signal } => {
                write!(
                    formatter,
                    "mode={MANAGED_FILE_ONLY_MODE} state=stopping signal={signal}"
                )
            }
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
        "mode={MANAGED_FILE_ONLY_MODE} resource={resource} id={id} observed_state={:?}",
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedFileSummary {
    pub mode: String,
    pub server_version: String,
    pub server_git_commit: String,
    pub worker_id: i64,
    pub worker_name: String,
    pub worker_uuid: String,
    pub worker_address: String,
    pub metrics_address: String,
    pub gpu_count: usize,
    pub shutdown_signal: String,
    pub desired_models: usize,
    pub desired_model_instances: usize,
    pub desired_model_files: usize,
    pub desired_revision: u64,
    pub reconciliations: u64,
    pub initialized_instances: u64,
    pub errored_instances: u64,
    pub ready_model_files: u64,
    pub errored_model_files: u64,
    pub noops: u64,
    pub failures: u64,
    pub heartbeat_ok: bool,
    pub status_ok: bool,
    pub full_sync_ok: bool,
    pub watch_connected: bool,
    pub reconnect_attempts: u64,
}

#[derive(Debug, Error)]
pub enum ResourceApiRunError {
    #[error("resource Worker mode requires configuration field {0}")]
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
    #[error("resource Worker safety invariant failed: {0}")]
    SafetyInvariant(&'static str),
    #[error("failed to wait for shutdown signal: {0}")]
    ShutdownSignal(#[source] io::Error),
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SurfaceEvent {
    Listening {
        worker_address: SocketAddr,
        metrics_address: SocketAddr,
    },
    IdentityEstablished {
        worker_id: i64,
        worker_name: String,
    },
    Running {
        gpu_count: usize,
    },
    Reconciled(ManagedResult),
    Stopping {
        signal: &'static str,
    },
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

struct SurfaceSummary {
    resource: ResourceApiSummary,
    desired: DesiredState,
    managed: ManagedStats,
}

/// Runs the M4 Worker surface while keeping every scheduling/runtime path disabled.
pub async fn run_resource_api_only<F>(
    config: &WorkerConfig,
    mut observe: F,
) -> Result<ResourceApiSummary, ResourceApiRunError>
where
    F: FnMut(&ResourceApiEvent),
{
    let summary = run_worker_surface(config, WorkerSurfaceMode::ResourceApiOnly, |event| {
        let event = match event {
            SurfaceEvent::Listening {
                worker_address,
                metrics_address,
            } => ResourceApiEvent::Listening {
                worker_address: *worker_address,
                metrics_address: *metrics_address,
            },
            SurfaceEvent::IdentityEstablished {
                worker_id,
                worker_name,
            } => ResourceApiEvent::IdentityEstablished {
                worker_id: *worker_id,
                worker_name: worker_name.clone(),
            },
            SurfaceEvent::Running { gpu_count } => ResourceApiEvent::Running {
                gpu_count: *gpu_count,
            },
            SurfaceEvent::Stopping { signal } => ResourceApiEvent::Stopping { signal },
            SurfaceEvent::Reconciled(_) => return,
        };
        observe(&event);
    })
    .await?;
    Ok(summary.resource)
}

/// Runs Managed reconciliation through the resource surface without a container runtime.
pub async fn run_managed_file_only<F>(
    config: &WorkerConfig,
    mut observe: F,
) -> Result<ManagedFileSummary, ResourceApiRunError>
where
    F: FnMut(&ManagedFileEvent),
{
    let summary = run_worker_surface(config, WorkerSurfaceMode::ManagedFileOnly, |event| {
        let event = match event {
            SurfaceEvent::Listening {
                worker_address,
                metrics_address,
            } => ManagedFileEvent::Listening {
                worker_address: *worker_address,
                metrics_address: *metrics_address,
            },
            SurfaceEvent::IdentityEstablished {
                worker_id,
                worker_name,
            } => ManagedFileEvent::IdentityEstablished {
                worker_id: *worker_id,
                worker_name: worker_name.clone(),
            },
            SurfaceEvent::Running { gpu_count } => ManagedFileEvent::Running {
                gpu_count: *gpu_count,
            },
            SurfaceEvent::Reconciled(result) => ManagedFileEvent::Reconciled(result.clone()),
            SurfaceEvent::Stopping { signal } => ManagedFileEvent::Stopping { signal },
        };
        observe(&event);
    })
    .await?;
    let resource = summary.resource;
    Ok(ManagedFileSummary {
        mode: resource.mode,
        server_version: resource.server_version,
        server_git_commit: resource.server_git_commit,
        worker_id: resource.worker_id,
        worker_name: resource.worker_name,
        worker_uuid: resource.worker_uuid,
        worker_address: resource.worker_address,
        metrics_address: resource.metrics_address,
        gpu_count: resource.gpu_count,
        shutdown_signal: resource.shutdown_signal,
        desired_models: summary.desired.models.len(),
        desired_model_instances: summary.desired.model_instances.len(),
        desired_model_files: summary.desired.model_files.len(),
        desired_revision: summary.desired.revision,
        reconciliations: summary.managed.reconciliations,
        initialized_instances: summary.managed.initialized_instances,
        errored_instances: summary.managed.errored_instances,
        ready_model_files: summary.managed.ready_model_files,
        errored_model_files: summary.managed.errored_model_files,
        noops: summary.managed.noops,
        failures: summary.managed.failures,
        heartbeat_ok: resource.heartbeat_ok,
        status_ok: resource.status_ok,
        full_sync_ok: resource.full_sync_ok,
        watch_connected: resource.watch_connected,
        reconnect_attempts: resource.reconnect_attempts,
    })
}

async fn run_worker_surface<F>(
    config: &WorkerConfig,
    mode: WorkerSurfaceMode,
    mut observe: F,
) -> Result<SurfaceSummary, ResourceApiRunError>
where
    F: FnMut(&SurfaceEvent),
{
    let profile = ResourceApiProfile::from_config(config, mode)?;
    // Canonicalizing roots before network setup keeps unsafe model paths fail-closed.
    let resolver = mode
        .reconciles_managed_files()
        .then(|| LocalModelResolver::new(config.model_roots.clone()))
        .transpose()?;
    let mut collector = ResourceCollector::new(SysinfoProbe::new(), NvmlGpuProbe::new()?);
    let initial_resources = collector.collect()?;
    verify_m4_gpu_inventory(&initial_resources)?;
    verify_model_root_mounts(config, &initial_resources)?;

    // Both listeners are reserved before contacting Server so a partial bind
    // cannot publish an unreachable Worker status.
    let worker_listener = TcpListener::bind(profile.worker_address())
        .await
        .map_err(|source| ResourceApiRunError::Bind {
            service: "Worker API",
            address: profile.worker_address(),
            source,
        })?;
    let metrics_listener =
        TcpListener::bind(profile.metrics_address())
            .await
            .map_err(|source| ResourceApiRunError::Bind {
                service: "metrics",
                address: profile.metrics_address(),
                source,
            })?;
    observe(&SurfaceEvent::Listening {
        worker_address: worker_listener.local_addr().map_err(|source| {
            ResourceApiRunError::Bind {
                service: "Worker API",
                address: profile.worker_address(),
                source,
            }
        })?,
        metrics_address: metrics_listener.local_addr().map_err(|source| {
            ResourceApiRunError::Bind {
                service: "metrics",
                address: profile.metrics_address(),
                source,
            }
        })?,
    });

    let transport = ReqwestTransport::new()?;
    let client = ServerClient::new(profile.server_url.clone(), transport.clone())?;
    let managed_client = mode
        .reconciles_managed_files()
        .then(|| ServerClient::new(profile.server_url.clone(), transport))
        .transpose()?;
    let bootstrap = IdentityBootstrap::new(
        &client,
        CredentialStore::new(worker_credential_path(&profile.data_dir)),
        IdentityRecordStore::new(worker_identity_path(&profile.data_dir)),
    );
    let identity = bootstrap
        .establish(None, &profile.registration_request(&initial_resources))
        .await?;
    if identity.source != IdentitySource::Persisted {
        return Err(ResourceApiRunError::SafetyInvariant(
            "restricted resource modes require an already persisted per-Worker identity",
        ));
    }
    let current_worker = client.me(&identity.token).await?;
    verify_maintenance_worker(&current_worker)?;
    observe(&SurfaceEvent::IdentityEstablished {
        worker_id: identity.id,
        worker_name: identity.name.clone(),
    });

    let api_state = WorkerApiState::new(config, identity.token.clone())?;
    let (resources_tx, resources_rx) = watch::channel(initial_resources.clone());
    let (status_tx, status_rx) =
        watch::channel(profile.status_report(identity.worker_uuid.clone(), &initial_resources));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (control, control_handle) = WorkerControlPlane::new(
        client,
        identity.token.clone(),
        identity.id,
        ControlLoopConfig::default(),
    )?;
    let desired_summary = control_handle.desired_state();
    let desired_managed = control_handle.desired_state();
    let control_health = control_handle.health();
    let control_future = control.run(status_rx, shutdown_rx.clone());
    let api_future = axum::serve(worker_listener, worker_api_router(api_state))
        .with_graceful_shutdown(shutdown_requested(shutdown_rx.clone()))
        .into_future();
    let metrics_future = axum::serve(
        metrics_listener,
        metrics_router(MetricsState {
            mode: profile.mode.metric_name(),
            identity: MetricIdentity {
                instance: profile.worker_ip.to_string(),
                worker_id: identity.id,
                worker_name: identity.name.clone(),
            },
            resources: resources_rx,
            control: control_health.clone(),
        }),
    )
    .with_graceful_shutdown(shutdown_requested(shutdown_rx))
    .into_future();
    let coordinator = managed_client.zip(resolver).map(|(client, resolver)| {
        ManagedCoordinator::new(
            client,
            identity.token.clone(),
            identity.id,
            profile.worker_ip,
            resolver,
            ManagedPortAllocator::production(),
        )
    });
    let (managed_tx, mut managed_rx) = mpsc::unbounded_channel();
    let managed_shutdown = shutdown_tx.subscribe();
    let managed_future = async move {
        if let Some(coordinator) = coordinator {
            coordinator
                .run(desired_managed, managed_shutdown, |result| {
                    let _ = managed_tx.send(result);
                })
                .await;
        } else {
            shutdown_requested(managed_shutdown).await;
        }
    };
    tokio::pin!(control_future);
    tokio::pin!(api_future);
    tokio::pin!(metrics_future);
    tokio::pin!(managed_future);
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
    observe(&SurfaceEvent::Running { gpu_count });
    let mut managed_stats = ManagedStats::default();

    let signal = loop {
        tokio::select! {
            signal = &mut shutdown_signal => {
                break signal.map_err(ResourceApiRunError::ShutdownSignal)?;
            }
            result = &mut api_future => {
                shutdown_tx.send_replace(true);
                return Err(match result {
                    Ok(()) => ResourceApiRunError::EndpointStopped("Worker API"),
                    Err(error) => ResourceApiRunError::EndpointServe("Worker API", error),
                });
            }
            result = &mut metrics_future => {
                shutdown_tx.send_replace(true);
                return Err(match result {
                    Ok(()) => ResourceApiRunError::EndpointStopped("metrics"),
                    Err(error) => ResourceApiRunError::EndpointServe("metrics", error),
                });
            }
            () = &mut control_future => {
                shutdown_tx.send_replace(true);
                return Err(ResourceApiRunError::ControlPlaneStopped);
            }
            () = &mut managed_future => {
                shutdown_tx.send_replace(true);
                return Err(ResourceApiRunError::ManagedCoordinatorStopped);
            }
            Some(result) = managed_rx.recv() => {
                managed_stats.record(&result.outcome);
                observe(&SurfaceEvent::Reconciled(result));
            }
            _ = refresh.tick() => {
                let resources = collector.collect()?;
                verify_m4_gpu_inventory(&resources)?;
                verify_model_root_mounts(config, &resources)?;
                resources_tx.send_replace(resources.clone());
                status_tx.send_replace(profile.status_report(
                    identity.worker_uuid.clone(),
                    &resources,
                ));
            }
        }
    };

    observe(&SurfaceEvent::Stopping {
        signal: shutdown_signal_name(signal),
    });
    shutdown_tx.send_replace(true);
    let (api_result, metrics_result, (), ()) = tokio::join!(
        &mut api_future,
        &mut metrics_future,
        &mut control_future,
        &mut managed_future
    );
    api_result.map_err(|error| ResourceApiRunError::EndpointServe("Worker API", error))?;
    metrics_result.map_err(|error| ResourceApiRunError::EndpointServe("metrics", error))?;
    let health = control_health.borrow().clone();
    let desired = desired_summary.borrow().clone();
    Ok(SurfaceSummary {
        resource: ResourceApiSummary {
            mode: profile.mode.name().to_owned(),
            server_version: SERVER_BASELINE_VERSION.to_owned(),
            server_git_commit: SERVER_BASELINE_GIT_COMMIT.to_owned(),
            worker_id: identity.id,
            worker_name: identity.name,
            worker_uuid: identity.worker_uuid,
            worker_address: profile.worker_address().to_string(),
            metrics_address: profile.metrics_address().to_string(),
            gpu_count,
            shutdown_signal: shutdown_signal_name(signal).to_owned(),
            heartbeat_ok: health.heartbeat_ok,
            status_ok: health.status_ok,
            full_sync_ok: health.full_sync_ok,
            watch_connected: health.watch_connected,
            reconnect_attempts: health.reconnect_attempts,
        },
        desired,
        managed: managed_stats,
    })
}

async fn shutdown_requested(mut shutdown: watch::Receiver<bool>) {
    loop {
        if *shutdown.borrow() || shutdown.changed().await.is_err() {
            return;
        }
    }
}

fn verify_m4_gpu_inventory(resources: &CollectedResources) -> Result<(), ResourceApiRunError> {
    if resources.machine_id.as_deref().is_none_or(str::is_empty) {
        return Err(ResourceApiRunError::SafetyInvariant(
            "resource snapshot must contain a machine ID",
        ));
    }
    let gpus =
        resources
            .status
            .gpu_devices
            .as_deref()
            .ok_or(ResourceApiRunError::SafetyInvariant(
                "resource snapshot must contain GPU devices",
            ))?;
    if gpus.len() < 2 {
        return Err(ResourceApiRunError::SafetyInvariant(
            "the frozen validation host requires at least two GPUs",
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
        return Err(ResourceApiRunError::SafetyInvariant(
            "GPU identity fields must be complete",
        ));
    }
    Ok(())
}

fn verify_model_root_mounts(
    config: &WorkerConfig,
    resources: &CollectedResources,
) -> Result<(), ResourceApiRunError> {
    let filesystems = resources.status.filesystem.as_deref().unwrap_or_default();
    let all_roots_covered = config.model_roots.iter().all(|root| {
        filesystems.iter().any(|filesystem| {
            let mount = std::path::Path::new(&filesystem.mount_point);
            root.starts_with(mount)
        })
    });
    if !all_roots_covered {
        return Err(ResourceApiRunError::SafetyInvariant(
            "resource snapshot must cover every configured model root mount",
        ));
    }
    Ok(())
}

fn verify_maintenance_worker(worker: &WorkerPublic) -> Result<(), ResourceApiRunError> {
    if !worker
        .maintenance
        .as_ref()
        .is_some_and(|maintenance| maintenance.enabled)
    {
        return Err(ResourceApiRunError::SafetyInvariant(
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
    use std::net::{IpAddr, Ipv4Addr};

    use gpustack_client::types::WorkerResourceStatus;

    use super::*;

    fn config(directory: PathBuf) -> WorkerConfig {
        WorkerConfig {
            server_url: Some("http://127.0.0.1/".parse().expect("server URL")),
            worker_name: Some("rust-m4-worker".to_owned()),
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
    fn profile_reports_typed_resources_and_never_enables_scheduling() {
        let directory = tempfile::tempdir().expect("temp directory");
        let profile = ResourceApiProfile::from_config(
            &config(directory.path().to_path_buf()),
            WorkerSurfaceMode::ResourceApiOnly,
        )
        .expect("resource profile");
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

        verify_m4_gpu_inventory(&resources).expect("valid dual GPU fixture");
        let create = profile.registration_request(&resources);
        let report = profile.status_report("contract-uuid".to_owned(), &resources);

        assert!(create.maintenance.is_some_and(|value| value.enabled));
        assert_eq!(create.metrics_port, Some(20151));
        assert_eq!(report.metrics_port, Some(20151));
        assert_eq!(report.status, Some(resources.status));
        assert_eq!(report.state_message.as_deref(), Some(MAINTENANCE_MESSAGE));
        assert!(create.labels.values().all(|value| value != "docker"));
    }

    #[test]
    fn managed_profile_identifies_the_mode_and_keeps_maintenance_enabled() {
        let directory = tempfile::tempdir().expect("temp directory");
        let profile = ResourceApiProfile::from_config(
            &config(directory.path().to_path_buf()),
            WorkerSurfaceMode::ManagedFileOnly,
        )
        .expect("managed profile");
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

        let create = profile.registration_request(&resources);
        let report = profile.status_report("contract-uuid".to_owned(), &resources);

        assert_eq!(
            create.labels.get("mode").map(String::as_str),
            Some(MANAGED_FILE_ONLY_MODE)
        );
        assert_eq!(create.labels.get("stage").map(String::as_str), Some("m6-a"));
        assert!(create.maintenance.is_some_and(|value| value.enabled));
        assert!(
            create
                .state_message
                .is_some_and(|message| message.contains("runtime execution disabled"))
        );
        assert_eq!(report.state_message, None);
    }

    #[test]
    fn managed_event_reports_starting_as_a_structured_noop() {
        let event = ManagedFileEvent::Reconciled(ManagedResult {
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
    fn missing_model_roots_fails_before_any_runtime_or_network_work() {
        let directory = tempfile::tempdir().expect("temp directory");
        let mut config = config(directory.path().to_path_buf());
        config.model_roots.clear();

        assert!(matches!(
            ResourceApiProfile::from_config(&config, WorkerSurfaceMode::ResourceApiOnly),
            Err(ResourceApiRunError::MissingConfiguration("model_roots"))
        ));
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
