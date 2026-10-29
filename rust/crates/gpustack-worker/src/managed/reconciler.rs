//! Server-authoritative reconciliation for Managed vLLM containers.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::future::Future;
use std::io::Write;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use gpustack_client::http::{ReqwestTransport, Transport};
use gpustack_client::types::{
    ModelInstancePublic, ModelInstanceState, ModelInstanceUpdate, ModelPublic,
};
use gpustack_client::{ClientError, SecretToken, ServerClient};
use gpustack_runtime::docker::{
    ContainerLogOptions, ContainerRuntime, ContainerState, DockerRuntime, RuntimeError,
    RuntimeLogStreamKind,
};
use nvml_wrapper::Nvml;
use thiserror::Error;
use tokio::sync::watch;

use crate::config::WorkerConfig;
use crate::control::DesiredState;
use crate::lab::ReqwestLabHttpClient;
use crate::resources::unix_seconds_to_rfc3339;
use crate::runtime_core::{
    OwnedContainer, RuntimeCoreError, VllmHealthProbe, VllmHealthState, discover_owned_container,
    observe_vllm_health,
};
use crate::workload::{LABEL_MODEL_INSTANCE_ID, WorkloadSpecV2};

use super::{
    FsManagedRuntimeStore, LocalModelResolver, MANAGED_MANIFEST_SCHEMA_VERSION, ManagedGpu,
    ManagedRegistryError, ManagedRuntimeManifest, ManagedRuntimePlanError, ManagedRuntimeRegistry,
    ManagedRuntimeStore, ManagedStoreError, ManagedWorkloadPlanner, ResolveError,
};

const PUBLIC_MESSAGE_LIMIT: usize = 512;
const RECONCILE_INTERVAL: Duration = Duration::from_secs(2);
const STOP_TIMEOUT_SECS: u64 = 30;
const RESTART_BACKOFF_BASE_SECS: u64 = 10;
const RESTART_BACKOFF_MAX_SECS: u64 = 300;

#[allow(clippy::too_many_arguments)]
pub async fn run_production_managed_runtime<F>(
    client: ServerClient<ReqwestTransport>,
    worker_token: SecretToken,
    worker_id: i64,
    worker_uuid: String,
    worker_ip: IpAddr,
    config: WorkerConfig,
    desired: watch::Receiver<DesiredState>,
    shutdown: watch::Receiver<bool>,
    registry: Arc<ManagedRuntimeRegistry>,
    observe: F,
) -> Result<(), ManagedRuntimeError>
where
    F: FnMut(ManagedRuntimeResult),
{
    let resolver = LocalModelResolver::new(config.model_roots.clone())?;
    let runtime = Arc::new(DockerRuntime::connect(
        &config.docker_socket,
        Duration::from_secs(30),
    )?);
    runtime.daemon_info().await?;
    let health = Arc::new(
        ReqwestLabHttpClient::new(Duration::from_secs(10))
            .map_err(|error| ManagedRuntimeError::Environment(error.to_string()))?,
    );
    let store = FsManagedRuntimeStore::open(&config.data_dir, &config.log_dir)?;
    let coordinator = ManagedRuntimeCoordinator::new(
        client,
        worker_token,
        worker_id,
        worker_uuid,
        worker_ip,
        resolver,
        current_container_user()?,
        runtime,
        health,
        NvmlManagedGpuProbe,
        store,
        SystemManagedClock,
        registry,
    );
    coordinator.run(desired, shutdown, observe).await;
    Ok(())
}

#[cfg(unix)]
fn current_container_user() -> Result<String, ManagedRuntimeError> {
    use rustix::process::{getegid, geteuid};

    Ok(format!("{}:{}", geteuid().as_raw(), getegid().as_raw()))
}

#[cfg(not(unix))]
fn current_container_user() -> Result<String, ManagedRuntimeError> {
    Err(ManagedRuntimeError::Environment(
        "Managed workloads require a Unix host identity".to_string(),
    ))
}

pub trait ManagedRuntimeApi: Send + Sync + 'static {
    fn get_model<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        model_id: i64,
    ) -> impl Future<Output = Result<ModelPublic, ClientError>> + Send + 'a;

    fn get_model_instance<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        instance_id: i64,
    ) -> impl Future<Output = Result<ModelInstancePublic, ClientError>> + Send + 'a;

    fn update_model_instance<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        worker_id: i64,
        instance_id: i64,
        update: &'a ModelInstanceUpdate,
    ) -> impl Future<Output = Result<ModelInstancePublic, ClientError>> + Send + 'a;
}

impl<T> ManagedRuntimeApi for ServerClient<T>
where
    T: Transport + 'static,
{
    fn get_model<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        model_id: i64,
    ) -> impl Future<Output = Result<ModelPublic, ClientError>> + Send + 'a {
        ServerClient::get_model(self, worker_token, model_id)
    }

    fn get_model_instance<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        instance_id: i64,
    ) -> impl Future<Output = Result<ModelInstancePublic, ClientError>> + Send + 'a {
        ServerClient::get_model_instance(self, worker_token, instance_id)
    }

    fn update_model_instance<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        worker_id: i64,
        instance_id: i64,
        update: &'a ModelInstanceUpdate,
    ) -> impl Future<Output = Result<ModelInstancePublic, ClientError>> + Send + 'a {
        ServerClient::update_assigned_model_instance(
            self,
            worker_token,
            worker_id,
            instance_id,
            update,
        )
    }
}

pub trait ManagedGpuProbe: Send + Sync + 'static {
    fn snapshot(&self) -> Result<Vec<ManagedGpu>, ManagedRuntimeError>;
}

#[derive(Debug, Default)]
pub struct NvmlManagedGpuProbe;

impl ManagedGpuProbe for NvmlManagedGpuProbe {
    fn snapshot(&self) -> Result<Vec<ManagedGpu>, ManagedRuntimeError> {
        let nvml = Nvml::init().map_err(|error| ManagedRuntimeError::Gpu(error.to_string()))?;
        let count = nvml
            .device_count()
            .map_err(|error| ManagedRuntimeError::Gpu(error.to_string()))?;
        let mut gpus = Vec::with_capacity(count as usize);
        for index in 0..count {
            let device = nvml
                .device_by_index(index)
                .map_err(|error| ManagedRuntimeError::Gpu(error.to_string()))?;
            let uuid = device
                .uuid()
                .map_err(|error| ManagedRuntimeError::Gpu(error.to_string()))?;
            let processes = device
                .running_compute_processes()
                .map_err(|error| ManagedRuntimeError::Gpu(error.to_string()))?;
            gpus.push(ManagedGpu {
                host_index: index,
                uuid,
                running_compute_processes: processes.len(),
            });
        }
        Ok(gpus)
    }
}

pub trait ManagedClock: Send + Sync + 'static {
    fn now_unix(&self) -> u64;

    fn now_unix_nanos(&self) -> u128 {
        u128::from(self.now_unix()) * 1_000_000_000
    }
}

#[derive(Debug, Default)]
pub struct SystemManagedClock;

