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
    },
    Errored {
        message: String,
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
        }
    }

    pub async fn reconcile_snapshot(&self, desired: &DesiredState) -> Vec<ManagedRuntimeResult> {
        let mut results = Vec::new();
        for instance in desired.model_instances.values().filter(|instance| {
            instance.worker_id == Some(self.worker_id) && instance.state.0 == "starting"
        }) {
            results.push(ManagedRuntimeResult {
                instance_id: instance.id,
                outcome: self.reconcile_starting(instance.id).await,
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
            let results = self.reconcile_snapshot(&desired.borrow().clone()).await;
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
            Err(ManagedRuntimeError::Api(error)) => failed(error),
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
            restart_count: 0,
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
        self.ensure_log_capture(refreshed.instance.id, &created.id)?;
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
                if manifest.container_id != container_id || manifest.spec != plan.spec {
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
                restart_count: 0,
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
            self.ensure_log_capture(plan.instance.id, &container_id)?;
            manifest.last_observed_state = "starting".to_string();
            self.store.save_manifest(&manifest)?;
            return Ok(ManagedRuntimeOutcome::Starting {
                container_id,
                created,
                adopted: true,
            });
        }
        if observation.container.state == ContainerState::Running {
            self.ensure_log_capture(plan.instance.id, &container_id)?;
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
        Ok(ManagedRuntimeOutcome::Running {
            container_id: container_id.to_string(),
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
        let spec = if require_idle_gpu {
            planner.plan(&model, &instance)?
        } else {
            planner.plan_existing(&model, &instance)?
        };
        Ok(RefreshedPlan {
            instance,
            spec,
            planner,
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
        self.store
            .remove_current_log(manifest.instance_id, manifest.restart_count)?;
        self.store.remove_manifest(manifest.instance_id)?;
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
    ) -> Result<(), ManagedRuntimeError> {
        let mut tasks = self
            .log_tasks
            .lock()
            .map_err(|_| ManagedRuntimeError::Unsafe("Managed log task lock failed".to_string()))?;
        if tasks.contains_key(&instance_id) {
            return Ok(());
        }
        let path = self.store.prepare_current_log(instance_id, 0)?;
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

    #[derive(Clone)]
    struct FakeApi {
        state: Arc<Mutex<FakeApiState>>,
    }

    struct FakeApiState {
        model: ModelPublic,
        instance: Option<ModelInstancePublic>,
        instance_reads: VecDeque<ModelInstancePublic>,
        updates: Vec<ModelInstanceUpdate>,
        get_instance_count: usize,
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
            let mut state = self.state.lock().expect("API lock");
            state.get_instance_count += 1;
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
                Ok(self
                    .state
                    .lock()
                    .expect("runtime lock")
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
                local_path: instance.local_path.clone(),
                image_name: Some(format!("runner@sha256:{}", "a".repeat(64))),
                extensions: BTreeMap::new(),
            };
            let api = FakeApi {
                state: Arc::new(Mutex::new(FakeApiState {
                    model: model.clone(),
                    instance: Some(instance.clone()),
                    instance_reads: VecDeque::new(),
                    updates: Vec::new(),
                    get_instance_count: 0,
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
        assert!(matches!(running, ManagedRuntimeOutcome::Running { .. }));
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
        assert!(
            fixture
                .registry
                .route(301)
                .expect("Managed route")
                .is_none()
        );
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
