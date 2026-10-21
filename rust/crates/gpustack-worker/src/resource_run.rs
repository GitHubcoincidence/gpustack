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
use tokio::sync::watch;

use crate::config::WorkerConfig;
use crate::control::{ControlLoopConfig, ControlLoopConfigError, WorkerControlPlane};
use crate::identity::{
    CredentialStore, IdentityBootstrap, IdentityError, IdentityRecordStore, IdentitySource,
    worker_credential_path, worker_identity_path,
};
use crate::metrics::{MetricIdentity, MetricsState, router as metrics_router};
use crate::resources::{
    CollectedResources, NvmlGpuProbe, ResourceCollector, ResourceError, SysinfoProbe,
};
use crate::signal::{ShutdownSignal, wait_for_shutdown};
use crate::worker_api::{WorkerApiBuildError, WorkerApiState, router as worker_api_router};

pub const RESOURCE_API_ONLY_MODE: &str = "resource-api-only";
const MAINTENANCE_MESSAGE: &str = "Rust M4 resource and API validation; scheduling disabled";
const UNASSIGNED_WORKER_UUID: &str = "00000000-0000-4000-8000-000000000000";
const RESOURCE_REFRESH_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
struct ResourceApiProfile {
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
    fn from_config(config: &WorkerConfig) -> Result<Self, ResourceApiRunError> {
        if config.model_roots.is_empty() {
            return Err(ResourceApiRunError::MissingConfiguration("model_roots"));
        }
        Ok(Self {
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
                ("mode".to_owned(), RESOURCE_API_ONLY_MODE.to_owned()),
                ("stage".to_owned(), "m4".to_owned()),
            ]),
            machine_id: resources.machine_id.clone(),
            maintenance: Some(WorkerMaintenance {
                enabled: true,
                message: Some(MAINTENANCE_MESSAGE.to_owned()),
            }),
            metrics_port: Some(self.metrics_port),
            name: self.worker_name.clone(),
            owner_principal_id: None,
            port: self.worker_port,
            proxy_mode: ProxyMode::Worker,
            state_message: Some(MAINTENANCE_MESSAGE.to_owned()),
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
            state_message: Some(MAINTENANCE_MESSAGE.to_owned()),
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

#[derive(Debug, Error)]
pub enum ResourceApiRunError {
    #[error("resource-api-only mode requires configuration field {0}")]
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
    #[error("resource-api-only safety invariant failed: {0}")]
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
}

/// Runs the M4 Worker surface while keeping every scheduling/runtime path disabled.
pub async fn run_resource_api_only<F>(
    config: &WorkerConfig,
    mut observe: F,
) -> Result<ResourceApiSummary, ResourceApiRunError>
where
    F: FnMut(&ResourceApiEvent),
{
    let profile = ResourceApiProfile::from_config(config)?;
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
    observe(&ResourceApiEvent::Listening {
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
    let client = ServerClient::new(profile.server_url.clone(), transport)?;
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
            "M4 requires an already persisted per-Worker identity",
        ));
    }
    let current_worker = client.me(&identity.token).await?;
    verify_maintenance_worker(&current_worker)?;
    observe(&ResourceApiEvent::IdentityEstablished {
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
    let control_health = control_handle.health();
    let control_future = control.run(status_rx, shutdown_rx.clone());
    let api_future = axum::serve(worker_listener, worker_api_router(api_state))
        .with_graceful_shutdown(shutdown_requested(shutdown_rx.clone()))
        .into_future();
    let metrics_future = axum::serve(
        metrics_listener,
        metrics_router(MetricsState {
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
    tokio::pin!(control_future);
    tokio::pin!(api_future);
    tokio::pin!(metrics_future);
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
    observe(&ResourceApiEvent::Running { gpu_count });

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

    observe(&ResourceApiEvent::Stopping {
        signal: shutdown_signal_name(signal),
    });
    shutdown_tx.send_replace(true);
    let (api_result, metrics_result, ()) =
        tokio::join!(&mut api_future, &mut metrics_future, &mut control_future);
    api_result.map_err(|error| ResourceApiRunError::EndpointServe("Worker API", error))?;
    metrics_result.map_err(|error| ResourceApiRunError::EndpointServe("metrics", error))?;
    let health = control_health.borrow().clone();
    Ok(ResourceApiSummary {
        mode: RESOURCE_API_ONLY_MODE.to_owned(),
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
            "M4 validation requires at least two GPUs",
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
        let profile = ResourceApiProfile::from_config(&config(directory.path().to_path_buf()))
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
        assert!(create.labels.values().all(|value| value != "docker"));
    }

    #[test]
    fn missing_model_roots_fails_before_any_runtime_or_network_work() {
        let directory = tempfile::tempdir().expect("temp directory");
        let mut config = config(directory.path().to_path_buf());
        config.model_roots.clear();

        assert!(matches!(
            ResourceApiProfile::from_config(&config),
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
