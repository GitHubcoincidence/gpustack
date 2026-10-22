use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use gpustack_runtime::docker::{
    ContainerDetails, ContainerRuntime, ContainerState, DockerRuntime, ImageIdentity, RuntimeError,
};

use super::{
    FsLabStore, FsModelPathResolver, KernelUuidSource, LAB_MODE, LAB_PORT_END, LAB_PORT_START,
    LAB_SCHEMA_VERSION, LabCleanupPlan, LabClock, LabDeployRequest, LabDeployment, LabError,
    LabGpu, LabGpuProbe, LabHttpClient, LabIdSource, LabInferRequest, LabInferenceResponse,
    LabLogLine, LabOwner, LabState, LabStatus, LabStore, LabStoreSession, ModelPathResolver,
    NvmlLabGpuProbe, PortProbe, ReqwestLabHttpClient, SystemLabClock, TcpPortProbe, log_options,
};
use crate::WorkerConfig;
use crate::workload::{
    LABEL_IMPLEMENTATION, LABEL_MODE, LABEL_OWNER_ID, WorkloadBuildRequest, WorkloadGpu,
    WorkloadMode, WorkloadSpecV1,
};

const LAB_BIND_ADDRESS: &str = "127.0.0.1";
const STOP_TIMEOUT_SECS: u64 = 30;

pub struct LabService<'a> {
    runtime: &'a dyn ContainerRuntime,
    store: &'a dyn LabStore,
    paths: &'a dyn ModelPathResolver,
    gpus: &'a dyn LabGpuProbe,
    ports: &'a dyn PortProbe,
    http: &'a dyn LabHttpClient,
    clock: &'a dyn LabClock,
    ids: &'a dyn LabIdSource,
    model_roots: &'a [PathBuf],
}