impl ManagedClock for SystemManagedClock {
    fn now_unix(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn now_unix_nanos(&self) -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedRuntimeOutcome {
    Starting {
        container_id: String,
        created: bool,
        adopted: bool,
    },
    Running {
        container_id: String,
        health_ready_at_unix_nanos: u128,
    },
    Errored {
        message: String,
    },
    RestartPending {
        delay_secs: u64,
        remaining_secs: u64,
    },
    RestartScheduled {
        restart_count: u32,
    },
    Deleted {
        container_removed: bool,
    },
    Noop {
        reason: &'static str,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntimeResult {
    pub instance_id: i64,
    pub outcome: ManagedRuntimeOutcome,
}

struct RefreshedPlan {
    instance: ModelInstancePublic,
    spec: WorkloadSpecV2,
    planner: ManagedWorkloadPlanner,
    restart_on_error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RestartBackoff {
    consecutive_failures: u32,
    last_attempt_unix: u64,
}

enum RunningObservation {
    Healthy,
    NotReady,
    Failed(String),
}

pub struct ManagedRuntimeCoordinator<A, G, S, C> {
    api: Arc<A>,
    worker_token: SecretToken,
    worker_id: i64,
    worker_uuid: String,
    worker_ip: IpAddr,
    resolver: LocalModelResolver,
    container_user: String,
    runtime: Arc<dyn ContainerRuntime>,
    health: Arc<dyn VllmHealthProbe>,
    gpus: Arc<G>,
    store: Arc<S>,
    clock: Arc<C>,
    registry: Arc<ManagedRuntimeRegistry>,
    log_tasks: Mutex<BTreeMap<i64, tokio::task::JoinHandle<()>>>,
    restart_backoffs: Mutex<BTreeMap<i64, RestartBackoff>>,
}

impl<A, G, S, C> ManagedRuntimeCoordinator<A, G, S, C>
where
    A: ManagedRuntimeApi,
    G: ManagedGpuProbe,
    S: ManagedRuntimeStore,
    C: ManagedClock,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api: A,
        worker_token: SecretToken,
        worker_id: i64,
        worker_uuid: String,
        worker_ip: IpAddr,
        resolver: LocalModelResolver,
        container_user: String,
        runtime: Arc<dyn ContainerRuntime>,
        health: Arc<dyn VllmHealthProbe>,
        gpus: G,
        store: S,
        clock: C,
        registry: Arc<ManagedRuntimeRegistry>,
    ) -> Self {
        Self {
            api: Arc::new(api),
            worker_token,
            worker_id,
            worker_uuid,
            worker_ip,
            resolver,
            container_user,
            runtime,
            health,
            gpus: Arc::new(gpus),
            store: Arc::new(store),
            clock: Arc::new(clock),
            registry,
            log_tasks: Mutex::new(BTreeMap::new()),
            restart_backoffs: Mutex::new(BTreeMap::new()),
        }
    }

    pub async fn reconcile_snapshot(&self, desired: &DesiredState) -> Vec<ManagedRuntimeResult> {
        let mut results = Vec::new();
        for instance in desired
            .model_instances
            .values()
            .filter(|instance| instance.worker_id == Some(self.worker_id))
        {
            let outcome = match instance.state.0.as_str() {
                "starting" => self.reconcile_starting(instance.id).await,
                "running" => self.reconcile_running(instance.id).await,
                "error" => self.reconcile_error(instance.id).await,
                _ => continue,
            };
            results.push(ManagedRuntimeResult {
                instance_id: instance.id,
                outcome,
            });
        }

        let desired_ids = desired
            .model_instances
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        match self.store.list_manifests() {
            Ok(manifests) => {
                for manifest in manifests {
                    if !desired_ids.contains(&manifest.instance_id) {
                        results.push(ManagedRuntimeResult {
                            instance_id: manifest.instance_id,
                            outcome: self.reconcile_deleted(manifest).await,
                        });
                    }
                }
            }
            Err(error) => results.push(ManagedRuntimeResult {
                instance_id: 0,
                outcome: failed(error),
            }),
        }
        results
    }

    pub async fn run<F>(
        &self,
        mut desired: watch::Receiver<DesiredState>,
        mut shutdown: watch::Receiver<bool>,
        mut observe: F,
    ) where
        F: FnMut(ManagedRuntimeResult),
    {
        let mut interval = tokio::time::interval(RECONCILE_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            if *shutdown.borrow() {
                self.registry.set_ready(false);
                return;
            }
            let snapshot = desired.borrow().clone();
            // Revision zero is the local empty seed, not proof that Server desires no workloads.
            if snapshot.revision == 0 {
                self.registry.set_ready(false);
                tokio::select! {
                    _ = interval.tick() => {}
                    changed = desired.changed() => {
                        if changed.is_err() {
                            return;
                        }
                    }
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            return;
                        }
                    }
                }
                continue;
            }
            let reconciliation = self.reconcile_snapshot(&snapshot);
            tokio::pin!(reconciliation);
            let results = tokio::select! {
                results = &mut reconciliation => results,
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        self.registry.set_ready(false);
                        return;
                    }
                    continue;
                }
            };
            self.registry.set_ready(
                !results
                    .iter()
                    .any(|result| matches!(result.outcome, ManagedRuntimeOutcome::Failed { .. })),
            );
            for result in results {
                observe(result);
            }
            tokio::select! {
                _ = interval.tick() => {}
                changed = desired.changed() => {
                    if changed.is_err() {
                        self.registry.set_ready(false);
                        return;
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        self.registry.set_ready(false);
                        return;
                    }
                }
            }
        }
    }

    async fn reconcile_starting(&self, instance_id: i64) -> ManagedRuntimeOutcome {
        if let Err(error) = self.registry.remove(instance_id) {
            return failed(error);
        }
        match self.reconcile_starting_inner(instance_id).await {
            Ok(outcome) => outcome,
            Err(error) if error.preserves_authoritative_state() => failed(error),
            Err(error) => self.report_error(instance_id, &error.to_string()).await,
        }
    }

    async fn reconcile_starting_inner(
        &self,
        instance_id: i64,
    ) -> Result<ManagedRuntimeOutcome, ManagedRuntimeError> {
        let initial = self.refresh_plan(instance_id, false).await?;
        initial
            .planner
            .verify_local_image(self.runtime.as_ref(), &initial.spec)
            .await?;
        let discovered = initial
            .planner
            .discover(self.runtime.as_ref(), &initial.spec)
            .await?;
        match discovered {
            OwnedContainer::Missing => self.create_and_start(initial).await,
            OwnedContainer::Present(container) => {
                self.adopt_or_observe(initial, container.id, false).await
            }
        }
    }

    async fn reconcile_running(&self, instance_id: i64) -> ManagedRuntimeOutcome {
        match self.registry.route(instance_id) {
            Ok(Some(_)) => match self.observe_authoritative_running(instance_id).await {
                Ok(RunningObservation::Healthy) => {
                    if let Err(error) = self.reset_restart_backoff(instance_id) {
                        return failed(error);
                    }
                    return ManagedRuntimeOutcome::Noop {
                        reason: "RUNNING Managed workload is already adopted",
                    };
                }
                Ok(RunningObservation::NotReady) => {
                    return ManagedRuntimeOutcome::Noop {
                        reason: "RUNNING Managed workload has no confirmed terminal failure",
                    };
                }
                Ok(RunningObservation::Failed(message)) => {
                    return self.report_running_error(instance_id, &message).await;
                }
                Err(error) => return failed(error),
            },
            Ok(None) => {}
            Err(error) => return failed(error),
        }
        match self.reconcile_running_inner(instance_id).await {
            Ok(outcome) => outcome,
            Err(error) => failed(error),
        }
    }

    async fn observe_authoritative_running(
        &self,
        instance_id: i64,
    ) -> Result<RunningObservation, ManagedRuntimeError> {
        let plan = self.refresh_running_plan(instance_id).await?;
        let container = match plan
            .planner
            .discover(self.runtime.as_ref(), &plan.spec)
            .await?
        {
            OwnedContainer::Missing => {
                return Ok(RunningObservation::Failed(
                    "Server-RUNNING Managed workload has no owned container".to_string(),
                ));
            }
            OwnedContainer::Present(container) => container,
        };
        let Some(manifest) = self.store.load_manifest(instance_id)? else {
            return Err(ManagedRuntimeError::Unsafe(
                "adopted RUNNING Managed workload has no recovery manifest".to_string(),
            ));
        };
        if manifest.container_id != container.id
            || manifest.spec != plan.spec
            || manifest.restart_count != plan.instance.restart_count.unwrap_or(0)
        {
            return Err(ManagedRuntimeError::Unsafe(
                "RUNNING Managed recovery state conflicts with Docker or Server".to_string(),
            ));
        }
        let observation = observe_vllm_health(
            self.runtime.as_ref(),
            self.health.as_ref(),
            &plan.spec,
            &container.id,
        )
        .await?;
        match observation.state {
            VllmHealthState::Ready => Ok(RunningObservation::Healthy),
            VllmHealthState::Starting => Ok(RunningObservation::NotReady),
            VllmHealthState::Failed => Ok(RunningObservation::Failed(
                observation
                    .message
                    .unwrap_or_else(|| "Managed vLLM container failed".to_string()),
            )),
        }
    }

    async fn reconcile_error(&self, instance_id: i64) -> ManagedRuntimeOutcome {
        match self.reconcile_error_inner(instance_id).await {
            Ok(outcome) => outcome,
            Err(error) => failed(error),
        }
    }

    async fn reconcile_error_inner(
        &self,
        instance_id: i64,
    ) -> Result<ManagedRuntimeOutcome, ManagedRuntimeError> {
        let initial = self.refresh_error_plan(instance_id).await?;
        if !initial.restart_on_error {
            return Ok(ManagedRuntimeOutcome::Noop {
                reason: "restart_on_error is disabled",
            });
        }

        let now = self.clock.now_unix();
        if let Some(backoff) = self.restart_backoff(instance_id)? {
            let delay_secs = restart_delay_secs(backoff.consecutive_failures);
            let elapsed = now.saturating_sub(backoff.last_attempt_unix);
            if elapsed < delay_secs {
                return Ok(ManagedRuntimeOutcome::RestartPending {
                    delay_secs,
                    remaining_secs: delay_secs - elapsed,
                });
            }
        }

        initial
            .planner
            .verify_local_image(self.runtime.as_ref(), &initial.spec)
            .await?;
        let authorized = self.refresh_same_error_plan(&initial).await?;
        if !authorized.restart_on_error {
            return Ok(ManagedRuntimeOutcome::Noop {
                reason: "restart_on_error was disabled before restart",
            });
        }
        self.remove_failed_runtime(&authorized).await?;

        let latest = self.refresh_same_error_plan(&authorized).await?;
        if !latest.restart_on_error {
            return Ok(ManagedRuntimeOutcome::Noop {
                reason: "restart_on_error was disabled before Server writeback",
            });
        }
        let restart_count = latest
            .instance
            .restart_count
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| {
                ManagedRuntimeError::Unsafe("Managed restart count overflow".to_string())
            })?;
        let mut update = ModelInstanceUpdate::from(&latest.instance);
        update.restart_count = Some(restart_count);
        update.last_restart_time = Some(unix_seconds_to_rfc3339(now));
        update.state = ModelInstanceState("scheduled".to_string());
        update.state_message = None;
        self.api
            .update_model_instance(
                &self.worker_token,
                self.worker_id,
                latest.instance.id,
                &update,
            )
            .await?;
        self.record_restart_attempt(instance_id, now)?;
        Ok(ManagedRuntimeOutcome::RestartScheduled { restart_count })
    }

    async fn remove_failed_runtime(&self, plan: &RefreshedPlan) -> Result<(), ManagedRuntimeError> {
        self.registry.remove(plan.instance.id)?;
        self.abort_log_capture(plan.instance.id)?;
        let discovered = plan
            .planner
            .discover(self.runtime.as_ref(), &plan.spec)
            .await?;
        let OwnedContainer::Present(container) = discovered else {
            return Ok(());
        };
        if matches!(
            container.state,
            ContainerState::Running
                | ContainerState::Paused
                | ContainerState::Restarting
                | ContainerState::Stopping
        ) {
            let refreshed = self.refresh_same_error_plan(plan).await?;
            if !refreshed.restart_on_error {
                return Err(ManagedRuntimeError::Unsafe(
                    "restart policy changed before failed container stop".to_string(),
                ));
            }
            let current = refreshed
                .planner
                .discover(self.runtime.as_ref(), &refreshed.spec)
                .await?;
            let OwnedContainer::Present(current) = current else {
                return Err(ManagedRuntimeError::Unsafe(
                    "Managed container disappeared before restart stop".to_string(),
                ));
            };
            if current.id != container.id {
                return Err(ManagedRuntimeError::Unsafe(
                    "Managed container identity changed before restart stop".to_string(),
                ));
            }
            self.runtime
                .stop_container(&current.id, STOP_TIMEOUT_SECS)
                .await?;
        }

        let refreshed = self.refresh_same_error_plan(plan).await?;
        if !refreshed.restart_on_error {
            return Err(ManagedRuntimeError::Unsafe(
                "restart policy changed before failed container removal".to_string(),
            ));
        }
        let current = refreshed
            .planner
            .discover(self.runtime.as_ref(), &refreshed.spec)
            .await?;
        let OwnedContainer::Present(current) = current else {
            return Err(ManagedRuntimeError::Unsafe(
                "Managed container disappeared before restart removal".to_string(),
            ));
        };
        if current.id != container.id || current.state == ContainerState::Running {
            return Err(ManagedRuntimeError::Unsafe(
                "Managed failed container is not safely removable".to_string(),
            ));
        }
        self.runtime.remove_container(&current.id, false).await?;
        Ok(())
    }

    async fn reconcile_running_inner(
        &self,
        instance_id: i64,
    ) -> Result<ManagedRuntimeOutcome, ManagedRuntimeError> {
        let plan = self.refresh_running_plan(instance_id).await?;
        plan.planner
            .verify_local_image(self.runtime.as_ref(), &plan.spec)
            .await?;
        match plan
            .planner
            .discover(self.runtime.as_ref(), &plan.spec)
            .await?
        {
            OwnedContainer::Missing => Ok(self
                .report_running_error(
                    instance_id,
                    "Server-RUNNING Managed workload has no owned container",
                )
                .await),
            OwnedContainer::Present(container) => self.adopt_running(plan, container.id).await,
        }
    }

    async fn adopt_running(
        &self,
        plan: RefreshedPlan,
        container_id: String,
    ) -> Result<ManagedRuntimeOutcome, ManagedRuntimeError> {
        let observation = observe_vllm_health(
            self.runtime.as_ref(),
            self.health.as_ref(),
            &plan.spec,
            &container_id,
        )
        .await?;
        if observation.state == VllmHealthState::Failed {
            let message = observation
                .message
                .unwrap_or_else(|| "Managed vLLM container failed".to_string());
            return Ok(self.report_running_error(plan.instance.id, &message).await);
        }
        if observation.container.state != ContainerState::Running
            || observation.state != VllmHealthState::Ready
        {
            return Err(ManagedRuntimeError::Unsafe(
                "Server-RUNNING Managed workload is not healthy enough for startup adoption"
                    .to_string(),
            ));
        }

        // Re-read both authorities after the health probe so a stale snapshot cannot publish a route.
        let refreshed = self.refresh_same_running_plan(&plan).await?;
        let current = refreshed
            .planner
            .discover(self.runtime.as_ref(), &refreshed.spec)
            .await?;
        let OwnedContainer::Present(current) = current else {
            return Err(ManagedRuntimeError::Unsafe(
                "Managed container disappeared before startup adoption".to_string(),
            ));
        };
        if current.id != container_id || current.state != ContainerState::Running {
            return Err(ManagedRuntimeError::Unsafe(
                "Managed container changed before startup adoption".to_string(),
            ));
        }

        let restart_count = refreshed.instance.restart_count.unwrap_or(0);
        let mut manifest = match self.store.load_manifest(refreshed.instance.id)? {
            Some(manifest) => {
                if manifest.container_id != container_id
                    || manifest.spec != refreshed.spec
                    || manifest.restart_count != restart_count
                {
                    return Err(ManagedRuntimeError::Unsafe(
                        "Managed manifest conflicts with the authoritative running workload"
                            .to_string(),
                    ));
                }
                manifest
            }
            None => ManagedRuntimeManifest {
                schema_version: MANAGED_MANIFEST_SCHEMA_VERSION,
                instance_id: refreshed.instance.id,
                worker_uuid: self.worker_uuid.clone(),
                spec: refreshed.spec.clone(),
                container_id: container_id.clone(),
                restart_count,
                started_at_unix: self.clock.now_unix(),
                last_observed_state: "adopted".to_string(),
            },
        };
        self.store.prepare_cache(refreshed.instance.id)?;
        manifest.last_observed_state = "ready".to_string();
        self.store.save_manifest(&manifest)?;
        self.ensure_log_capture(refreshed.instance.id, &container_id, restart_count)?;
        self.registry.publish(
            refreshed.instance.id,
            refreshed.spec.service_port,
            refreshed.spec.served_model_name.clone(),
        )?;
        self.reset_restart_backoff(refreshed.instance.id)?;
        Ok(ManagedRuntimeOutcome::Running {
            container_id,
            health_ready_at_unix_nanos: self.clock.now_unix_nanos(),
        })
    }

    async fn create_and_start(
        &self,
        initial: RefreshedPlan,
    ) -> Result<ManagedRuntimeOutcome, ManagedRuntimeError> {
        let refreshed = self.refresh_same_plan(&initial, true).await?;
        refreshed
            .planner
            .verify_local_image(self.runtime.as_ref(), &refreshed.spec)
            .await?;
        if !matches!(
            refreshed
                .planner
                .discover(self.runtime.as_ref(), &refreshed.spec)
                .await?,
            OwnedContainer::Missing
        ) {
            return Err(ManagedRuntimeError::Unsafe(
                "Managed container appeared before create authorization".to_string(),
            ));
        }

        self.store.prepare_cache(refreshed.instance.id)?;
        let created = self
            .runtime
            .create_container(&refreshed.spec.container_spec())
            .await?;
        let manifest = ManagedRuntimeManifest {
            schema_version: MANAGED_MANIFEST_SCHEMA_VERSION,
            instance_id: refreshed.instance.id,
            worker_uuid: self.worker_uuid.clone(),
            spec: refreshed.spec.clone(),
            container_id: created.id.clone(),
            restart_count: refreshed.instance.restart_count.unwrap_or(0),
            started_at_unix: self.clock.now_unix(),
            last_observed_state: "created".to_string(),
        };
        self.store.save_manifest(&manifest)?;
        if !created.warnings.is_empty() {
            return Err(ManagedRuntimeError::Unsafe(
                "Docker returned warnings while creating the Managed container".to_string(),
            ));
        }
        self.authorize_and_start(&refreshed, &created.id).await?;
        self.ensure_log_capture(
            refreshed.instance.id,
            &created.id,
            refreshed.instance.restart_count.unwrap_or(0),
        )?;
        Ok(ManagedRuntimeOutcome::Starting {
            container_id: created.id,
            created: true,
            adopted: false,
        })
    }

    async fn adopt_or_observe(
        &self,
        plan: RefreshedPlan,
        container_id: String,
        created: bool,
    ) -> Result<ManagedRuntimeOutcome, ManagedRuntimeError> {
        let mut manifest = match self.store.load_manifest(plan.instance.id)? {
            Some(manifest) => {
                if manifest.container_id != container_id
                    || manifest.spec != plan.spec
                    || manifest.restart_count != plan.instance.restart_count.unwrap_or(0)
                {
                    return Err(ManagedRuntimeError::Unsafe(
                        "Managed manifest conflicts with the authoritative workload".to_string(),
                    ));
                }
                manifest
            }
            None => ManagedRuntimeManifest {
                schema_version: MANAGED_MANIFEST_SCHEMA_VERSION,
                instance_id: plan.instance.id,
                worker_uuid: self.worker_uuid.clone(),
                spec: plan.spec.clone(),
                container_id: container_id.clone(),
                restart_count: plan.instance.restart_count.unwrap_or(0),
                started_at_unix: self.clock.now_unix(),
                last_observed_state: "adopted".to_string(),
            },
        };
        self.store.save_manifest(&manifest)?;

        let observation = observe_vllm_health(
            self.runtime.as_ref(),
            self.health.as_ref(),
            &plan.spec,
            &container_id,
        )
        .await?;
        if observation.container.state == ContainerState::Created {
            self.authorize_and_start(&plan, &container_id).await?;
            self.ensure_log_capture(
                plan.instance.id,
                &container_id,
                plan.instance.restart_count.unwrap_or(0),
            )?;
            manifest.last_observed_state = "starting".to_string();
            self.store.save_manifest(&manifest)?;
            return Ok(ManagedRuntimeOutcome::Starting {
                container_id,
                created,
                adopted: true,
            });
        }
        if observation.container.state == ContainerState::Running {
            self.ensure_log_capture(
                plan.instance.id,
                &container_id,
                plan.instance.restart_count.unwrap_or(0),
            )?;
        }
        manifest.last_observed_state = format!("{:?}", observation.state).to_ascii_lowercase();
        self.store.save_manifest(&manifest)?;
        match observation.state {
            VllmHealthState::Ready => self.report_running(&plan, &container_id).await,
            VllmHealthState::Failed => Err(ManagedRuntimeError::Unsafe(
                observation
                    .message
                    .unwrap_or_else(|| "Managed vLLM container failed".to_string()),
            )),
            VllmHealthState::Starting => {
                if self
                    .clock
                    .now_unix()
                    .saturating_sub(manifest.started_at_unix)
                    >= plan.spec.health.deadline_secs
                {
                    Err(ManagedRuntimeError::Unsafe(
                        "Managed vLLM readiness deadline expired".to_string(),
                    ))
                } else {
                    Ok(ManagedRuntimeOutcome::Starting {
                        container_id,
                        created,
                        adopted: !created,
                    })
                }
            }
        }
    }

    async fn authorize_and_start(
        &self,
        previous: &RefreshedPlan,
        container_id: &str,
    ) -> Result<(), ManagedRuntimeError> {
        let refreshed = self.refresh_same_plan(previous, true).await?;
        match refreshed
            .planner
            .discover(self.runtime.as_ref(), &refreshed.spec)
            .await?
        {
            OwnedContainer::Present(container) if container.id == container_id => {
                self.runtime.start_container(container_id).await?;
                Ok(())
            }
            _ => Err(ManagedRuntimeError::Unsafe(
                "Managed container changed before start authorization".to_string(),
            )),
        }
    }

    async fn report_running(
        &self,
        previous: &RefreshedPlan,
        container_id: &str,
    ) -> Result<ManagedRuntimeOutcome, ManagedRuntimeError> {
        let refreshed = self.refresh_same_plan(previous, false).await?;
        self.registry.publish(
            refreshed.instance.id,
            refreshed.spec.service_port,
            refreshed.spec.served_model_name.clone(),
        )?;
        let mut update = ModelInstanceUpdate::from(&refreshed.instance);
        update.state = ModelInstanceState("running".to_string());
        update.state_message = None;
        // Capture the passed health gate before the Server RUNNING writeback for auditable ordering.
        let health_ready_at_unix_nanos = self.clock.now_unix_nanos();
        if let Err(error) = self
            .api
            .update_model_instance(
                &self.worker_token,
                self.worker_id,
                refreshed.instance.id,
                &update,
            )
            .await
        {
            self.registry.remove(refreshed.instance.id)?;
            return Err(error.into());
        }
        self.reset_restart_backoff(refreshed.instance.id)?;
        Ok(ManagedRuntimeOutcome::Running {
            container_id: container_id.to_string(),
            health_ready_at_unix_nanos,
        })
    }

    async fn report_error(&self, instance_id: i64, message: &str) -> ManagedRuntimeOutcome {
        if let Err(error) = self.registry.remove(instance_id) {
            return failed(error);
        }
        let latest = match self
            .api
            .get_model_instance(&self.worker_token, instance_id)
            .await
        {
            Ok(instance) => instance,
            Err(error) => return failed(error),
        };
        if latest.worker_id != Some(self.worker_id) || latest.state.0 != "starting" {
            return ManagedRuntimeOutcome::Noop {
                reason: "ModelInstance changed before ERROR writeback",
            };
        }
        let message = public_message(message);
        let mut update = ModelInstanceUpdate::from(&latest);
        update.state = ModelInstanceState("error".to_string());
        update.state_message = Some(message.clone());
        match self
            .api
            .update_model_instance(&self.worker_token, self.worker_id, instance_id, &update)
            .await
        {
            Ok(_) => ManagedRuntimeOutcome::Errored { message },
            Err(error) => failed(error),
        }
    }

    async fn report_running_error(&self, instance_id: i64, message: &str) -> ManagedRuntimeOutcome {
        let latest = match self
            .api
            .get_model_instance(&self.worker_token, instance_id)
            .await
        {
            Ok(instance) => instance,
            Err(error) => return failed(error),
        };
        if latest.worker_id != Some(self.worker_id) || latest.state.0 != "running" {
            return ManagedRuntimeOutcome::Noop {
                reason: "ModelInstance changed before runtime ERROR writeback",
            };
        }
        if let Err(error) = self.registry.remove(instance_id) {
            return failed(error);
        }
        if let Err(error) = self.abort_log_capture(instance_id) {
            return failed(error);
        }
        let message = public_message(message);
        let mut update = ModelInstanceUpdate::from(&latest);
        update.state = ModelInstanceState("error".to_string());
        update.state_message = Some(message.clone());
        match self
            .api
            .update_model_instance(&self.worker_token, self.worker_id, instance_id, &update)
            .await
        {
            Ok(_) => ManagedRuntimeOutcome::Errored { message },
            Err(error) => failed(error),
        }
    }

    async fn refresh_plan(
        &self,
        instance_id: i64,
        require_idle_gpu: bool,
    ) -> Result<RefreshedPlan, ManagedRuntimeError> {
        let instance = self
            .api
            .get_model_instance(&self.worker_token, instance_id)
            .await?;
        let model = self
            .api
            .get_model(&self.worker_token, instance.model_id)
            .await?;
        let planner = ManagedWorkloadPlanner::new(
            self.worker_id,
            self.worker_uuid.clone(),
            self.worker_ip,
            self.resolver.clone(),
            self.store.cache_root(),
            self.container_user.clone(),
            self.gpus.snapshot()?,
        );
        let restart_on_error = model.restart_on_error.unwrap_or(true);
        let spec = if require_idle_gpu {
            planner.plan(&model, &instance)?
        } else {
            planner.plan_existing(&model, &instance)?
        };
        Ok(RefreshedPlan {
            instance,
            spec,
            planner,
            restart_on_error,
        })
    }

    async fn refresh_running_plan(
        &self,
        instance_id: i64,
    ) -> Result<RefreshedPlan, ManagedRuntimeError> {
        let instance = self
            .api
            .get_model_instance(&self.worker_token, instance_id)
            .await?;
        let model = self
            .api
            .get_model(&self.worker_token, instance.model_id)
            .await?;
        let planner = ManagedWorkloadPlanner::new(
            self.worker_id,
            self.worker_uuid.clone(),
            self.worker_ip,
            self.resolver.clone(),
            self.store.cache_root(),
            self.container_user.clone(),
            self.gpus.snapshot()?,
        );
        let restart_on_error = model.restart_on_error.unwrap_or(true);
        let spec = planner.plan_running(&model, &instance)?;
        Ok(RefreshedPlan {
            instance,
            spec,
            planner,
            restart_on_error,
        })
    }

    async fn refresh_error_plan(
        &self,
        instance_id: i64,
    ) -> Result<RefreshedPlan, ManagedRuntimeError> {
        let instance = self
            .api
            .get_model_instance(&self.worker_token, instance_id)
            .await?;
        let model = self
            .api
            .get_model(&self.worker_token, instance.model_id)
            .await?;
        let planner = ManagedWorkloadPlanner::new(
            self.worker_id,
            self.worker_uuid.clone(),
            self.worker_ip,
            self.resolver.clone(),
            self.store.cache_root(),
            self.container_user.clone(),
            self.gpus.snapshot()?,
        );
        let restart_on_error = model.restart_on_error.unwrap_or(true);
        let spec = planner.plan_error(&model, &instance)?;
        Ok(RefreshedPlan {
            instance,
            spec,
            planner,
            restart_on_error,
        })
    }

    async fn refresh_same_plan(
        &self,
        previous: &RefreshedPlan,
        require_idle_gpu: bool,
    ) -> Result<RefreshedPlan, ManagedRuntimeError> {
        let refreshed = self
            .refresh_plan(previous.instance.id, require_idle_gpu)
            .await?;
        if refreshed.spec != previous.spec {
            return Err(ManagedRuntimeError::Unsafe(
                "Managed workload inputs changed during authoritative refresh".to_string(),
            ));
        }
        Ok(refreshed)
    }

    async fn refresh_same_running_plan(
        &self,
        previous: &RefreshedPlan,
    ) -> Result<RefreshedPlan, ManagedRuntimeError> {
        let refreshed = self.refresh_running_plan(previous.instance.id).await?;
        if refreshed.spec != previous.spec {
            return Err(ManagedRuntimeError::Unsafe(
                "Managed running workload inputs changed during authoritative refresh".to_string(),
            ));
        }
        Ok(refreshed)
    }

    async fn refresh_same_error_plan(
        &self,
        previous: &RefreshedPlan,
    ) -> Result<RefreshedPlan, ManagedRuntimeError> {
        let refreshed = self.refresh_error_plan(previous.instance.id).await?;
        if refreshed.spec != previous.spec {
            return Err(ManagedRuntimeError::Unsafe(
                "Managed failed workload inputs changed during authoritative refresh".to_string(),
            ));
        }
        Ok(refreshed)
    }

    async fn reconcile_deleted(&self, manifest: ManagedRuntimeManifest) -> ManagedRuntimeOutcome {
        match self.reconcile_deleted_inner(&manifest).await {
            Ok(removed) => ManagedRuntimeOutcome::Deleted {
                container_removed: removed,
            },
            Err(ManagedRuntimeError::InstanceStillExists) => ManagedRuntimeOutcome::Noop {
                reason: "ModelInstance is absent from the cache but still exists on Server",
            },
            Err(error) => failed(error),
        }
    }

    async fn reconcile_deleted_inner(
        &self,
        manifest: &ManagedRuntimeManifest,
    ) -> Result<bool, ManagedRuntimeError> {
        self.confirm_instance_deleted(manifest.instance_id).await?;
        self.registry.remove(manifest.instance_id)?;
        let filters = BTreeMap::from([(
            LABEL_MODEL_INSTANCE_ID.to_string(),
            manifest.instance_id.to_string(),
        )]);
        let mut removed = false;
        if let OwnedContainer::Present(container) =
            discover_owned_container(self.runtime.as_ref(), &filters, &manifest.spec).await?
        {
            if matches!(
                container.state,
                ContainerState::Running
                    | ContainerState::Paused
                    | ContainerState::Restarting
                    | ContainerState::Stopping
            ) {
                self.confirm_instance_deleted(manifest.instance_id).await?;
                let current =
                    discover_owned_container(self.runtime.as_ref(), &filters, &manifest.spec)
                        .await?;
                let OwnedContainer::Present(current) = current else {
                    return Err(ManagedRuntimeError::Unsafe(
                        "Managed container disappeared before stop".to_string(),
                    ));
                };
                if current.id != container.id {
                    return Err(ManagedRuntimeError::Unsafe(
                        "Managed container identity changed before stop".to_string(),
                    ));
                }
                self.runtime
                    .stop_container(&current.id, STOP_TIMEOUT_SECS)
                    .await?;
            }
            self.confirm_instance_deleted(manifest.instance_id).await?;
            let current =
                discover_owned_container(self.runtime.as_ref(), &filters, &manifest.spec).await?;
            let OwnedContainer::Present(current) = current else {
                return Err(ManagedRuntimeError::Unsafe(
                    "Managed container disappeared before remove".to_string(),
                ));
            };
            if current.id != container.id || current.state == ContainerState::Running {
                return Err(ManagedRuntimeError::Unsafe(
                    "Managed container is not safely removable".to_string(),
                ));
            }
            self.runtime.remove_container(&current.id, false).await?;
            removed = true;
        }

        self.confirm_instance_deleted(manifest.instance_id).await?;
        self.abort_log_capture(manifest.instance_id)?;
        self.store.remove_cache(manifest.instance_id)?;
        self.store.remove_logs(manifest.instance_id)?;
        self.store.remove_manifest(manifest.instance_id)?;
        self.reset_restart_backoff(manifest.instance_id)?;
        Ok(removed)
    }

    async fn confirm_instance_deleted(&self, instance_id: i64) -> Result<(), ManagedRuntimeError> {
        match self
            .api
            .get_model_instance(&self.worker_token, instance_id)
            .await
        {
            Err(ClientError::Api { status: 404, .. })
            | Err(ClientError::UnknownResponse { status: 404, .. }) => Ok(()),
            Ok(_) => Err(ManagedRuntimeError::InstanceStillExists),
            Err(error) => Err(error.into()),
        }
    }

    fn ensure_log_capture(
        &self,
        instance_id: i64,
        container_id: &str,
        restart_count: u32,
    ) -> Result<(), ManagedRuntimeError> {
        let mut tasks = self
            .log_tasks
            .lock()
            .map_err(|_| ManagedRuntimeError::Unsafe("Managed log task lock failed".to_string()))?;
        if tasks.contains_key(&instance_id) {
            return Ok(());
        }
        self.store.prune_logs(instance_id, restart_count)?;
        let path = self.store.prepare_current_log(instance_id, restart_count)?;
        let runtime = Arc::clone(&self.runtime);
        let container_id = container_id.to_string();
        let task = tokio::spawn(async move {
            let Ok(mut file) = OpenOptions::new().append(true).open(&path) else {
                return;
            };
            let mut stream = runtime.logs(
                &container_id,
                ContainerLogOptions {
                    follow: true,
                    since_unix: 0,
                    tail: "all".to_string(),
                    timestamps: false,
                },
            );
            while let Some(line) = stream.next().await {
                let Ok(line) = line else {
                    return;
                };
                let prefix = match line.stream {
                    RuntimeLogStreamKind::Stdout => b"stdout ".as_slice(),
                    RuntimeLogStreamKind::Stderr => b"stderr ".as_slice(),
                    RuntimeLogStreamKind::Stdin => b"stdin ".as_slice(),
                    RuntimeLogStreamKind::Console => b"console ".as_slice(),
                };
                if file
                    .write_all(prefix)
                    .and_then(|_| file.write_all(&line.message))
                    .and_then(|_| file.flush())
                    .is_err()
                {
                    return;
                }
            }
        });
        tasks.insert(instance_id, task);
        Ok(())
    }

    fn abort_log_capture(&self, instance_id: i64) -> Result<(), ManagedRuntimeError> {
        let mut tasks = self
            .log_tasks
            .lock()
            .map_err(|_| ManagedRuntimeError::Unsafe("Managed log task lock failed".to_string()))?;
        if let Some(task) = tasks.remove(&instance_id) {
            task.abort();
        }
        Ok(())
    }

    fn restart_backoff(
        &self,
        instance_id: i64,
    ) -> Result<Option<RestartBackoff>, ManagedRuntimeError> {
        self.restart_backoffs
            .lock()
            .map_err(|_| ManagedRuntimeError::Unsafe("restart backoff lock failed".to_string()))
            .map(|backoffs| backoffs.get(&instance_id).copied())
    }

    fn record_restart_attempt(
        &self,
        instance_id: i64,
        now_unix: u64,
    ) -> Result<(), ManagedRuntimeError> {
        let mut backoffs = self
            .restart_backoffs
            .lock()
            .map_err(|_| ManagedRuntimeError::Unsafe("restart backoff lock failed".to_string()))?;
        let consecutive_failures = backoffs
            .get(&instance_id)
            .map_or(1, |backoff| backoff.consecutive_failures.saturating_add(1));
        backoffs.insert(
            instance_id,
            RestartBackoff {
                consecutive_failures,
                last_attempt_unix: now_unix,
            },
        );
        Ok(())
    }

    fn reset_restart_backoff(&self, instance_id: i64) -> Result<(), ManagedRuntimeError> {
        self.restart_backoffs
            .lock()
            .map_err(|_| ManagedRuntimeError::Unsafe("restart backoff lock failed".to_string()))?
            .remove(&instance_id);
        Ok(())
    }
}