impl<'a> LabService<'a> {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        runtime: &'a dyn ContainerRuntime,
        store: &'a dyn LabStore,
        paths: &'a dyn ModelPathResolver,
        gpus: &'a dyn LabGpuProbe,
        ports: &'a dyn PortProbe,
        http: &'a dyn LabHttpClient,
        clock: &'a dyn LabClock,
        ids: &'a dyn LabIdSource,
        model_roots: &'a [PathBuf],
    ) -> Self {
        Self {
            runtime,
            store,
            paths,
            gpus,
            ports,
            http,
            clock,
            ids,
            model_roots,
        }
    }

    pub async fn deploy(&self, request: LabDeployRequest) -> Result<LabStatus, LabError> {
        validate_name(&request.name)?;
        let mut session = self.store.lock()?;
        if session.managed_lock_active()? {
            return Err(LabError::Conflict(
                "Rust Managed mode currently holds the data-directory lock".to_string(),
            ));
        }
        let owner = ensure_owner(session.as_mut(), self.ids)?;
        let deployments = load_current_deployments(session.as_ref())?;
        let existing = find_by_name(&deployments, &request.name)?.cloned();
        let lab_id = existing
            .as_ref()
            .map(|deployment| deployment.lab_id.clone())
            .map_or_else(|| self.ids.next_id(), Ok)?;
        let model_path = self.paths.resolve(self.model_roots, &request.model_path)?;
        let image = self.resolve_image(&request.image).await?;
        let gpu = self.gpus.resolve(&request.gpu)?;
        let port = match (request.port, existing.as_ref()) {
            (Some(port), _) => port,
            (None, Some(deployment)) => deployment.spec.service_port,
            (None, None) => self.allocate_port(&deployments)?,
        };
        validate_explicit_port(port)?;
        let cache_path = session.cache_path(&lab_id)?;
        let spec = WorkloadSpecV1::build(WorkloadBuildRequest {
            mode: WorkloadMode::Lab,
            owner_id: owner.owner_id,
            deployment_id: lab_id.clone(),
            managed_worker_uuid: None,
            model_path,
            cache_path,
            served_model_name: request
                .served_model_name
                .unwrap_or_else(|| request.name.clone()),
            requested_image: request.image,
            resolved_image_digest: image,
            gpus: vec![WorkloadGpu {
                uuid: gpu.uuid.clone(),
                host_index: gpu.host_index,
            }],
            service_host: LAB_BIND_ADDRESS.to_string(),
            service_port: port,
            tensor_parallel_size: 1,
            vllm: request.vllm,
            environment: request.environment,
        })?;

        if let Some(mut deployment) = existing {
            if deployment.spec.spec_hash != spec.spec_hash {
                return Err(LabError::Conflict(format!(
                    "deployment '{}' already exists with a different normalized spec; stop and remove it first",
                    deployment.name
                )));
            }
            ensure_gpu_not_recorded_by_other(&deployments, &deployment.lab_id, &gpu.uuid)?;
            return self.activate(session.as_mut(), &mut deployment, &gpu).await;
        }

        ensure_gpu_not_recorded_by_other(&deployments, &lab_id, &gpu.uuid)?;
        ensure_gpu_idle(&gpu)?;
        ensure_port_not_recorded(&deployments, &lab_id, port)?;
        self.ensure_port_available(port)?;
        session.prepare_cache(&lab_id)?;
        let now = self.clock.now_unix();
        let mut deployment = LabDeployment {
            schema_version: LAB_SCHEMA_VERSION,
            lab_id,
            name: request.name,
            state: LabState::Created,
            spec,
            container_id: None,
            last_error: None,
            created_at_unix: now,
            updated_at_unix: now,
        };
        session.save_deployment(&deployment)?;
        self.activate(session.as_mut(), &mut deployment, &gpu).await
    }

    pub async fn list(&self) -> Result<Vec<LabStatus>, LabError> {
        let mut session = self.store.lock()?;
        let deployments = load_current_deployments(session.as_ref())?;
        let mut statuses = Vec::with_capacity(deployments.len());
        for mut deployment in deployments {
            statuses.push(
                self.status_locked(session.as_mut(), &mut deployment)
                    .await?,
            );
        }
        Ok(statuses)
    }

    pub async fn status(&self, name: &str) -> Result<LabStatus, LabError> {
        let mut session = self.store.lock()?;
        let mut deployment = load_by_name(session.as_ref(), name)?;
        self.status_locked(session.as_mut(), &mut deployment).await
    }

    pub async fn logs(
        &self,
        name: &str,
        follow: bool,
        since_unix: i32,
        tail: String,
    ) -> Result<Vec<LabLogLine>, LabError> {
        let mut lines = Vec::new();
        self.stream_logs(name, follow, since_unix, tail, |line| lines.push(line))
            .await?;
        Ok(lines)
    }

    pub async fn stream_logs<F>(
        &self,
        name: &str,
        follow: bool,
        since_unix: i32,
        tail: String,
        mut emit: F,
    ) -> Result<(), LabError>
    where
        F: FnMut(LabLogLine),
    {
        validate_tail(&tail)?;
        let session = self.store.lock()?;
        let deployment = load_by_name(session.as_ref(), name)?;
        let container_id = deployment.container_id.clone().ok_or_else(|| {
            LabError::NotFound(format!("deployment '{}' has no container", deployment.name))
        })?;
        let details = self.runtime.inspect_container(&container_id).await?;
        deployment
            .spec
            .verify_ownership(&details.labels)
            .map_err(|error| LabError::Unsafe(error.to_string()))?;
        drop(session);

        let mut stream = self
            .runtime
            .logs(&container_id, log_options(follow, since_unix, tail));
        while let Some(line) = stream.next().await {
            emit(line?.into());
        }
        Ok(())
    }

    pub async fn infer(&self, request: &LabInferRequest) -> Result<LabInferenceResponse, LabError> {
        if request.prompt.is_empty() {
            return Err(LabError::Invalid(
                "Lab inference prompt must not be empty".to_string(),
            ));
        }
        let mut session = self.store.lock()?;
        let mut deployment = load_by_name(session.as_ref(), &request.name)?;
        let status = self
            .status_locked(session.as_mut(), &mut deployment)
            .await?;
        if status.state != LabState::Running {
            return Err(LabError::Conflict(format!(
                "deployment '{}' is not RUNNING",
                deployment.name
            )));
        }
        let endpoint = endpoint(&deployment.spec);
        let model = deployment.spec.served_model_name.clone();
        let lab_id = deployment.lab_id.clone();
        drop(session);
        let response = self.http.infer(&endpoint, &model, request).await?;
        Ok(LabInferenceResponse {
            mode: LAB_MODE,
            lab_id,
            endpoint,
            response,
        })
    }

    pub async fn stop(&self, name: &str) -> Result<LabStatus, LabError> {
        let mut session = self.store.lock()?;
        let mut deployment = load_by_name(session.as_ref(), name)?;
        let container_id = deployment.container_id.clone().ok_or_else(|| {
            LabError::NotFound(format!("deployment '{}' has no container", deployment.name))
        })?;
        let details = self.runtime.inspect_container(&container_id).await?;
        deployment
            .spec
            .verify_ownership(&details.labels)
            .map_err(|error| LabError::Unsafe(error.to_string()))?;
        if details.state == ContainerState::Running {
            self.runtime
                .stop_container(&container_id, STOP_TIMEOUT_SECS)
                .await?;
        }
        deployment.state = LabState::Stopped;
        deployment.last_error = None;
        deployment.updated_at_unix = self.clock.now_unix();
        session.save_deployment(&deployment)?;
        Ok(status_from(&deployment, Some(ContainerState::Exited)))
    }

    pub async fn remove(&self, name: &str) -> Result<LabStatus, LabError> {
        let mut session = self.store.lock()?;
        let deployment = load_by_name(session.as_ref(), name)?;
        if !deployment.state.is_removable() {
            return Err(LabError::Conflict(format!(
                "deployment '{}' must be STOPPED or FAILED before removal",
                deployment.name
            )));
        }
        let mut container_state = None;
        if let Some(container_id) = &deployment.container_id
            && let Some(details) = self.inspect_optional(container_id).await?
        {
            deployment
                .spec
                .verify_ownership(&details.labels)
                .map_err(|error| LabError::Unsafe(error.to_string()))?;
            if details.state == ContainerState::Running {
                return Err(LabError::Unsafe(
                    "manifest is removable but its owned container is still running".to_string(),
                ));
            }
            container_state = Some(details.state);
            self.runtime.remove_container(container_id, false).await?;
        }
        session.remove_cache(&deployment.lab_id)?;
        session.remove_deployment(&deployment.lab_id)?;
        Ok(status_from(&deployment, container_state))
    }

    pub async fn cleanup(&self, execute: bool) -> Result<LabCleanupPlan, LabError> {
        let mut session = self.store.lock()?;
        let Some(owner) = session.load_owner()? else {
            let cache_ids = session.list_cache_ids()?;
            let blocked = (!cache_ids.is_empty())
                .then(|| "Lab owner is missing, so cache ownership cannot be proven".to_string())
                .into_iter()
                .collect::<Vec<_>>();
            if execute && !blocked.is_empty() {
                return Err(LabError::Unsafe(blocked[0].clone()));
            }
            return Ok(LabCleanupPlan {
                mode: LAB_MODE,
                dry_run: !execute,
                deployment_ids: Vec::new(),
                container_ids: Vec::new(),
                cache_ids,
                blocked,
                removed_deployments: 0,
                removed_containers: 0,
                removed_caches: 0,
            });
        };
        let deployments = load_current_deployments(session.as_ref())?;
        let cache_ids = session.list_cache_ids()?;
        let filters = BTreeMap::from([
            (LABEL_IMPLEMENTATION.to_string(), "rust".to_string()),
            (LABEL_MODE.to_string(), LAB_MODE.to_string()),
            (LABEL_OWNER_ID.to_string(), owner.owner_id),
        ]);
        let listed = self.runtime.list_containers(&filters).await?;
        let mut plan = LabCleanupPlan {
            mode: LAB_MODE,
            dry_run: !execute,
            deployment_ids: deployments
                .iter()
                .map(|deployment| deployment.lab_id.clone())
                .collect(),
            container_ids: Vec::new(),
            cache_ids,
            blocked: Vec::new(),
            removed_deployments: 0,
            removed_containers: 0,
            removed_caches: 0,
        };
        let mut referenced = BTreeSet::new();
        for deployment in &deployments {
            if deployment.state.is_active() {
                plan.blocked.push(format!(
                    "deployment {} is {}",
                    deployment.lab_id,
                    state_name(deployment.state)
                ));
            }
            if let Some(container_id) = &deployment.container_id
                && let Some(details) = self.inspect_optional(container_id).await?
            {
                referenced.insert(details.id.clone());
                if let Err(error) = deployment.spec.verify_ownership(&details.labels) {
                    plan.blocked.push(format!(
                        "container {} ownership mismatch: {error}",
                        details.id
                    ));
                } else {
                    if matches!(
                        details.state,
                        ContainerState::Running
                            | ContainerState::Paused
                            | ContainerState::Restarting
                            | ContainerState::Stopping
                    ) {
                        plan.blocked.push(format!(
                            "container {} is still {}",
                            details.id,
                            container_state_name(details.state)
                        ));
                    }
                    plan.container_ids.push(details.id);
                }
            }
        }
        for container in listed {
            if !referenced.contains(&container.id) {
                plan.blocked.push(format!(
                    "owned-label container {} has no matching deployment manifest",
                    container.id
                ));
            }
        }
        plan.container_ids.sort();
        plan.container_ids.dedup();
        plan.blocked.sort();

        if !execute {
            return Ok(plan);
        }
        if !plan.blocked.is_empty() {
            return Err(LabError::Unsafe(format!(
                "cleanup is blocked: {}",
                plan.blocked.join("; ")
            )));
        }
        for container_id in &plan.container_ids {
            self.runtime.remove_container(container_id, false).await?;
            plan.removed_containers += 1;
        }
        for deployment in &deployments {
            session.remove_deployment(&deployment.lab_id)?;
            plan.removed_deployments += 1;
        }
        for cache_id in &plan.cache_ids {
            session.remove_cache(cache_id)?;
            plan.removed_caches += 1;
        }
        Ok(plan)
    }

    async fn activate(
        &self,
        session: &mut dyn LabStoreSession,
        deployment: &mut LabDeployment,
        gpu: &LabGpu,
    ) -> Result<LabStatus, LabError> {
        let result = self.activate_inner(session, deployment, gpu).await;
        if let Err(error) = &result {
            deployment.state = LabState::Failed;
            deployment.last_error = Some(redact_diagnostic(&error.to_string()));
            deployment.updated_at_unix = self.clock.now_unix();
            session.save_deployment(deployment)?;
        }
        result
    }

    async fn activate_inner(
        &self,
        session: &mut dyn LabStoreSession,
        deployment: &mut LabDeployment,
        gpu: &LabGpu,
    ) -> Result<LabStatus, LabError> {
        let existing = match deployment.container_id.as_deref() {
            Some(container_id) => self.inspect_optional(container_id).await?,
            None => None,
        };
        if let Some(details) = &existing {
            deployment
                .spec
                .verify_ownership(&details.labels)
                .map_err(|error| LabError::Unsafe(error.to_string()))?;
            if details.state == ContainerState::Running {
                return self.wait_until_ready(session, deployment).await;
            }
            if matches!(
                details.state,
                ContainerState::Paused
                    | ContainerState::Restarting
                    | ContainerState::Removing
                    | ContainerState::Stopping
            ) {
                return Err(LabError::Conflict(format!(
                    "owned container {} is in transitional state {}",
                    details.id,
                    container_state_name(details.state)
                )));
            }
        }

        ensure_gpu_idle(gpu)?;
        self.ensure_port_available(deployment.spec.service_port)?;
        session.prepare_cache(&deployment.lab_id)?;
        if let Some(details) = existing {
            deployment.state = LabState::Starting;
            deployment.last_error = None;
            deployment.updated_at_unix = self.clock.now_unix();
            session.save_deployment(deployment)?;
            self.runtime.start_container(&details.id).await?;
        } else {
            deployment.state = LabState::Preparing;
            deployment.last_error = None;
            deployment.updated_at_unix = self.clock.now_unix();
            session.save_deployment(deployment)?;
            let created = self
                .runtime
                .create_container(&deployment.spec.container_spec())
                .await?;
            deployment.container_id = Some(created.id.clone());
            deployment.state = LabState::Starting;
            deployment.updated_at_unix = self.clock.now_unix();
            session.save_deployment(deployment)?;
            let details = self.runtime.inspect_container(&created.id).await?;
            deployment
                .spec
                .verify_ownership(&details.labels)
                .map_err(|error| LabError::Unsafe(error.to_string()))?;
            self.runtime.start_container(&created.id).await?;
        }
        self.wait_until_ready(session, deployment).await
    }

    async fn wait_until_ready(
        &self,
        session: &mut dyn LabStoreSession,
        deployment: &mut LabDeployment,
    ) -> Result<LabStatus, LabError> {
        let deadline = self
            .clock
            .now_unix()
            .saturating_add(deployment.spec.health.deadline_secs);
        let endpoint = endpoint(&deployment.spec);
        let mut last_health_error = None;
        loop {
            let container_id = deployment
                .container_id
                .as_deref()
                .expect("activated deployment has a container ID");
            let details = self.runtime.inspect_container(container_id).await?;
            deployment
                .spec
                .verify_ownership(&details.labels)
                .map_err(|error| LabError::Unsafe(error.to_string()))?;
            if matches!(details.state, ContainerState::Exited | ContainerState::Dead) {
                return Err(LabError::DeploymentFailed {
                    lab_id: deployment.lab_id.clone(),
                    message: self.container_failure_diagnostic(&details).await,
                });
            }
            if details.state == ContainerState::Running {
                match self.http.models(&endpoint).await {
                    Ok(models)
                        if models
                            .iter()
                            .any(|model| model == &deployment.spec.served_model_name) =>
                    {
                        deployment.state = LabState::Running;
                        deployment.last_error = None;
                        deployment.updated_at_unix = self.clock.now_unix();
                        session.save_deployment(deployment)?;
                        return Ok(status_from(deployment, Some(details.state)));
                    }
                    Ok(_) => {
                        last_health_error = Some(format!(
                            "expected model '{}' is absent from /v1/models",
                            deployment.spec.served_model_name
                        ));
                    }
                    Err(error) => last_health_error = Some(redact_diagnostic(&error.to_string())),
                }
            }
            if self.clock.now_unix() >= deadline {
                return Err(LabError::DeploymentFailed {
                    lab_id: deployment.lab_id.clone(),
                    message: last_health_error
                        .unwrap_or_else(|| "vLLM health deadline expired".to_string()),
                });
            }
            self.clock
                .sleep(Duration::from_secs(
                    deployment.spec.health.interval_secs.max(1),
                ))
                .await;
        }
    }

    async fn status_locked(
        &self,
        session: &mut dyn LabStoreSession,
        deployment: &mut LabDeployment,
    ) -> Result<LabStatus, LabError> {
        let Some(container_id) = deployment.container_id.as_deref() else {
            deployment.state = LabState::Failed;
            deployment.last_error = Some("deployment has no container ID".to_string());
            deployment.updated_at_unix = self.clock.now_unix();
            session.save_deployment(deployment)?;
            return Ok(status_from(deployment, None));
        };
        let Some(details) = self.inspect_optional(container_id).await? else {
            deployment.state = LabState::Failed;
            deployment.last_error = Some("owned container is missing".to_string());
            deployment.updated_at_unix = self.clock.now_unix();
            session.save_deployment(deployment)?;
            return Ok(status_from(deployment, None));
        };
        deployment
            .spec
            .verify_ownership(&details.labels)
            .map_err(|error| LabError::Unsafe(error.to_string()))?;
        match details.state {
            ContainerState::Running => match self.http.models(&endpoint(&deployment.spec)).await {
                Ok(models)
                    if models
                        .iter()
                        .any(|model| model == &deployment.spec.served_model_name) =>
                {
                    deployment.state = LabState::Running;
                    deployment.last_error = None;
                }
                Ok(_) => {
                    deployment.state = LabState::Starting;
                    deployment.last_error = Some(format!(
                        "expected model '{}' is absent from /v1/models",
                        deployment.spec.served_model_name
                    ));
                }
                Err(error) => {
                    deployment.state = LabState::Starting;
                    deployment.last_error = Some(redact_diagnostic(&error.to_string()));
                }
            },
            ContainerState::Exited | ContainerState::Dead => {
                if deployment.state != LabState::Stopped {
                    deployment.state = LabState::Failed;
                    deployment.last_error = Some(self.container_failure_diagnostic(&details).await);
                }
            }
            _ => {
                deployment.state = LabState::Starting;
            }
        }
        deployment.updated_at_unix = self.clock.now_unix();
        session.save_deployment(deployment)?;
        Ok(status_from(deployment, Some(details.state)))
    }

    async fn inspect_optional(
        &self,
        container_id: &str,
    ) -> Result<Option<ContainerDetails>, LabError> {
        match self.runtime.inspect_container(container_id).await {
            Ok(details) => Ok(Some(details)),
            Err(RuntimeError::NotFound { .. }) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn resolve_image(&self, requested: &str) -> Result<String, LabError> {
        let requested_digest = digest(requested).ok_or_else(|| {
            LabError::Invalid("Lab image must use an immutable SHA-256 digest".to_string())
        })?;
        let identity = self.runtime.inspect_image(requested).await?;
        if !image_matches(&identity, requested, requested_digest) {
            return Err(LabError::Unsafe(
                "local image identity does not match the requested immutable digest".to_string(),
            ));
        }
        if identity.os.as_deref() != Some("linux")
            || !matches!(identity.architecture.as_deref(), Some("amd64" | "x86_64"))
        {
            return Err(LabError::Unsafe(
                "Lab runner image must target Linux amd64".to_string(),
            ));
        }
        Ok(requested.to_string())
    }

    fn allocate_port(&self, deployments: &[LabDeployment]) -> Result<u16, LabError> {
        let reserved: BTreeSet<u16> = deployments
            .iter()
            .map(|deployment| deployment.spec.service_port)
            .collect();
        for port in LAB_PORT_START..=LAB_PORT_END {
            if !reserved.contains(&port) && self.ports.is_available(LAB_BIND_ADDRESS, port)? {
                return Ok(port);
            }
        }
        Err(LabError::Conflict(format!(
            "no free Lab port remains in {LAB_PORT_START}-{LAB_PORT_END}"
        )))
    }

    fn ensure_port_available(&self, port: u16) -> Result<(), LabError> {
        if self.ports.is_available(LAB_BIND_ADDRESS, port)? {
            Ok(())
        } else {
            Err(LabError::Conflict(format!(
                "Lab port {LAB_BIND_ADDRESS}:{port} is already in use"
            )))
        }
    }

    async fn container_failure_diagnostic(&self, details: &ContainerDetails) -> String {
        let mut diagnostic = container_failure(details);
        let mut logs = self
            .runtime
            .logs(&details.id, log_options(false, 0, "20".to_string()));
        let mut tail = String::new();
        while tail.len() < 4096 {
            match logs.next().await {
                Some(Ok(line)) => {
                    let remaining = 4096usize.saturating_sub(tail.len());
                    tail.extend(
                        String::from_utf8_lossy(&line.message)
                            .chars()
                            .take(remaining),
                    );
                }
                Some(Err(_)) | None => break,
            }
        }
        let tail = redact_diagnostic(tail.trim());
        if !tail.is_empty() {
            diagnostic.push_str("; log tail: ");
            diagnostic.push_str(&tail);
        }
        redact_diagnostic(&diagnostic)
    }
}

pub struct LabEnvironment {
    runtime: DockerRuntime,
    store: FsLabStore,
    paths: FsModelPathResolver,
    gpus: NvmlLabGpuProbe,
    ports: TcpPortProbe,
    http: ReqwestLabHttpClient,
    clock: SystemLabClock,
    ids: KernelUuidSource,
    model_roots: Vec<PathBuf>,
}

impl LabEnvironment {
    pub fn new(config: &WorkerConfig, request_timeout: Duration) -> Result<Self, LabError> {
        Ok(Self {
            runtime: DockerRuntime::connect(&config.docker_socket, request_timeout)?,
            store: FsLabStore::new(config.data_dir.join("lab")),
            paths: FsModelPathResolver,
            gpus: NvmlLabGpuProbe,
            ports: TcpPortProbe,
            http: ReqwestLabHttpClient::new(request_timeout)?,
            clock: SystemLabClock,
            ids: KernelUuidSource,
            model_roots: config.model_roots.clone(),
        })
    }

    #[must_use]
    pub fn service(&self) -> LabService<'_> {
        LabService::new(
            &self.runtime,
            &self.store,
            &self.paths,
            &self.gpus,
            &self.ports,
            &self.http,
            &self.clock,
            &self.ids,
            &self.model_roots,
        )
    }
}

fn ensure_owner(
    session: &mut dyn LabStoreSession,
    ids: &dyn LabIdSource,
) -> Result<LabOwner, LabError> {
    if let Some(owner) = session.load_owner()? {
        if owner.schema_version != LAB_SCHEMA_VERSION {
            return Err(LabError::Unsafe(
                "Lab owner schema version is unsupported".to_string(),
            ));
        }
        return Ok(owner);
    }
    let owner = LabOwner {
        schema_version: LAB_SCHEMA_VERSION,
        owner_id: ids.next_id()?,
    };
    session.save_owner(&owner)?;
    Ok(owner)
}

fn load_by_name(session: &dyn LabStoreSession, name: &str) -> Result<LabDeployment, LabError> {
    validate_name(name)?;
    find_by_name(&load_current_deployments(session)?, name)?
        .cloned()
        .ok_or_else(|| LabError::NotFound(name.to_string()))
}

fn load_current_deployments(session: &dyn LabStoreSession) -> Result<Vec<LabDeployment>, LabError> {
    let deployments = session.list_deployments()?;
    if deployments.is_empty() {
        return Ok(deployments);
    }
    let owner = session.load_owner()?.ok_or_else(|| {
        LabError::Unsafe("Lab deployments exist without a local owner record".to_string())
    })?;
    for deployment in &deployments {
        if deployment.schema_version != LAB_SCHEMA_VERSION
            || deployment.spec.mode != WorkloadMode::Lab
            || deployment.spec.deployment_id != deployment.lab_id
            || deployment.spec.owner_id != owner.owner_id
        {
            return Err(LabError::Unsafe(format!(
                "Lab deployment manifest identity is inconsistent: {}",
                deployment.lab_id
            )));
        }
        deployment
            .spec
            .verify_integrity()
            .map_err(|error| LabError::Unsafe(error.to_string()))?;
    }
    Ok(deployments)
}