impl<A, G, S, C> Drop for ManagedRuntimeCoordinator<A, G, S, C> {
    fn drop(&mut self) {
        if let Ok(mut tasks) = self.log_tasks.lock() {
            for (_, task) in std::mem::take(&mut *tasks) {
                task.abort();
            }
        }
    }
}

fn restart_delay_secs(consecutive_failures: u32) -> u64 {
    if consecutive_failures == 0 {
        return 0;
    }
    let factor = 1_u64
        .checked_shl(consecutive_failures.saturating_sub(1))
        .unwrap_or(u64::MAX);
    RESTART_BACKOFF_BASE_SECS
        .saturating_mul(factor)
        .min(RESTART_BACKOFF_MAX_SECS)
}

fn public_message(message: &str) -> String {
    crate::runtime_core::redact_diagnostic(message)
        .chars()
        .take(PUBLIC_MESSAGE_LIMIT)
        .collect()
}

fn failed(error: impl std::fmt::Display) -> ManagedRuntimeOutcome {
    ManagedRuntimeOutcome::Failed {
        message: public_message(&error.to_string()),
    }
}

#[derive(Debug, Error)]
pub enum ManagedRuntimeError {
    #[error(transparent)]
    Api(#[from] ClientError),
    #[error(transparent)]
    Plan(#[from] ManagedRuntimePlanError),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    RuntimeCore(#[from] RuntimeCoreError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Store(#[from] ManagedStoreError),
    #[error(transparent)]
    Registry(#[from] ManagedRegistryError),
    #[error("Managed GPU probe failed: {0}")]
    Gpu(String),
    #[error("Managed runtime safety check failed: {0}")]
    Unsafe(String),
    #[error("Managed runtime environment failed: {0}")]
    Environment(String),
    #[error("ModelInstance still exists on authoritative Server")]
    InstanceStillExists,
}

impl ManagedRuntimeError {
    fn preserves_authoritative_state(&self) -> bool {
        // Infrastructure uncertainty cannot prove that the workload itself failed.
        match self {
            Self::Runtime(error) | Self::RuntimeCore(RuntimeCoreError::Runtime(error)) => {
                !matches!(error, RuntimeError::InvalidSpec(_))
            }
            Self::Api(_)
            | Self::Store(_)
            | Self::Registry(_)
            | Self::Gpu(_)
            | Self::Environment(_) => true,
            Self::Plan(_)
            | Self::Resolve(_)
            | Self::RuntimeCore(RuntimeCoreError::Unsafe(_))
            | Self::Unsafe(_)
            | Self::InstanceStillExists => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use futures_util::stream;
    use gpustack_client::types::{ModelInstanceState, ModelSource, Paginated};
    use gpustack_runtime::docker::{
        ContainerCreateResult, ContainerDetails, ContainerListEntry, ContainerSpec,
        DockerDaemonInfo, ImageIdentity, RuntimeFuture, RuntimeLogLine, RuntimeLogStream,
    };

    use super::*;
    use crate::workload::{LABEL_MODE, LABEL_SPEC_HASH};

    #[derive(Clone)]
    struct FakeApi {
        state: Arc<Mutex<FakeApiState>>,
    }

    struct FakeApiState {
        model: ModelPublic,
        instance: Option<ModelInstancePublic>,
        instance_reads: VecDeque<ModelInstancePublic>,
        instance_errors: VecDeque<ClientError>,
        updates: Vec<ModelInstanceUpdate>,
        get_instance_count: usize,
        block_instance_reads: bool,
    }

    impl ManagedRuntimeApi for FakeApi {
        async fn get_model(
            &self,
            _worker_token: &SecretToken,
            _model_id: i64,
        ) -> Result<ModelPublic, ClientError> {
            Ok(self.state.lock().expect("API lock").model.clone())
        }

        async fn get_model_instance(
            &self,
            _worker_token: &SecretToken,
            _instance_id: i64,
        ) -> Result<ModelInstancePublic, ClientError> {
            if self.state.lock().expect("API lock").block_instance_reads {
                std::future::pending::<()>().await;
            }
            let mut state = self.state.lock().expect("API lock");
            state.get_instance_count += 1;
            if let Some(error) = state.instance_errors.pop_front() {
                return Err(error);
            }
            if let Some(instance) = state.instance_reads.pop_front() {
                return Ok(instance);
            }
            state.instance.clone().ok_or_else(not_found)
        }

        async fn update_model_instance(
            &self,
            _worker_token: &SecretToken,
            _worker_id: i64,
            _instance_id: i64,
            update: &ModelInstanceUpdate,
        ) -> Result<ModelInstancePublic, ClientError> {
            let mut state = self.state.lock().expect("API lock");
            state.updates.push(update.clone());
            let instance = state.instance.as_mut().ok_or_else(not_found)?;
            instance.state = update.state.clone();
            instance.state_message = update.state_message.clone();
            instance.restart_count = update.restart_count;
            instance.last_restart_time = update.last_restart_time.clone();
            Ok(instance.clone())
        }
    }

    fn not_found() -> ClientError {
        ClientError::Api {
            status: 404,
            code: 404,
            reason: "Not Found".to_string(),
            message: "missing".to_string(),
        }
    }

    #[derive(Default)]
    struct FakeRuntimeState {
        containers: BTreeMap<String, ContainerDetails>,
        created_specs: Vec<ContainerSpec>,
        starts: usize,
        stops: usize,
        removes: usize,
        logs: Vec<RuntimeLogLine>,
        list_failures: usize,
        start_failures: usize,
    }

    #[derive(Default)]
    struct FakeRuntime {
        state: Mutex<FakeRuntimeState>,
    }

    impl FakeRuntime {
        fn set_state(&self, state: ContainerState) {
            let mut runtime = self.state.lock().expect("runtime lock");
            let container = runtime.containers.values_mut().next().expect("container");
            container.state = state;
            if state == ContainerState::Exited {
                container.exit_code = Some(1);
            }
        }

        fn fail_next_list(&self) {
            self.state.lock().expect("runtime lock").list_failures += 1;
        }

        fn fail_next_start(&self) {
            self.state.lock().expect("runtime lock").start_failures += 1;
        }
    }

    impl ContainerRuntime for FakeRuntime {
        fn daemon_info(&self) -> RuntimeFuture<'_, DockerDaemonInfo> {
            Box::pin(async {
                Ok(DockerDaemonInfo {
                    ping: "OK".to_string(),
                    version: "fake".to_string(),
                    api_version: "fake".to_string(),
                    min_api_version: None,
                    os: Some("linux".to_string()),
                    arch: Some("amd64".to_string()),
                })
            })
        }

        fn inspect_image<'a>(&'a self, image: &'a str) -> RuntimeFuture<'a, ImageIdentity> {
            Box::pin(async move {
                let digest = image.rsplit_once("@sha256:").expect("digest image").1;
                Ok(ImageIdentity {
                    id: format!("sha256:{digest}"),
                    repo_digests: vec![image.to_string()],
                    architecture: Some("amd64".to_string()),
                    os: Some("linux".to_string()),
                })
            })
        }

        fn list_containers<'a>(
            &'a self,
            labels: &'a BTreeMap<String, String>,
        ) -> RuntimeFuture<'a, Vec<ContainerListEntry>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("runtime lock");
                if state.list_failures > 0 {
                    state.list_failures -= 1;
                    return Err(RuntimeError::Client(
                        "scripted Docker list outage".to_string(),
                    ));
                }
                Ok(state
                    .containers
                    .values()
                    .filter(|container| {
                        labels
                            .iter()
                            .all(|(key, value)| container.labels.get(key) == Some(value))
                    })
                    .map(|container| ContainerListEntry {
                        id: container.id.clone(),
                        names: container.name.clone().into_iter().collect(),
                        image: None,
                        image_id: container.image_id.clone(),
                        state: container.state,
                        status: None,
                        labels: container.labels.clone(),
                    })
                    .collect())
            })
        }

        fn create_container<'a>(
            &'a self,
            spec: &'a ContainerSpec,
        ) -> RuntimeFuture<'a, ContainerCreateResult> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("runtime lock");
                let id = format!("container-{}", state.created_specs.len() + 1);
                state.created_specs.push(spec.clone());
                state.containers.insert(
                    id.clone(),
                    ContainerDetails {
                        id: id.clone(),
                        name: Some(spec.name.clone()),
                        image_id: Some(spec.image.clone()),
                        state: ContainerState::Created,
                        exit_code: None,
                        error: None,
                        oom_killed: false,
                        labels: spec.labels.clone(),
                    },
                );
                Ok(ContainerCreateResult {
                    id,
                    warnings: Vec::new(),
                })
            })
        }

        fn start_container<'a>(&'a self, id: &'a str) -> RuntimeFuture<'a, ()> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("runtime lock");
                if state.start_failures > 0 {
                    state.start_failures -= 1;
                    return Err(RuntimeError::Client(
                        "scripted Docker start outage".to_string(),
                    ));
                }
                state.starts += 1;
                state
                    .containers
                    .get_mut(id)
                    .ok_or(RuntimeError::NotFound { operation: "start" })?
                    .state = ContainerState::Running;
                Ok(())
            })
        }

        fn inspect_container<'a>(&'a self, id: &'a str) -> RuntimeFuture<'a, ContainerDetails> {
            Box::pin(async move {
                self.state
                    .lock()
                    .expect("runtime lock")
                    .containers
                    .get(id)
                    .cloned()
                    .ok_or(RuntimeError::NotFound {
                        operation: "inspect",
                    })
            })
        }

        fn stop_container<'a>(&'a self, id: &'a str, _timeout: u64) -> RuntimeFuture<'a, ()> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("runtime lock");
                state.stops += 1;
                state
                    .containers
                    .get_mut(id)
                    .ok_or(RuntimeError::NotFound { operation: "stop" })?
                    .state = ContainerState::Exited;
                Ok(())
            })
        }

        fn remove_container<'a>(&'a self, id: &'a str, _force: bool) -> RuntimeFuture<'a, ()> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("runtime lock");
                state.removes += 1;
                state.containers.remove(id).ok_or(RuntimeError::NotFound {
                    operation: "remove",
                })?;
                Ok(())
            })
        }

        fn logs<'a>(&'a self, _id: &'a str, _options: ContainerLogOptions) -> RuntimeLogStream<'a> {
            let logs = self.state.lock().expect("runtime lock").logs.clone();
            Box::pin(stream::iter(logs.into_iter().map(Ok)))
        }
    }

    #[derive(Clone)]
    struct FakeHealth {
        models: Arc<Mutex<Result<Vec<String>, String>>>,
    }

    impl VllmHealthProbe for FakeHealth {
        fn models<'a>(&'a self, _endpoint: &'a str) -> crate::runtime_core::VllmHealthFuture<'a> {
            let result = self.models.lock().expect("health lock").clone();
            Box::pin(async move { result })
        }
    }

    #[derive(Clone)]
    struct FakeGpuProbe;

    impl ManagedGpuProbe for FakeGpuProbe {
        fn snapshot(&self) -> Result<Vec<ManagedGpu>, ManagedRuntimeError> {
            Ok(vec![ManagedGpu {
                host_index: 0,
                uuid: "GPU-test".to_string(),
                running_compute_processes: 0,
            }])
        }
    }

    #[derive(Clone)]
    struct FakeClock(Arc<AtomicU64>);

    impl ManagedClock for FakeClock {
        fn now_unix(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    struct Fixture {
        _directory: tempfile::TempDir,
        data_dir: std::path::PathBuf,
        log_dir: std::path::PathBuf,
        api: FakeApi,
        runtime: Arc<FakeRuntime>,
        health: Arc<FakeHealth>,
        clock: FakeClock,
        registry: Arc<ManagedRuntimeRegistry>,
        desired: DesiredState,
        coordinator:
            ManagedRuntimeCoordinator<FakeApi, FakeGpuProbe, FsManagedRuntimeStore, FakeClock>,
    }

    impl Fixture {
        fn new(ready: bool) -> Self {
            let directory = tempfile::tempdir().expect("temporary directory");
            let data_dir = directory.path().join("data");
            let log_dir = directory.path().join("logs");
            let model_root = directory.path().join("models");
            let model_path = model_root.join("qwen");
            fs::create_dir_all(&model_path).expect("model directory");
            fs::write(model_path.join("config.json"), b"{}").expect("model file");
            let mut instance =
                serde_json::from_slice::<Paginated<ModelInstancePublic>>(include_bytes!(
                    "../../../../fixtures/server-v2.2.3/model-instances-full-sync.response.json"
                ))
                .expect("instance fixture")
                .items
                .remove(0);
            instance.id = 301;
            instance.model_id = 201;
            instance.model_name = "qwen".to_string();
            instance.name = "qwen-instance".to_string();
            instance.source = ModelSource("local_path".to_string());
            instance.backend = Some("vLLM".to_string());
            instance.local_path = Some(model_path.to_string_lossy().into_owned());
            instance.resolved_path = instance.local_path.clone();
            instance.worker_id = Some(101);
            instance.worker_ip = Some("127.0.0.1".to_string());
            instance.gpu_indexes = Some(vec![0]);
            instance.gpu_addresses = Some(vec!["worker:0".to_string()]);
            instance.port = Some(40_000);
            instance.ports = Some(vec![40_000]);
            instance.state = ModelInstanceState("starting".to_string());
            instance.draft_model_source = None;
            instance.mounted_loras = None;
            instance.injected_backend_parameters = None;
            let model = ModelPublic {
                id: 201,
                name: "qwen".to_string(),
                source: ModelSource("local_path".to_string()),
                backend: Some("vLLM".to_string()),
                backend_parameters: Some(vec!["--enforce-eager".to_string()]),
                env: None,
                restart_on_error: Some(true),
                local_path: instance.local_path.clone(),
                image_name: Some(format!("runner@sha256:{}", "a".repeat(64))),
                extensions: BTreeMap::new(),
            };
            let api = FakeApi {
                state: Arc::new(Mutex::new(FakeApiState {
                    model: model.clone(),
                    instance: Some(instance.clone()),
                    instance_reads: VecDeque::new(),
                    instance_errors: VecDeque::new(),
                    updates: Vec::new(),
                    get_instance_count: 0,
                    block_instance_reads: false,
                })),
            };
            let runtime = Arc::new(FakeRuntime::default());
            runtime
                .state
                .lock()
                .expect("runtime lock")
                .logs
                .push(RuntimeLogLine {
                    stream: RuntimeLogStreamKind::Stdout,
                    message: b"ready\n".to_vec(),
                });
            let health = Arc::new(FakeHealth {
                models: Arc::new(Mutex::new(Ok(if ready {
                    vec!["qwen".to_string()]
                } else {
                    Vec::new()
                }))),
            });
            let clock = FakeClock(Arc::new(AtomicU64::new(10)));
            let store = FsManagedRuntimeStore::open(&data_dir, &log_dir).expect("Managed store");
            let registry = Arc::new(ManagedRuntimeRegistry::default());
            let coordinator = ManagedRuntimeCoordinator::new(
                api.clone(),
                SecretToken::new("worker-token").expect("token"),
                101,
                "worker-uuid".to_string(),
                "127.0.0.1".parse().expect("worker IP"),
                LocalModelResolver::new([model_root]).expect("resolver"),
                "1000:1000".to_string(),
                runtime.clone(),
                health.clone(),
                FakeGpuProbe,
                store,
                clock.clone(),
                Arc::clone(&registry),
            );
            let desired = DesiredState {
                models: BTreeMap::from([(model.id, model)]),
                model_instances: BTreeMap::from([(instance.id, instance)]),
                model_files: BTreeMap::new(),
                revision: 1,
            };
            Self {
                _directory: directory,
                data_dir,
                log_dir,
                api,
                runtime,
                health,
                clock,
                registry,
                desired,
                coordinator,
            }
        }

        async fn create(&self) -> ManagedRuntimeOutcome {
            self.coordinator
                .reconcile_snapshot(&self.desired)
                .await
                .remove(0)
                .outcome
        }

        async fn start_and_promote(&mut self) {
            assert!(matches!(
                self.create().await,
                ManagedRuntimeOutcome::Starting { .. }
            ));
            assert!(matches!(
                self.create().await,
                ManagedRuntimeOutcome::Running { .. }
            ));
            self.set_running();
        }

        fn set_running(&mut self) {
            let running = {
                let mut api = self.api.state.lock().expect("API lock");
                let instance = api.instance.as_mut().expect("instance");
                instance.state = ModelInstanceState("running".to_string());
                instance.state_message = None;
                instance.clone()
            };
            self.desired.model_instances.insert(running.id, running);
        }

        fn set_state(&mut self, state: &str) {
            let instance = {
                let mut api = self.api.state.lock().expect("API lock");
                let instance = api.instance.as_mut().expect("instance");
                instance.state = ModelInstanceState(state.to_string());
                instance.state_message = None;
                instance.clone()
            };
            self.desired.model_instances.insert(instance.id, instance);
        }

        fn sync_desired_instance(&mut self) {
            let instance = self
                .api
                .state
                .lock()
                .expect("API lock")
                .instance
                .clone()
                .expect("instance");
            self.desired.model_instances.insert(instance.id, instance);
        }

        fn set_restart_on_error(&self, enabled: bool) {
            self.api
                .state
                .lock()
                .expect("API lock")
                .model
                .restart_on_error = Some(enabled);
        }

        fn advance_clock(&self, seconds: u64) {
            self.clock.0.fetch_add(seconds, Ordering::SeqCst);
        }

        fn fail_next_instance_read(&self) {
            self.api
                .state
                .lock()
                .expect("API lock")
                .instance_errors
                .push_back(ClientError::Watch(
                    "scripted Server read outage".to_string(),
                ));
        }

        fn set_health_error(&self, message: &str) {
            *self.health.models.lock().expect("health lock") = Err(message.to_string());
        }
    }

    #[tokio::test]
    async fn creates_starts_logs_and_promotes_only_after_health() {
        let fixture = Fixture::new(true);

        let created = fixture.create().await;
        assert!(matches!(
            created,
            ManagedRuntimeOutcome::Starting { created: true, .. }
        ));
        let running = fixture.create().await;
        assert!(matches!(
            running,
            ManagedRuntimeOutcome::Running {
                health_ready_at_unix_nanos: 10_000_000_000,
                ..
            }
        ));
        assert_eq!(
            fixture.registry.route(301).expect("Managed route"),
            Some(super::super::ManagedRoute {
                instance_id: 301,
                port: 40_000,
                served_model_name: "qwen".to_string(),
            })
        );
        tokio::task::yield_now().await;

        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.created_specs.len(), 1);
        assert_eq!(runtime.starts, 1);
        drop(runtime);
        let api = fixture.api.state.lock().expect("API lock");
        assert_eq!(
            api.updates.last().expect("RUNNING update").state.0,
            "running"
        );
        assert!(api.get_instance_count >= 4);
        drop(api);
        let log = fixture.log_dir.join("serve/301.0.log");
        assert!(
            fs::read_to_string(log)
                .expect("read log")
                .contains("stdout ready")
        );
    }

    #[tokio::test]
    async fn repeated_starting_events_adopt_without_duplicate_create() {
        let fixture = Fixture::new(false);

        fixture.create().await;
        let repeated = fixture.create().await;

        assert!(matches!(
            repeated,
            ManagedRuntimeOutcome::Starting {
                created: false,
                adopted: true,
                ..
            }
        ));
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.created_specs.len(), 1);
        assert_eq!(runtime.starts, 1);
    }

    #[tokio::test]
    async fn running_snapshot_adopts_the_same_healthy_container_without_mutation() {
        let mut fixture = Fixture::new(true);
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Starting { created: true, .. }
        ));
        fixture.set_running();

        let outcome = fixture.create().await;

        assert!(matches!(
            outcome,
            ManagedRuntimeOutcome::Running {
                ref container_id,
                ..
            } if container_id == "container-1"
        ));
        {
            let runtime = fixture.runtime.state.lock().expect("runtime lock");
            assert_eq!(runtime.created_specs.len(), 1);
            assert_eq!(runtime.starts, 1);
            assert_eq!(runtime.stops, 0);
            assert_eq!(runtime.removes, 0);
        }
        assert!(
            fixture
                .api
                .state
                .lock()
                .expect("API lock")
                .updates
                .is_empty()
        );
        assert_eq!(
            fixture.registry.route(301).expect("Managed route"),
            Some(super::super::ManagedRoute {
                instance_id: 301,
                port: 40_000,
                served_model_name: "qwen".to_string(),
            })
        );

        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Noop {
                reason: "RUNNING Managed workload is already adopted"
            }
        ));
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.created_specs.len(), 1);
        assert_eq!(runtime.starts, 1);
        assert_eq!(runtime.stops, 0);
        assert_eq!(runtime.removes, 0);
    }

    #[tokio::test]
    async fn running_snapshot_rebuilds_a_missing_recovery_manifest() {
        let mut fixture = Fixture::new(true);
        fixture.create().await;
        let manifest_path = fixture.data_dir.join("managed/manifests/301.json");
        fs::remove_file(&manifest_path).expect("remove recovery manifest");
        fixture.set_running();

        let outcome = fixture.create().await;

        assert!(matches!(outcome, ManagedRuntimeOutcome::Running { .. }));
        let manifest: ManagedRuntimeManifest =
            serde_json::from_slice(&fs::read(manifest_path).expect("recreated manifest"))
                .expect("manifest JSON");
        assert_eq!(manifest.container_id, "container-1");
        assert_eq!(manifest.last_observed_state, "ready");
        assert_eq!(manifest.restart_count, 0);
    }

    #[tokio::test]
    async fn new_coordinator_rebuilds_route_and_log_capture_without_restarting_container() {
        let mut fixture = Fixture::new(true);
        fixture.create().await;
        fixture.set_running();
        let data_dir = fixture.data_dir.clone();
        let log_dir = fixture.log_dir.clone();
        let model_root = data_dir.parent().expect("fixture root").join("models");
        let api = fixture.api.clone();
        let runtime = Arc::clone(&fixture.runtime);
        let health = Arc::clone(&fixture.health);
        let clock = fixture.clock.clone();
        let desired = fixture.desired.clone();
        drop(fixture.coordinator);

        let log_path = log_dir.join("serve/301.0.log");
        fs::write(&log_path, b"stale duplicate\n").expect("seed previous process log");
        let registry = Arc::new(ManagedRuntimeRegistry::default());
        let coordinator = ManagedRuntimeCoordinator::new(
            api,
            SecretToken::new("worker-token").expect("token"),
            101,
            "worker-uuid".to_string(),
            "127.0.0.1".parse().expect("worker IP"),
            LocalModelResolver::new([model_root]).expect("resolver"),
            "1000:1000".to_string(),
            runtime.clone(),
            health,
            FakeGpuProbe,
            FsManagedRuntimeStore::open(&data_dir, &log_dir).expect("reopen Managed store"),
            clock,
            Arc::clone(&registry),
        );

        let outcome = coordinator
            .reconcile_snapshot(&desired)
            .await
            .remove(0)
            .outcome;

        assert!(matches!(outcome, ManagedRuntimeOutcome::Running { .. }));
        assert!(registry.route(301).expect("rebuilt route").is_some());
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let captured = fs::read_to_string(&log_path).expect("captured log");
                if captured.contains("stdout ready") {
                    assert!(!captured.contains("stale duplicate"));
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("reattached log capture");
        let runtime = runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.created_specs.len(), 1);
        assert_eq!(runtime.starts, 1);
        assert_eq!(runtime.stops, 0);
        assert_eq!(runtime.removes, 0);
    }

    #[tokio::test]
    async fn startup_adopts_a_nonzero_restart_generation_and_keeps_two_logs() {
        let mut fixture = Fixture::new(true);
        fixture.create().await;
        fixture.set_running();
        {
            let mut api = fixture.api.state.lock().expect("API lock");
            api.instance.as_mut().expect("instance").restart_count = Some(2);
        }
        fixture.sync_desired_instance();
        let manifest_path = fixture.data_dir.join("managed/manifests/301.json");
        let mut manifest: ManagedRuntimeManifest =
            serde_json::from_slice(&fs::read(&manifest_path).expect("manifest"))
                .expect("manifest JSON");
        manifest.restart_count = 2;
        fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).expect("serialize manifest"),
        )
        .expect("update manifest generation");
        let log_dir = fixture.log_dir.join("serve");
        fs::write(log_dir.join("301.1.log"), b"previous\n").expect("previous log");
        fs::write(log_dir.join("301.2.log"), b"stale\n").expect("current log");

        let data_dir = fixture.data_dir.clone();
        let worker_log_dir = fixture.log_dir.clone();
        let model_root = data_dir.parent().expect("fixture root").join("models");
        let api = fixture.api.clone();
        let runtime = Arc::clone(&fixture.runtime);
        let health = Arc::clone(&fixture.health);
        let clock = fixture.clock.clone();
        let desired = fixture.desired.clone();
        drop(fixture.coordinator);
        let registry = Arc::new(ManagedRuntimeRegistry::default());
        let coordinator = ManagedRuntimeCoordinator::new(
            api,
            SecretToken::new("worker-token").expect("token"),
            101,
            "worker-uuid".to_string(),
            "127.0.0.1".parse().expect("worker IP"),
            LocalModelResolver::new([model_root]).expect("resolver"),
            "1000:1000".to_string(),
            runtime,
            health,
            FakeGpuProbe,
            FsManagedRuntimeStore::open(&data_dir, &worker_log_dir).expect("reopen Managed store"),
            clock,
            Arc::clone(&registry),
        );

        let outcome = coordinator
            .reconcile_snapshot(&desired)
            .await
            .remove(0)
            .outcome;

        assert!(matches!(outcome, ManagedRuntimeOutcome::Running { .. }));
        assert!(registry.route(301).expect("Managed route").is_some());
        assert!(!log_dir.join("301.0.log").exists());
        assert!(log_dir.join("301.1.log").is_file());
        assert!(log_dir.join("301.2.log").is_file());
    }

    #[tokio::test]
    async fn running_snapshot_without_a_container_enters_error_before_restart() {
        let mut fixture = Fixture::new(true);
        fixture.set_running();

        let outcome = fixture.create().await;

        let ManagedRuntimeOutcome::Errored { message } = outcome else {
            panic!("missing RUNNING container must enter ERROR")
        };
        assert!(message.contains("no owned container"));
        {
            let runtime = fixture.runtime.state.lock().expect("runtime lock");
            assert!(runtime.created_specs.is_empty());
            assert_eq!(runtime.starts, 0);
            assert_eq!(runtime.stops, 0);
            assert_eq!(runtime.removes, 0);
        }
        {
            let api = fixture.api.state.lock().expect("API lock");
            assert_eq!(api.updates.len(), 1);
            assert_eq!(api.updates[0].state.0, "error");
        }
        assert!(
            fixture
                .registry
                .route(301)
                .expect("Managed route")
                .is_none()
        );

        fixture.sync_desired_instance();
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::RestartScheduled { restart_count: 1 }
        ));
        let api = fixture.api.state.lock().expect("API lock");
        let restart = api.updates.last().expect("restart update");
        assert_eq!(restart.state.0, "scheduled");
        assert_eq!(restart.restart_count, Some(1));
        assert_eq!(
            restart.last_restart_time.as_deref(),
            Some("1970-01-01T00:00:10Z")
        );
    }

    #[tokio::test]
    async fn error_state_respects_disabled_restart_policy() {
        let mut fixture = Fixture::new(true);
        fixture.set_state("error");
        fixture.set_restart_on_error(false);

        let outcome = fixture.create().await;

        assert!(matches!(
            outcome,
            ManagedRuntimeOutcome::Noop {
                reason: "restart_on_error is disabled"
            }
        ));
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert!(runtime.created_specs.is_empty());
        assert_eq!(runtime.starts, 0);
        assert_eq!(runtime.stops, 0);
        assert_eq!(runtime.removes, 0);
        assert!(
            fixture
                .api
                .state
                .lock()
                .expect("API lock")
                .updates
                .is_empty()
        );
    }

    #[tokio::test]
    async fn failed_runtime_restarts_with_backoff_and_two_log_generations() {
        let mut fixture = Fixture::new(true);
        fixture.create().await;
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Running { .. }
        ));
        fixture.set_running();
        fixture.runtime.set_state(ContainerState::Exited);
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Errored { .. }
        ));
        fixture.sync_desired_instance();

        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::RestartScheduled { restart_count: 1 }
        ));
        fixture.set_state("starting");
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Starting { created: true, .. }
        ));
        assert!(fixture.log_dir.join("serve/301.0.log").is_file());
        assert!(fixture.log_dir.join("serve/301.1.log").is_file());
        let manifest: ManagedRuntimeManifest = serde_json::from_slice(
            &fs::read(fixture.data_dir.join("managed/manifests/301.json"))
                .expect("restart manifest"),
        )
        .expect("manifest JSON");
        assert_eq!(manifest.restart_count, 1);

        fixture.runtime.set_state(ContainerState::Exited);
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Errored { .. }
        ));
        fixture.sync_desired_instance();
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::RestartPending {
                delay_secs: 10,
                remaining_secs: 10
            }
        ));
        fixture.advance_clock(9);
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::RestartPending {
                delay_secs: 10,
                remaining_secs: 1
            }
        ));
        fixture.advance_clock(1);
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::RestartScheduled { restart_count: 2 }
        ));

        fixture.set_state("starting");
        fixture.create().await;
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Running { .. }
        ));
        assert!(!fixture.log_dir.join("serve/301.0.log").exists());
        assert!(fixture.log_dir.join("serve/301.1.log").is_file());
        assert!(fixture.log_dir.join("serve/301.2.log").is_file());

        fixture.set_running();
        fixture.runtime.set_state(ContainerState::Exited);
        fixture.create().await;
        fixture.sync_desired_instance();
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::RestartScheduled { restart_count: 3 }
        ));
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.created_specs.len(), 3);
        assert_eq!(runtime.starts, 3);
        assert_eq!(runtime.removes, 3);
    }

    #[test]
    fn restart_delay_matches_the_frozen_python_sequence_and_cap() {
        assert_eq!(restart_delay_secs(0), 0);
        assert_eq!(restart_delay_secs(1), 10);
        assert_eq!(restart_delay_secs(2), 20);
        assert_eq!(restart_delay_secs(3), 40);
        assert_eq!(restart_delay_secs(4), 80);
        assert_eq!(restart_delay_secs(5), 160);
        assert_eq!(restart_delay_secs(6), 300);
        assert_eq!(restart_delay_secs(32), 300);
    }

    #[test]
    fn only_uncertain_infrastructure_errors_preserve_the_server_state() {
        assert!(
            ManagedRuntimeError::Runtime(RuntimeError::Client(
                "scripted Docker outage".to_string()
            ))
            .preserves_authoritative_state()
        );
        assert!(
            ManagedRuntimeError::Api(ClientError::Watch("scripted Server outage".to_string()))
                .preserves_authoritative_state()
        );
        assert!(
            !ManagedRuntimeError::Runtime(RuntimeError::InvalidSpec(
                "invalid workload".to_string()
            ))
            .preserves_authoritative_state()
        );
        assert!(
            !ManagedRuntimeError::Unsafe("confirmed workload failure".to_string())
                .preserves_authoritative_state()
        );
    }

    #[tokio::test]
    async fn transient_server_read_failure_preserves_running_route_and_runtime() {
        let mut fixture = Fixture::new(true);
        fixture.start_and_promote().await;
        let updates_before = fixture.api.state.lock().expect("API lock").updates.len();
        fixture.fail_next_instance_read();

        let outcome = fixture.create().await;

        assert!(matches!(outcome, ManagedRuntimeOutcome::Failed { .. }));
        assert!(
            fixture
                .registry
                .route(301)
                .expect("Managed route")
                .is_some()
        );
        {
            let runtime = fixture.runtime.state.lock().expect("runtime lock");
            assert_eq!(runtime.containers.len(), 1);
            assert_eq!(runtime.stops, 0);
            assert_eq!(runtime.removes, 0);
        }
        assert_eq!(
            fixture.api.state.lock().expect("API lock").updates.len(),
            updates_before
        );
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Noop { .. }
        ));
    }

    #[tokio::test]
    async fn transient_docker_read_failure_preserves_running_route_and_server_state() {
        let mut fixture = Fixture::new(true);
        fixture.start_and_promote().await;
        let updates_before = fixture.api.state.lock().expect("API lock").updates.len();
        fixture.runtime.fail_next_list();

        let outcome = fixture.create().await;

        let ManagedRuntimeOutcome::Failed { message } = outcome else {
            panic!("Docker uncertainty must remain local")
        };
        assert!(message.contains("Docker"));
        assert!(
            fixture
                .registry
                .route(301)
                .expect("Managed route")
                .is_some()
        );
        {
            let runtime = fixture.runtime.state.lock().expect("runtime lock");
            assert_eq!(runtime.containers.len(), 1);
            assert_eq!(runtime.stops, 0);
            assert_eq!(runtime.removes, 0);
        }
        assert_eq!(
            fixture.api.state.lock().expect("API lock").updates.len(),
            updates_before
        );
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Noop { .. }
        ));
    }

    #[tokio::test]
    async fn transient_health_probe_failure_preserves_running_route_and_state() {
        let mut fixture = Fixture::new(true);
        fixture.start_and_promote().await;
        let updates_before = fixture.api.state.lock().expect("API lock").updates.len();
        fixture.set_health_error("scripted connection reset");

        let outcome = fixture.create().await;

        assert_eq!(
            outcome,
            ManagedRuntimeOutcome::Noop {
                reason: "RUNNING Managed workload has no confirmed terminal failure"
            }
        );
        assert!(
            fixture
                .registry
                .route(301)
                .expect("Managed route")
                .is_some()
        );
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.stops, 0);
        assert_eq!(runtime.removes, 0);
        drop(runtime);
        assert_eq!(
            fixture.api.state.lock().expect("API lock").updates.len(),
            updates_before
        );
    }

    #[tokio::test]
    async fn transient_docker_start_failure_preserves_starting_state_and_recovers() {
        let fixture = Fixture::new(true);
        fixture.runtime.fail_next_start();

        let outcome = fixture.create().await;

        let ManagedRuntimeOutcome::Failed { message } = outcome else {
            panic!("Docker start uncertainty must remain local")
        };
        assert!(message.contains("Docker"));
        assert!(
            fixture
                .api
                .state
                .lock()
                .expect("API lock")
                .updates
                .is_empty()
        );
        {
            let runtime = fixture.runtime.state.lock().expect("runtime lock");
            assert_eq!(runtime.containers.len(), 1);
            assert_eq!(runtime.starts, 0);
            assert_eq!(runtime.stops, 0);
            assert_eq!(runtime.removes, 0);
        }

        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Starting {
                created: false,
                adopted: true,
                ..
            }
        ));
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.containers.len(), 1);
        assert_eq!(runtime.starts, 1);
    }

    #[tokio::test]
    async fn authoritative_delete_during_backoff_waits_for_full_sync_cleanup() {
        let mut fixture = Fixture::new(true);
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Starting { .. }
        ));
        fixture.set_state("error");
        fixture
            .coordinator
            .record_restart_attempt(301, fixture.clock.now_unix())
            .expect("restart attempt");

        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::RestartPending {
                delay_secs: 10,
                remaining_secs: 10
            }
        ));
        fixture.api.state.lock().expect("API lock").instance = None;
        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Failed { .. }
        ));
        {
            let runtime = fixture.runtime.state.lock().expect("runtime lock");
            assert_eq!(runtime.stops, 0);
            assert_eq!(runtime.removes, 0);
        }

        fixture.desired.model_instances.remove(&301);
        let outcome = fixture
            .coordinator
            .reconcile_snapshot(&fixture.desired)
            .await
            .remove(0)
            .outcome;

        assert_eq!(
            outcome,
            ManagedRuntimeOutcome::Deleted {
                container_removed: true
            }
        );
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.stops, 1);
        assert_eq!(runtime.removes, 1);
        assert!(runtime.containers.is_empty());
    }

    #[tokio::test]
    async fn deletion_waits_for_server_and_docker_recovery_before_mutation() {
        let mut fixture = Fixture::new(true);
        fixture.create().await;
        fixture.api.state.lock().expect("API lock").instance = None;
        fixture.desired.model_instances.remove(&301);
        fixture.fail_next_instance_read();

        let server_outage = fixture
            .coordinator
            .reconcile_snapshot(&fixture.desired)
            .await
            .remove(0)
            .outcome;

        assert!(matches!(
            server_outage,
            ManagedRuntimeOutcome::Failed { .. }
        ));
        fixture.runtime.fail_next_list();
        let docker_outage = fixture
            .coordinator
            .reconcile_snapshot(&fixture.desired)
            .await
            .remove(0)
            .outcome;
        assert!(matches!(
            docker_outage,
            ManagedRuntimeOutcome::Failed { .. }
        ));
        {
            let runtime = fixture.runtime.state.lock().expect("runtime lock");
            assert_eq!(runtime.containers.len(), 1);
            assert_eq!(runtime.stops, 0);
            assert_eq!(runtime.removes, 0);
        }
        assert!(
            fixture
                .data_dir
                .join("managed/manifests/301.json")
                .is_file()
        );

        let recovered = fixture
            .coordinator
            .reconcile_snapshot(&fixture.desired)
            .await
            .remove(0)
            .outcome;

        assert_eq!(
            recovered,
            ManagedRuntimeOutcome::Deleted {
                container_removed: true
            }
        );
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.stops, 1);
        assert_eq!(runtime.removes, 1);
        assert!(runtime.containers.is_empty());
    }

    #[tokio::test]
    async fn unknown_and_conflicting_containers_are_never_mutated() {
        let mut fixture = Fixture::new(true);
        fixture.create().await;
        {
            let mut runtime = fixture.runtime.state.lock().expect("runtime lock");
            let mut unknown = runtime
                .containers
                .values()
                .next()
                .expect("owned container")
                .clone();
            unknown.id = "unknown-container".to_string();
            unknown.name = Some("unrelated-runtime".to_string());
            unknown.labels.remove(LABEL_MODEL_INSTANCE_ID);
            runtime.containers.insert(unknown.id.clone(), unknown);
        }
        fixture.set_running();

        assert!(matches!(
            fixture.create().await,
            ManagedRuntimeOutcome::Running { .. }
        ));
        {
            let runtime = fixture.runtime.state.lock().expect("runtime lock");
            assert!(runtime.containers.contains_key("unknown-container"));
            assert_eq!(runtime.stops, 0);
            assert_eq!(runtime.removes, 0);
        }

        let mut foreign = Fixture::new(true);
        foreign.create().await;
        foreign
            .runtime
            .state
            .lock()
            .expect("runtime lock")
            .containers
            .values_mut()
            .next()
            .expect("container")
            .labels
            .insert(LABEL_MODE.to_string(), "lab".to_string());
        foreign.set_running();
        assert!(matches!(
            foreign.create().await,
            ManagedRuntimeOutcome::Failed { .. }
        ));
        {
            let runtime = foreign.runtime.state.lock().expect("runtime lock");
            assert_eq!(runtime.stops, 0);
            assert_eq!(runtime.removes, 0);
        }
        assert!(
            foreign
                .api
                .state
                .lock()
                .expect("API lock")
                .updates
                .is_empty()
        );

        let mut conflicting = Fixture::new(true);
        conflicting.create().await;
        conflicting
            .runtime
            .state
            .lock()
            .expect("runtime lock")
            .containers
            .values_mut()
            .next()
            .expect("container")
            .labels
            .insert(LABEL_SPEC_HASH.to_string(), "b".repeat(64));
        conflicting.set_running();
        assert!(matches!(
            conflicting.create().await,
            ManagedRuntimeOutcome::Failed { .. }
        ));
        let runtime = conflicting.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.stops, 0);
        assert_eq!(runtime.removes, 0);
        drop(runtime);
        assert!(
            conflicting
                .api
                .state
                .lock()
                .expect("API lock")
                .updates
                .is_empty()
        );
    }

    #[tokio::test]
    async fn running_snapshot_does_not_adopt_an_unhealthy_container() {
        let mut fixture = Fixture::new(false);
        fixture.create().await;
        fixture.set_running();

        let outcome = fixture.create().await;

        let ManagedRuntimeOutcome::Failed { message } = outcome else {
            panic!("unhealthy RUNNING container must fail locally")
        };
        assert!(message.contains("not healthy enough"));
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.created_specs.len(), 1);
        assert_eq!(runtime.starts, 1);
        assert_eq!(runtime.stops, 0);
        assert_eq!(runtime.removes, 0);
        assert!(
            fixture
                .registry
                .route(301)
                .expect("Managed route")
                .is_none()
        );
        assert!(
            fixture
                .api
                .state
                .lock()
                .expect("API lock")
                .updates
                .is_empty()
        );
    }

    #[tokio::test]
    async fn running_snapshot_refuses_a_conflicting_recovery_manifest() {
        let mut fixture = Fixture::new(true);
        fixture.create().await;
        let manifest_path = fixture.data_dir.join("managed/manifests/301.json");
        let mut manifest: ManagedRuntimeManifest =
            serde_json::from_slice(&fs::read(&manifest_path).expect("manifest"))
                .expect("manifest JSON");
        manifest.container_id = "different-container".to_string();
        let cache_path = fixture.data_dir.join("managed/cache/301");
        fs::remove_dir_all(&cache_path).expect("remove recovery cache");
        fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).expect("serialize manifest"),
        )
        .expect("write conflicting manifest");
        fixture.set_running();

        let outcome = fixture.create().await;

        let ManagedRuntimeOutcome::Failed { message } = outcome else {
            panic!("conflicting recovery manifest must fail locally")
        };
        assert!(message.contains("manifest conflicts"));
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.created_specs.len(), 1);
        assert_eq!(runtime.starts, 1);
        assert_eq!(runtime.stops, 0);
        assert_eq!(runtime.removes, 0);
        assert!(!cache_path.exists());
        assert!(
            fixture
                .registry
                .route(301)
                .expect("Managed route")
                .is_none()
        );
    }

    #[tokio::test]
    async fn coordinator_readiness_starts_after_convergence_and_clears_on_shutdown() {
        let fixture = Fixture::new(false);
        let (_desired_tx, desired_rx) = watch::channel(fixture.desired.clone());
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let run = fixture.coordinator.run(desired_rx, shutdown_rx, |_| {});
        tokio::pin!(run);

        tokio::time::timeout(Duration::from_secs(1), async {
            while !fixture.registry.is_ready() {
                tokio::select! {
                    () = &mut run => panic!("coordinator stopped before readiness"),
                    () = tokio::task::yield_now() => {}
                }
            }
        })
        .await
        .expect("coordinator readiness");
        shutdown_tx.send_replace(true);
        run.await;
        assert!(!fixture.registry.is_ready());
    }

    #[tokio::test]
    async fn shutdown_interrupts_an_in_flight_runtime_reconciliation() {
        let fixture = Fixture::new(false);
        fixture
            .api
            .state
            .lock()
            .expect("API lock")
            .block_instance_reads = true;
        let (_desired_tx, desired_rx) = watch::channel(fixture.desired.clone());
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let run = fixture.coordinator.run(desired_rx, shutdown_rx, |_| {});
        tokio::pin!(run);
        tokio::task::yield_now().await;

        shutdown_tx.send_replace(true);

        tokio::time::timeout(Duration::from_secs(1), &mut run)
            .await
            .expect("runtime coordinator shutdown timeout");
        assert!(!fixture.registry.is_ready());
    }

    #[tokio::test]
    async fn exited_container_reports_bounded_redacted_error_without_removal() {
        let fixture = Fixture::new(false);
        fixture.create().await;
        {
            let mut runtime = fixture.runtime.state.lock().expect("runtime lock");
            runtime.logs = vec![RuntimeLogLine {
                stream: RuntimeLogStreamKind::Stderr,
                message: b"authorization token=secret\n".to_vec(),
            }];
        }
        fixture.runtime.set_state(ContainerState::Exited);

        let outcome = fixture.create().await;

        let ManagedRuntimeOutcome::Errored { message } = outcome else {
            panic!("expected ERROR writeback")
        };
        assert!(!message.contains("token=secret"));
        assert!(message.len() <= PUBLIC_MESSAGE_LIMIT);
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.removes, 0);
        assert_eq!(runtime.containers.len(), 1);
    }

    #[tokio::test]
    async fn readiness_timeout_reports_error_and_preserves_container() {
        let fixture = Fixture::new(false);
        fixture.create().await;
        fixture.clock.0.store(700, Ordering::SeqCst);

        let outcome = fixture.create().await;

        assert!(matches!(outcome, ManagedRuntimeOutcome::Errored { .. }));
        assert_eq!(
            fixture
                .runtime
                .state
                .lock()
                .expect("runtime lock")
                .containers
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn cache_gap_does_not_cleanup_an_authoritative_instance() {
        let fixture = Fixture::new(false);
        fixture.create().await;
        let desired = DesiredState::default();

        let outcome = fixture
            .coordinator
            .reconcile_snapshot(&desired)
            .await
            .remove(0)
            .outcome;

        assert!(matches!(outcome, ManagedRuntimeOutcome::Noop { .. }));
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.stops, 0);
        assert_eq!(runtime.removes, 0);
    }

    #[tokio::test]
    async fn authoritative_deletion_removes_only_owned_runtime_and_local_state() {
        let fixture = Fixture::new(false);
        fixture.create().await;
        fs::write(fixture.log_dir.join("serve/301.1.log"), b"previous")
            .expect("previous restart log");
        fs::write(fixture.log_dir.join("serve/301.2.log"), b"current")
            .expect("current restart log");
        fixture.api.state.lock().expect("API lock").instance = None;

        let outcome = fixture
            .coordinator
            .reconcile_snapshot(&DesiredState::default())
            .await
            .remove(0)
            .outcome;

        assert_eq!(
            outcome,
            ManagedRuntimeOutcome::Deleted {
                container_removed: true
            }
        );
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert_eq!(runtime.stops, 1);
        assert_eq!(runtime.removes, 1);
        assert!(runtime.containers.is_empty());
        drop(runtime);
        assert!(!fixture.data_dir.join("managed/manifests/301.json").exists());
        assert!(!fixture.data_dir.join("managed/cache/301").exists());
        assert!(!fixture.log_dir.join("serve/301.0.log").exists());
        assert!(!fixture.log_dir.join("serve/301.1.log").exists());
        assert!(!fixture.log_dir.join("serve/301.2.log").exists());
        assert!(
            fixture
                .registry
                .route(301)
                .expect("Managed route")
                .is_none()
        );
    }

    #[tokio::test]
    async fn run_loop_reconciles_an_authoritative_desired_state_deletion() {
        let fixture = Fixture::new(false);
        fixture.create().await;
        fixture.api.state.lock().expect("API lock").instance = None;
        let (desired_tx, desired_rx) = watch::channel(DesiredState::default());
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let run = fixture.coordinator.run(desired_rx, shutdown_rx, |_| {});
        tokio::pin!(run);

        tokio::task::yield_now().await;
        assert_eq!(
            fixture
                .runtime
                .state
                .lock()
                .expect("runtime lock")
                .containers
                .len(),
            1
        );
        assert!(!fixture.registry.is_ready());
        desired_tx.send_replace(DesiredState {
            revision: 1,
            ..DesiredState::default()
        });

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if fixture
                    .runtime
                    .state
                    .lock()
                    .expect("runtime lock")
                    .containers
                    .is_empty()
                {
                    return;
                }
                tokio::select! {
                    () = &mut run => panic!("coordinator stopped before deletion"),
                    () = tokio::task::yield_now() => {}
                }
            }
        })
        .await
        .expect("authoritative deletion convergence");
        shutdown_tx.send_replace(true);
        run.await;
        assert!(!fixture.data_dir.join("managed/manifests/301.json").exists());
        assert!(!fixture.log_dir.join("serve/301.0.log").exists());
    }

    #[tokio::test]
    async fn input_drift_before_create_prevents_every_runtime_mutation() {
        let fixture = Fixture::new(false);
        let original = fixture.desired.model_instances[&301].clone();
        let mut changed = original.clone();
        changed.port = Some(40_001);
        changed.ports = Some(vec![40_001]);
        fixture
            .api
            .state
            .lock()
            .expect("API lock")
            .instance_reads
            .extend([original, changed]);

        let outcome = fixture.create().await;

        assert!(matches!(outcome, ManagedRuntimeOutcome::Errored { .. }));
        let runtime = fixture.runtime.state.lock().expect("runtime lock");
        assert!(runtime.created_specs.is_empty());
        assert_eq!(runtime.starts, 0);
    }

    #[tokio::test]
    async fn state_drift_before_running_writeback_preserves_server_authority() {
        let fixture = Fixture::new(true);
        fixture.create().await;
        {
            let mut api = fixture.api.state.lock().expect("API lock");
            let starting = api.instance.clone().expect("starting instance");
            let mut externally_running = starting.clone();
            externally_running.state = ModelInstanceState("running".to_string());
            externally_running.state_message = Some("external transition".to_string());
            api.instance = Some(externally_running.clone());
            api.instance_reads.extend([starting, externally_running]);
        }

        let outcome = fixture.create().await;

        assert_eq!(
            outcome,
            ManagedRuntimeOutcome::Noop {
                reason: "ModelInstance changed before ERROR writeback",
            }
        );
        assert!(
            fixture
                .api
                .state
                .lock()
                .expect("API lock")
                .updates
                .is_empty()
        );
        assert!(
            fixture
                .registry
                .route(301)
                .expect("Managed route")
                .is_none()
        );
    }
}