fn find_by_name<'a>(
    deployments: &'a [LabDeployment],
    name: &str,
) -> Result<Option<&'a LabDeployment>, LabError> {
    let mut matches = deployments
        .iter()
        .filter(|deployment| deployment.name == name);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(LabError::Unsafe(format!(
            "multiple Lab manifests use deployment name '{name}'"
        )));
    }
    Ok(first)
}

fn validate_name(name: &str) -> Result<(), LabError> {
    if name.is_empty()
        || name.len() > 63
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        || matches!(name, "." | "..")
    {
        return Err(LabError::Invalid(
            "Lab name must contain 1-63 safe ASCII characters".to_string(),
        ));
    }
    Ok(())
}

fn validate_explicit_port(port: u16) -> Result<(), LabError> {
    if port == 0 {
        Err(LabError::Invalid(
            "Lab service port must be greater than zero".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn validate_tail(tail: &str) -> Result<(), LabError> {
    if tail == "all" || tail.parse::<u64>().is_ok() {
        Ok(())
    } else {
        Err(LabError::Invalid(
            "Lab log tail must be 'all' or a non-negative integer".to_string(),
        ))
    }
}

fn ensure_gpu_idle(gpu: &LabGpu) -> Result<(), LabError> {
    if gpu.running_compute_processes == 0 {
        Ok(())
    } else {
        Err(LabError::Conflict(format!(
            "GPU {} has {} running compute process(es)",
            gpu.uuid, gpu.running_compute_processes
        )))
    }
}

fn ensure_gpu_not_recorded_by_other(
    deployments: &[LabDeployment],
    current_id: &str,
    gpu_uuid: &str,
) -> Result<(), LabError> {
    if let Some(other) = deployments.iter().find(|deployment| {
        deployment.lab_id != current_id
            && deployment.state.is_active()
            && deployment.spec.gpus.iter().any(|gpu| gpu.uuid == gpu_uuid)
    }) {
        return Err(LabError::Conflict(format!(
            "GPU {gpu_uuid} is reserved by Lab deployment '{}'",
            other.name
        )));
    }
    Ok(())
}

fn ensure_port_not_recorded(
    deployments: &[LabDeployment],
    current_id: &str,
    port: u16,
) -> Result<(), LabError> {
    if let Some(other) = deployments
        .iter()
        .find(|deployment| deployment.lab_id != current_id && deployment.spec.service_port == port)
    {
        return Err(LabError::Conflict(format!(
            "port {port} is recorded by Lab deployment '{}'",
            other.name
        )));
    }
    Ok(())
}

fn digest(value: &str) -> Option<&str> {
    value
        .rsplit_once("@sha256:")
        .map(|(_, digest)| digest)
        .or_else(|| value.strip_prefix("sha256:"))
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn image_matches(identity: &ImageIdentity, requested: &str, requested_digest: &str) -> bool {
    identity.id.strip_prefix("sha256:") == Some(requested_digest)
        || identity.repo_digests.iter().any(|digest_value| {
            digest_value == requested || digest(digest_value) == Some(requested_digest)
        })
}

fn endpoint(spec: &WorkloadSpecV1) -> String {
    format!("http://{}:{}", spec.service_host, spec.service_port)
}

fn status_from(deployment: &LabDeployment, container_state: Option<ContainerState>) -> LabStatus {
    LabStatus {
        mode: LAB_MODE,
        lab_id: deployment.lab_id.clone(),
        name: deployment.name.clone(),
        state: deployment.state,
        container_state: container_state
            .map(container_state_name)
            .map(str::to_string),
        container_id: deployment.container_id.clone(),
        spec_hash: deployment.spec.spec_hash.clone(),
        image_digest: deployment.spec.resolved_image_digest.clone(),
        gpu_uuid: deployment.spec.gpus[0].uuid.clone(),
        endpoint: endpoint(&deployment.spec),
        last_error: deployment.last_error.clone(),
    }
}

fn container_state_name(state: ContainerState) -> &'static str {
    match state {
        ContainerState::Created => "created",
        ContainerState::Running => "running",
        ContainerState::Paused => "paused",
        ContainerState::Restarting => "restarting",
        ContainerState::Removing => "removing",
        ContainerState::Exited => "exited",
        ContainerState::Dead => "dead",
        ContainerState::Stopping => "stopping",
        ContainerState::Unknown => "unknown",
    }
}

fn state_name(state: LabState) -> &'static str {
    match state {
        LabState::Created => "CREATED",
        LabState::Preparing => "PREPARING",
        LabState::Starting => "STARTING",
        LabState::Running => "RUNNING",
        LabState::Failed => "FAILED",
        LabState::Stopped => "STOPPED",
    }
}

fn container_failure(details: &ContainerDetails) -> String {
    let reason = if details.oom_killed {
        "container was OOM-killed".to_string()
    } else if let Some(error) = &details.error {
        redact_diagnostic(error)
    } else {
        "container exited before vLLM became ready".to_string()
    };
    match details.exit_code {
        Some(code) => format!("{reason} (exit code {code})"),
        None => reason,
    }
}

fn redact_diagnostic(message: &str) -> String {
    let lowered = message.to_ascii_lowercase();
    if ["token", "password", "api-key", "api_key", "authorization"]
        .iter()
        .any(|needle| lowered.contains(needle))
    {
        return "diagnostic omitted because it may contain a secret".to_string();
    }
    message
        .chars()
        .filter(|character| !character.is_control() || *character == ' ')
        .take(512)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use futures_util::stream;
    use gpustack_runtime::docker::{
        ContainerCreateResult, ContainerListEntry, ContainerLogOptions, ContainerSpec,
        DockerDaemonInfo, ImageIdentity, RuntimeFuture, RuntimeLogLine, RuntimeLogStream,
        RuntimeLogStreamKind,
    };
    use serde_json::json;

    use super::*;
    use crate::lab::{ClockFuture, LabFuture, LabInferenceKind};
    use crate::workload::VllmOptions;

    const IMAGE: &str =
        "runner@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[derive(Default)]
    struct MemoryData {
        owner: Option<LabOwner>,
        deployments: BTreeMap<String, LabDeployment>,
        caches: BTreeSet<String>,
        managed_lock_active: bool,
    }

    #[derive(Default)]
    struct MemoryStore {
        data: Arc<Mutex<MemoryData>>,
        locked: Arc<AtomicBool>,
    }

    impl LabStore for MemoryStore {
        fn lock(&self) -> Result<Box<dyn LabStoreSession>, LabError> {
            if self.locked.swap(true, Ordering::SeqCst) {
                return Err(LabError::Busy);
            }
            Ok(Box::new(MemorySession {
                data: Arc::clone(&self.data),
                locked: Arc::clone(&self.locked),
            }))
        }
    }

    struct MemorySession {
        data: Arc<Mutex<MemoryData>>,
        locked: Arc<AtomicBool>,
    }

    impl Drop for MemorySession {
        fn drop(&mut self) {
            self.locked.store(false, Ordering::SeqCst);
        }
    }

    impl LabStoreSession for MemorySession {
        fn managed_lock_active(&self) -> Result<bool, LabError> {
            Ok(self.data.lock().expect("memory store").managed_lock_active)
        }

        fn load_owner(&self) -> Result<Option<LabOwner>, LabError> {
            Ok(self.data.lock().expect("memory store").owner.clone())
        }

        fn save_owner(&mut self, owner: &LabOwner) -> Result<(), LabError> {
            let mut data = self.data.lock().expect("memory store");
            if data.owner.is_some() {
                return Err(LabError::Unsafe("owner exists".to_string()));
            }
            data.owner = Some(owner.clone());
            Ok(())
        }

        fn list_deployments(&self) -> Result<Vec<LabDeployment>, LabError> {
            Ok(self
                .data
                .lock()
                .expect("memory store")
                .deployments
                .values()
                .cloned()
                .collect())
        }

        fn save_deployment(&mut self, deployment: &LabDeployment) -> Result<(), LabError> {
            self.data
                .lock()
                .expect("memory store")
                .deployments
                .insert(deployment.lab_id.clone(), deployment.clone());
            Ok(())
        }

        fn remove_deployment(&mut self, lab_id: &str) -> Result<(), LabError> {
            self.data
                .lock()
                .expect("memory store")
                .deployments
                .remove(lab_id);
            Ok(())
        }

        fn cache_path(&self, lab_id: &str) -> Result<PathBuf, LabError> {
            Ok(PathBuf::from(format!("/cache/{lab_id}")))
        }

        fn prepare_cache(&mut self, lab_id: &str) -> Result<PathBuf, LabError> {
            self.data
                .lock()
                .expect("memory store")
                .caches
                .insert(lab_id.to_string());
            self.cache_path(lab_id)
        }

        fn list_cache_ids(&self) -> Result<Vec<String>, LabError> {
            Ok(self
                .data
                .lock()
                .expect("memory store")
                .caches
                .iter()
                .cloned()
                .collect())
        }

        fn remove_cache(&mut self, lab_id: &str) -> Result<(), LabError> {
            self.data
                .lock()
                .expect("memory store")
                .caches
                .remove(lab_id);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeRuntimeState {
        containers: BTreeMap<String, ContainerDetails>,
        create_calls: usize,
        remove_calls: usize,
        logs: VecDeque<RuntimeLogLine>,
        exit_on_start: bool,
    }

    #[derive(Default)]
    struct FakeRuntime {
        state: Arc<Mutex<FakeRuntimeState>>,
    }

    impl FakeRuntime {
        fn mutate_labels(&self, id: &str) {
            self.state
                .lock()
                .expect("runtime")
                .containers
                .get_mut(id)
                .expect("container")
                .labels
                .insert(LABEL_OWNER_ID.to_string(), "other-owner".to_string());
        }
    }

    impl ContainerRuntime for FakeRuntime {
        fn daemon_info(&self) -> RuntimeFuture<'_, DockerDaemonInfo> {
            Box::pin(async { unreachable!("not used") })
        }

        fn inspect_image<'a>(&'a self, image: &'a str) -> RuntimeFuture<'a, ImageIdentity> {
            Box::pin(async move {
                Ok(ImageIdentity {
                    id: format!("sha256:{}", digest(image).expect("digest")),
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
                    .expect("runtime")
                    .containers
                    .values()
                    .filter(|details| {
                        labels
                            .iter()
                            .all(|(key, value)| details.labels.get(key) == Some(value))
                    })
                    .map(|details| ContainerListEntry {
                        id: details.id.clone(),
                        names: details.name.clone().into_iter().collect(),
                        image: None,
                        image_id: details.image_id.clone(),
                        state: details.state,
                        status: None,
                        labels: details.labels.clone(),
                    })
                    .collect())
            })
        }

        fn create_container<'a>(
            &'a self,
            spec: &'a ContainerSpec,
        ) -> RuntimeFuture<'a, ContainerCreateResult> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("runtime");
                state.create_calls += 1;
                let id = format!("container-{}", state.create_calls);
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
                let mut state = self.state.lock().expect("runtime");
                let exit_on_start = state.exit_on_start;
                let container = state.containers.get_mut(id).ok_or(RuntimeError::NotFound {
                    operation: "container start",
                })?;
                if exit_on_start {
                    container.state = ContainerState::Exited;
                    container.exit_code = Some(7);
                    container.error = Some("backend initialization failed".to_string());
                } else {
                    container.state = ContainerState::Running;
                }
                Ok(())
            })
        }

        fn inspect_container<'a>(&'a self, id: &'a str) -> RuntimeFuture<'a, ContainerDetails> {
            Box::pin(async move {
                self.state
                    .lock()
                    .expect("runtime")
                    .containers
                    .get(id)
                    .cloned()
                    .ok_or(RuntimeError::NotFound {
                        operation: "container inspect",
                    })
            })
        }

        fn stop_container<'a>(&'a self, id: &'a str, _timeout_secs: u64) -> RuntimeFuture<'a, ()> {
            Box::pin(async move {
                self.state
                    .lock()
                    .expect("runtime")
                    .containers
                    .get_mut(id)
                    .ok_or(RuntimeError::NotFound {
                        operation: "container stop",
                    })?
                    .state = ContainerState::Exited;
                Ok(())
            })
        }

        fn remove_container<'a>(&'a self, id: &'a str, _force: bool) -> RuntimeFuture<'a, ()> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("runtime");
                state.containers.remove(id).ok_or(RuntimeError::NotFound {
                    operation: "container remove",
                })?;
                state.remove_calls += 1;
                Ok(())
            })
        }

        fn logs<'a>(&'a self, _id: &'a str, _options: ContainerLogOptions) -> RuntimeLogStream<'a> {
            let lines = self
                .state
                .lock()
                .expect("runtime")
                .logs
                .drain(..)
                .map(Ok)
                .collect::<Vec<_>>();
            Box::pin(stream::iter(lines))
        }
    }

    struct FakePath;

    impl ModelPathResolver for FakePath {
        fn resolve(
            &self,
            _roots: &[PathBuf],
            requested: &std::path::Path,
        ) -> Result<PathBuf, LabError> {
            Ok(requested.to_path_buf())
        }
    }

    struct FakeGpu {
        processes: usize,
    }

    impl LabGpuProbe for FakeGpu {
        fn resolve(&self, _selector: &str) -> Result<LabGpu, LabError> {
            Ok(LabGpu {
                host_index: 0,
                uuid: "GPU-example".to_string(),
                running_compute_processes: self.processes,
            })
        }
    }

    #[derive(Default)]
    struct FakePorts {
        busy: BTreeSet<u16>,
    }

    impl PortProbe for FakePorts {
        fn is_available(&self, _host: &str, port: u16) -> Result<bool, LabError> {
            Ok(!self.busy.contains(&port))
        }
    }

    struct FakeHttp {
        models: Vec<String>,
    }

    impl LabHttpClient for FakeHttp {
        fn models<'a>(&'a self, _endpoint: &'a str) -> LabFuture<'a, Vec<String>> {
            Box::pin(async move { Ok(self.models.clone()) })
        }

        fn infer<'a>(
            &'a self,
            _endpoint: &'a str,
            _model: &'a str,
            _request: &'a LabInferRequest,
        ) -> LabFuture<'a, serde_json::Value> {
            Box::pin(async { Ok(json!({"choices": [{"text": "ok"}]})) })
        }
    }

    #[derive(Default)]
    struct FakeClock(AtomicU64);

    impl LabClock for FakeClock {
        fn now_unix(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }

        fn sleep(&self, duration: Duration) -> ClockFuture<'_> {
            self.0.fetch_add(duration.as_secs(), Ordering::SeqCst);
            Box::pin(async {})
        }
    }

    struct FakeIds(Mutex<VecDeque<String>>);

    impl LabIdSource for FakeIds {
        fn next_id(&self) -> Result<String, LabError> {
            self.0
                .lock()
                .expect("IDs")
                .pop_front()
                .ok_or_else(|| LabError::Invalid("no fake ID".to_string()))
        }
    }

    struct Fixture {
        runtime: FakeRuntime,
        store: MemoryStore,
        path: FakePath,
        gpu: FakeGpu,
        ports: FakePorts,
        http: FakeHttp,
        clock: FakeClock,
        ids: FakeIds,
        roots: Vec<PathBuf>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                runtime: FakeRuntime::default(),
                store: MemoryStore::default(),
                path: FakePath,
                gpu: FakeGpu { processes: 0 },
                ports: FakePorts::default(),
                http: FakeHttp {
                    models: vec!["demo".to_string()],
                },
                clock: FakeClock::default(),
                ids: FakeIds(Mutex::new(VecDeque::from([
                    "owner-a".to_string(),
                    "lab-a".to_string(),
                    "lab-b".to_string(),
                ]))),
                roots: vec![PathBuf::from("/models")],
            }
        }

        fn service(&self) -> LabService<'_> {
            LabService::new(
                &self.runtime,
                &self.store,
                &self.path,
                &self.gpu,
                &self.ports,
                &self.http,
                &self.clock,
                &self.ids,
                &self.roots,
            )
        }
    }

    fn request() -> LabDeployRequest {
        LabDeployRequest {
            name: "demo".to_string(),
            model_path: PathBuf::from("/models/demo"),
            image: IMAGE.to_string(),
            gpu: "0".to_string(),
            port: None,
            served_model_name: None,
            vllm: VllmOptions::default(),
            environment: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn deploy_waits_for_expected_model_and_is_idempotent() {
        let fixture = Fixture::new();

        let first = fixture.service().deploy(request()).await.expect("deploy");
        let second = fixture
            .service()
            .deploy(request())
            .await
            .expect("idempotent deploy");

        assert_eq!(first.state, LabState::Running);
        assert_eq!(second.lab_id, first.lab_id);
        assert_eq!(
            fixture.runtime.state.lock().expect("runtime").create_calls,
            1
        );
    }

    #[tokio::test]
    async fn same_name_with_a_different_spec_is_rejected() {
        let fixture = Fixture::new();
        fixture.service().deploy(request()).await.expect("deploy");
        let mut changed = request();
        changed.port = Some(42_001);

        let error = fixture
            .service()
            .deploy(changed)
            .await
            .expect_err("spec conflict");

        assert!(matches!(error, LabError::Conflict(_)));
        assert_eq!(
            fixture.runtime.state.lock().expect("runtime").create_calls,
            1
        );
    }

    #[tokio::test]
    async fn stopped_workload_restarts_and_missing_container_is_recreated() {
        let fixture = Fixture::new();
        let first = fixture.service().deploy(request()).await.expect("deploy");
        fixture.service().stop("demo").await.expect("stop");

        let restarted = fixture.service().deploy(request()).await.expect("restart");
        assert_eq!(restarted.lab_id, first.lab_id);
        assert_eq!(
            fixture.runtime.state.lock().expect("runtime").create_calls,
            1
        );
        fixture
            .runtime
            .state
            .lock()
            .expect("runtime")
            .containers
            .remove(restarted.container_id.as_deref().expect("container ID"));

        let recreated = fixture.service().deploy(request()).await.expect("recreate");

        assert_eq!(recreated.lab_id, first.lab_id);
        assert_ne!(recreated.container_id, restarted.container_id);
        assert_eq!(
            fixture.runtime.state.lock().expect("runtime").create_calls,
            2
        );
    }

    #[tokio::test]
    async fn automatic_port_allocation_skips_a_busy_loopback_port() {
        let mut fixture = Fixture::new();
        fixture.ports.busy.insert(LAB_PORT_START);

        let status = fixture.service().deploy(request()).await.expect("deploy");

        assert_eq!(status.endpoint, "http://127.0.0.1:42001");
    }

    #[tokio::test]
    async fn ownership_mismatch_blocks_mutation() {
        let fixture = Fixture::new();
        let deployed = fixture.service().deploy(request()).await.expect("deploy");
        fixture
            .runtime
            .mutate_labels(deployed.container_id.as_deref().expect("container ID"));

        let error = fixture
            .service()
            .stop("demo")
            .await
            .expect_err("ownership must fail");

        assert!(matches!(error, LabError::Unsafe(_)));
    }

    #[tokio::test]
    async fn stop_remove_and_cleanup_preserve_the_two_step_safety_boundary() {
        let fixture = Fixture::new();
        fixture.service().deploy(request()).await.expect("deploy");
        assert!(matches!(
            fixture.service().remove("demo").await,
            Err(LabError::Conflict(_))
        ));
        fixture.service().stop("demo").await.expect("stop");
        let dry_run = fixture.service().cleanup(false).await.expect("dry run");
        assert!(dry_run.dry_run);
        assert_eq!(dry_run.container_ids.len(), 1);
        assert_eq!(
            fixture.runtime.state.lock().expect("runtime").remove_calls,
            0
        );

        let executed = fixture.service().cleanup(true).await.expect("cleanup");

        assert_eq!(executed.removed_containers, 1);
        assert_eq!(executed.removed_deployments, 1);
        assert_eq!(executed.removed_caches, 1);
    }

    #[tokio::test]
    async fn inference_and_logs_use_only_the_owned_loopback_deployment() {
        let fixture = Fixture::new();
        fixture.service().deploy(request()).await.expect("deploy");
        fixture
            .runtime
            .state
            .lock()
            .expect("runtime")
            .logs
            .push_back(RuntimeLogLine {
                stream: RuntimeLogStreamKind::Stdout,
                message: b"ready\n".to_vec(),
            });
        let logs = fixture
            .service()
            .logs("demo", false, 0, "10".to_string())
            .await
            .expect("logs");
        let inferred = fixture
            .service()
            .infer(&LabInferRequest {
                name: "demo".to_string(),
                kind: LabInferenceKind::Completion,
                prompt: "hello".to_string(),
                max_tokens: Some(8),
            })
            .await
            .expect("infer");

        assert_eq!(logs[0].message, "ready\n");
        assert_eq!(inferred.mode, LAB_MODE);
        assert!(inferred.endpoint.starts_with("http://127.0.0.1:"));
    }

    #[tokio::test]
    async fn active_compute_processes_block_a_new_deployment_before_create() {
        let mut fixture = Fixture::new();
        fixture.gpu.processes = 2;

        let error = fixture
            .service()
            .deploy(request())
            .await
            .expect_err("busy GPU");

        assert!(matches!(error, LabError::Conflict(_)));
        assert_eq!(
            fixture.runtime.state.lock().expect("runtime").create_calls,
            0
        );
    }

    #[tokio::test]
    async fn active_managed_lock_blocks_lab_before_owner_or_container_creation() {
        let fixture = Fixture::new();
        fixture
            .store
            .data
            .lock()
            .expect("memory store")
            .managed_lock_active = true;

        let error = fixture
            .service()
            .deploy(request())
            .await
            .expect_err("Managed lock must block Lab");

        assert!(matches!(error, LabError::Conflict(_)));
        let data = fixture.store.data.lock().expect("memory store");
        assert!(data.owner.is_none());
        assert!(data.deployments.is_empty());
        assert_eq!(
            fixture.runtime.state.lock().expect("runtime").create_calls,
            0
        );
    }

    #[tokio::test]
    async fn cleanup_dry_run_does_not_create_an_owner() {
        let fixture = Fixture::new();

        let plan = fixture.service().cleanup(false).await.expect("dry run");

        assert!(plan.dry_run);
        assert!(plan.deployment_ids.is_empty());
        assert!(
            fixture
                .store
                .data
                .lock()
                .expect("memory store")
                .owner
                .is_none()
        );
    }

    #[tokio::test]
    async fn cleanup_refuses_a_failed_manifest_while_its_container_is_running() {
        let fixture = Fixture::new();
        fixture.service().deploy(request()).await.expect("deploy");
        fixture
            .store
            .data
            .lock()
            .expect("memory store")
            .deployments
            .values_mut()
            .next()
            .expect("deployment")
            .state = LabState::Failed;

        let plan = fixture.service().cleanup(false).await.expect("dry run");
        assert!(
            plan.blocked
                .iter()
                .any(|reason| reason.contains("still running"))
        );
        assert!(matches!(
            fixture.service().cleanup(true).await,
            Err(LabError::Unsafe(_))
        ));
        assert_eq!(
            fixture.runtime.state.lock().expect("runtime").remove_calls,
            0
        );
    }

    #[tokio::test]
    async fn failed_start_preserves_container_and_bounded_diagnostic_in_manifest() {
        let fixture = Fixture::new();
        {
            let mut runtime = fixture.runtime.state.lock().expect("runtime");
            runtime.exit_on_start = true;
            runtime.logs.push_back(RuntimeLogLine {
                stream: RuntimeLogStreamKind::Stderr,
                message: b"model load failed\n".to_vec(),
            });
        }

        let error = fixture
            .service()
            .deploy(request())
            .await
            .expect_err("start must fail");

        assert!(matches!(error, LabError::DeploymentFailed { .. }));
        let deployments = fixture
            .store
            .data
            .lock()
            .expect("memory store")
            .deployments
            .values()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(deployments[0].state, LabState::Failed);
        assert!(deployments[0].container_id.is_some());
        let diagnostic = deployments[0].last_error.as_deref().expect("diagnostic");
        assert!(diagnostic.contains("exit code 7"));
        assert!(diagnostic.contains("model load failed"));
        assert!(diagnostic.len() <= 512);
    }

    #[test]
    fn fake_boundaries_are_sendable_without_real_resources() {
        fn assert_send<T: Send>() {}
        fn assert_future<T: Future>() {}
        assert_send::<MemoryStore>();
        assert_future::<Pin<Box<dyn Future<Output = ()> + Send>>>();
    }
}
