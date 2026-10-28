//! Offline planning and ownership discovery for Server-managed vLLM workloads.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use gpustack_client::types::{ModelInstancePublic, ModelInstanceUpdate, ModelPublic};
use gpustack_runtime::docker::ContainerRuntime;
use thiserror::Error;

use crate::runtime_core::{
    OwnedContainer, RuntimeCoreError, discover_owned_container, resolve_immutable_image,
};
use crate::workload::{
    LABEL_MODEL_INSTANCE_ID, VllmDtype, VllmOptions, WorkloadBuildRequest, WorkloadError,
    WorkloadGpu, WorkloadMode, WorkloadSpecV2,
};

use super::{LocalModelResolver, MANAGED_PORT_END, MANAGED_PORT_START, ResolveError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedGpu {
    pub host_index: u32,
    pub uuid: String,
    pub running_compute_processes: usize,
}

pub struct ManagedWorkloadPlanner {
    worker_id: i64,
    worker_uuid: String,
    worker_ip: IpAddr,
    resolver: LocalModelResolver,
    cache_root: PathBuf,
    container_user: String,
    gpus: Vec<ManagedGpu>,
}

impl ManagedWorkloadPlanner {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        worker_id: i64,
        worker_uuid: String,
        worker_ip: IpAddr,
        resolver: LocalModelResolver,
        cache_root: PathBuf,
        container_user: String,
        gpus: Vec<ManagedGpu>,
    ) -> Self {
        Self {
            worker_id,
            worker_uuid,
            worker_ip,
            resolver,
            cache_root,
            container_user,
            gpus,
        }
    }

    pub fn plan(
        &self,
        model: &ModelPublic,
        instance: &ModelInstancePublic,
    ) -> Result<WorkloadSpecV2, ManagedRuntimePlanError> {
        self.validate_model_and_instance(model, instance)?;
        let resolved_path = instance
            .resolved_path
            .as_deref()
            .ok_or_else(|| contract("STARTING ModelInstance requires a non-empty resolved_path"))?;
        let resolved = self.resolver.resolve(Path::new(resolved_path))?;
        let canonical_path = fs::canonicalize(resolved_path)
            .map_err(|error| contract(format!("cannot canonicalize resolved_path: {error}")))?;
        if !resolved.paths.iter().any(|path| path == &canonical_path) {
            return Err(contract(
                "resolved_path is not one of the canonical local model artifacts",
            ));
        }

        let gpu_index = instance
            .gpu_indexes
            .as_deref()
            .and_then(|indexes| indexes.first().copied())
            .ok_or_else(|| contract("Managed R1 requires exactly one GPU index"))?;
        let gpu = self
            .gpus
            .iter()
            .find(|gpu| gpu.host_index == gpu_index)
            .ok_or_else(|| contract(format!("GPU index {gpu_index} is absent from inventory")))?;
        if gpu.running_compute_processes != 0 {
            return Err(contract(format!(
                "GPU index {gpu_index} has active compute processes"
            )));
        }
        if !gpu.uuid.starts_with("GPU-") {
            return Err(contract("Managed GPU must have a stable NVIDIA UUID"));
        }

        let port = instance
            .port
            .ok_or_else(|| contract("STARTING ModelInstance requires an assigned port"))?;
        if !(MANAGED_PORT_START..=MANAGED_PORT_END).contains(&port)
            || instance.ports.as_deref() != Some(&[port])
        {
            return Err(contract(
                "Managed port and ports must identify one port in 40000-40063",
            ));
        }

        let image = model
            .image_name
            .as_deref()
            .ok_or_else(|| contract("Managed Model requires a Runner image"))?;
        require_immutable_digest(image)?;
        let vllm = parse_vllm_parameters(model.backend_parameters.as_deref())?;

        WorkloadSpecV2::build(WorkloadBuildRequest {
            mode: WorkloadMode::Managed,
            owner_id: self.worker_uuid.clone(),
            deployment_id: instance.id.to_string(),
            managed_worker_uuid: Some(self.worker_uuid.clone()),
            model_path: canonical_path,
            cache_path: self.cache_root.join(instance.id.to_string()),
            served_model_name: model.name.clone(),
            requested_image: image.to_string(),
            resolved_image_digest: image.to_string(),
            gpus: vec![WorkloadGpu {
                uuid: gpu.uuid.clone(),
                host_index: gpu.host_index,
            }],
            service_host: self.worker_ip.to_string(),
            service_port: port,
            tensor_parallel_size: 1,
            container_user: self.container_user.clone(),
            vllm,
            environment: BTreeMap::new(),
        })
        .map_err(Into::into)
    }

    pub async fn discover(
        &self,
        runtime: &dyn ContainerRuntime,
        spec: &WorkloadSpecV2,
    ) -> Result<OwnedContainer, ManagedRuntimePlanError> {
        if spec.mode != WorkloadMode::Managed {
            return Err(contract(
                "Managed discovery requires a Managed workload spec",
            ));
        }
        // The broad instance-only query exposes partially labelled conflicts before mutation.
        let filters = BTreeMap::from([(
            LABEL_MODEL_INSTANCE_ID.to_string(),
            spec.deployment_id.clone(),
        )]);
        discover_owned_container(runtime, &filters, spec)
            .await
            .map_err(Into::into)
    }

    pub async fn verify_local_image(
        &self,
        runtime: &dyn ContainerRuntime,
        spec: &WorkloadSpecV2,
    ) -> Result<(), ManagedRuntimePlanError> {
        resolve_immutable_image(runtime, &spec.requested_image).await?;
        Ok(())
    }

    fn validate_model_and_instance(
        &self,
        model: &ModelPublic,
        instance: &ModelInstancePublic,
    ) -> Result<(), ManagedRuntimePlanError> {
        ModelInstanceUpdate::from(instance)
            .validate_for_worker(self.worker_id)
            .map_err(|error| contract(error.to_string()))?;
        if instance.state.0 != "starting" {
            return Err(contract(
                "Managed runtime planning requires authoritative STARTING state",
            ));
        }
        if instance.model_id != model.id
            || instance.model_name != model.name
            || model.source != instance.source
            || model.backend.as_deref() != Some("vLLM")
            || model.local_path != instance.local_path
        {
            return Err(contract(
                "Model and ModelInstance runtime inputs do not match",
            ));
        }
        let worker_ip = self.worker_ip.to_string();
        if instance.worker_ip.as_deref() != Some(worker_ip.as_str()) {
            return Err(contract(
                "ModelInstance Worker IP does not match this Worker",
            ));
        }
        if instance
            .gpu_indexes
            .as_ref()
            .is_none_or(|indexes| indexes.len() != 1)
        {
            return Err(contract("Managed R1 requires exactly one GPU index"));
        }
        if model
            .env
            .as_ref()
            .is_some_and(|environment| !environment.is_empty())
        {
            return Err(contract(
                "Managed R1 does not accept Model environment overrides",
            ));
        }
        if instance
            .injected_backend_parameters
            .as_ref()
            .is_some_and(|parameters| !parameters.is_empty())
        {
            return Err(contract(
                "Managed R1 does not accept pre-existing injected backend parameters",
            ));
        }
        Ok(())
    }
}

fn parse_vllm_parameters(
    parameters: Option<&[String]>,
) -> Result<VllmOptions, ManagedRuntimePlanError> {
    let mut options = VllmOptions::default();
    let mut seen = BTreeSet::new();
    for parameter in parameters.unwrap_or_default() {
        let parameter = parameter.trim();
        if parameter.is_empty()
            || parameter.contains(char::is_whitespace)
            || parameter.contains('\0')
        {
            return Err(contract(
                "backend parameters must be one non-empty argv token per entry",
            ));
        }
        let (flag, value) = parameter
            .split_once('=')
            .map_or((parameter, None), |(flag, value)| (flag, Some(value)));
        if !seen.insert(flag.to_string()) {
            return Err(contract(format!("duplicate backend parameter {flag}")));
        }
        match flag {
            "--tensor-parallel-size" => {
                if value != Some("1") {
                    return Err(contract("Managed R1 requires tensor parallel size 1"));
                }
            }
            "--max-model-len" => {
                options.max_model_len = Some(parse_positive_u32(flag, value)?);
            }
            "--gpu-memory-utilization" => {
                let value = required_value(flag, value)?
                    .parse::<f64>()
                    .map_err(|_| contract("GPU memory utilization must be numeric"))?;
                if !value.is_finite() || value <= 0.0 || value > 1.0 {
                    return Err(contract("GPU memory utilization must be in (0, 1]"));
                }
                options.gpu_memory_utilization = Some(value);
            }
            "--dtype" => {
                options.dtype = Some(match required_value(flag, value)? {
                    "auto" => VllmDtype::Auto,
                    "half" => VllmDtype::Half,
                    "float16" => VllmDtype::Float16,
                    "bfloat16" => VllmDtype::Bfloat16,
                    "float" => VllmDtype::Float,
                    "float32" => VllmDtype::Float32,
                    _ => return Err(contract("unsupported vLLM dtype")),
                });
            }
            "--trust-remote-code" => {
                require_flag(flag, value)?;
                options.trust_remote_code = true;
            }
            "--enforce-eager" => {
                require_flag(flag, value)?;
                options.enforce_eager = true;
            }
            _ => return Err(contract(format!("unsupported backend parameter {flag}"))),
        }
    }
    Ok(options)
}

fn parse_positive_u32(flag: &str, value: Option<&str>) -> Result<u32, ManagedRuntimePlanError> {
    let value = required_value(flag, value)?
        .parse::<u32>()
        .map_err(|_| contract(format!("{flag} must be a positive integer")))?;
    if value == 0 {
        return Err(contract(format!("{flag} must be a positive integer")));
    }
    Ok(value)
}

fn required_value<'a>(
    flag: &str,
    value: Option<&'a str>,
) -> Result<&'a str, ManagedRuntimePlanError> {
    value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| contract(format!("{flag} requires an inline value")))
}

fn require_flag(flag: &str, value: Option<&str>) -> Result<(), ManagedRuntimePlanError> {
    if value.is_some() {
        return Err(contract(format!("{flag} does not accept a value")));
    }
    Ok(())
}

fn require_immutable_digest(image: &str) -> Result<(), ManagedRuntimePlanError> {
    let digest = image.rsplit_once("@sha256:").map(|(_, digest)| digest);
    if digest.is_none_or(|digest| {
        digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        return Err(contract(
            "Managed Runner image must include an immutable SHA-256 digest",
        ));
    }
    Ok(())
}

fn contract(message: impl Into<String>) -> ManagedRuntimePlanError {
    ManagedRuntimePlanError::Contract(message.into())
}

#[derive(Debug, Error)]
pub enum ManagedRuntimePlanError {
    #[error("Managed runtime contract failed: {0}")]
    Contract(String),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Workload(#[from] WorkloadError),
    #[error(transparent)]
    RuntimeCore(#[from] RuntimeCoreError),
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};
    use std::sync::Mutex;

    use futures_util::stream;
    use gpustack_client::types::{ModelInstanceState, Paginated};
    use gpustack_runtime::docker::{
        ContainerCreateResult, ContainerDetails, ContainerListEntry, ContainerLogOptions,
        ContainerSpec, ContainerState, DockerDaemonInfo, ImageIdentity, RuntimeError,
        RuntimeFuture, RuntimeLogStream,
    };

    use super::*;
    use crate::workload::{LABEL_SPEC_HASH, VllmDtype};

    const IMAGE: &str =
        "runner@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    struct Fixture {
        _root: tempfile::TempDir,
        planner: ManagedWorkloadPlanner,
        model: ModelPublic,
        instance: ModelInstancePublic,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().expect("model root");
            let model_path = root.path().join("model");
            fs::create_dir(&model_path).expect("model directory");
            fs::write(model_path.join("config.json"), b"{}").expect("model config");
            let resolver = LocalModelResolver::new([root.path().to_path_buf()]).expect("resolver");
            let models: Paginated<ModelPublic> = serde_json::from_str(include_str!(
                "../../../../fixtures/server-v2.2.3/models-full-sync.response.json"
            ))
            .expect("model fixture");
            let instances: Paginated<ModelInstancePublic> = serde_json::from_str(include_str!(
                "../../../../fixtures/server-v2.2.3/model-instances-full-sync.response.json"
            ))
            .expect("instance fixture");
            let mut model = models.items.into_iter().next().expect("model");
            model.image_name = Some(IMAGE.to_string());
            model.backend_parameters = Some(vec![
                "--tensor-parallel-size=1".to_string(),
                "--max-model-len=2048".to_string(),
                "--gpu-memory-utilization=0.5".to_string(),
                "--dtype=auto".to_string(),
                "--enforce-eager".to_string(),
            ]);
            model.local_path = Some(model_path.display().to_string());
            let mut instance = instances.items.into_iter().next().expect("instance");
            instance.state = ModelInstanceState("starting".to_string());
            instance.port = Some(MANAGED_PORT_START);
            instance.ports = Some(vec![MANAGED_PORT_START]);
            instance.local_path = model.local_path.clone();
            instance.resolved_path = model.local_path.clone();
            let planner = ManagedWorkloadPlanner::new(
                101,
                "worker-uuid".to_string(),
                "192.0.2.10".parse().expect("worker IP"),
                resolver,
                root.path().join("cache"),
                "1004:1004".to_string(),
                vec![ManagedGpu {
                    host_index: 0,
                    uuid: "GPU-example".to_string(),
                    running_compute_processes: 0,
                }],
            );
            Self {
                _root: root,
                planner,
                model,
                instance,
            }
        }

        fn spec(&self) -> WorkloadSpecV2 {
            self.planner
                .plan(&self.model, &self.instance)
                .expect("managed plan")
        }
    }

    #[derive(Default)]
    struct FakeRuntime {
        image: Option<ImageIdentity>,
        listed: Vec<ContainerListEntry>,
        inspected: BTreeMap<String, ContainerDetails>,
        filters: Mutex<VecDeque<BTreeMap<String, String>>>,
    }

    impl ContainerRuntime for FakeRuntime {
        fn daemon_info(&self) -> RuntimeFuture<'_, DockerDaemonInfo> {
            Box::pin(async { Err(RuntimeError::Client("not used".to_string())) })
        }

        fn inspect_image<'a>(&'a self, _image: &'a str) -> RuntimeFuture<'a, ImageIdentity> {
            Box::pin(async move {
                self.image
                    .clone()
                    .ok_or_else(|| RuntimeError::Client("missing fake image".to_string()))
            })
        }

        fn list_containers<'a>(
            &'a self,
            labels: &'a BTreeMap<String, String>,
        ) -> RuntimeFuture<'a, Vec<ContainerListEntry>> {
            Box::pin(async move {
                self.filters
                    .lock()
                    .expect("filters")
                    .push_back(labels.clone());
                Ok(self.listed.clone())
            })
        }

        fn create_container<'a>(
            &'a self,
            _spec: &'a ContainerSpec,
        ) -> RuntimeFuture<'a, ContainerCreateResult> {
            Box::pin(async { Err(RuntimeError::Client("not used".to_string())) })
        }

        fn start_container<'a>(&'a self, _id: &'a str) -> RuntimeFuture<'a, ()> {
            Box::pin(async { Err(RuntimeError::Client("not used".to_string())) })
        }

        fn inspect_container<'a>(&'a self, id: &'a str) -> RuntimeFuture<'a, ContainerDetails> {
            Box::pin(async move {
                self.inspected
                    .get(id)
                    .cloned()
                    .ok_or(RuntimeError::NotFound {
                        operation: "container inspect",
                    })
            })
        }

        fn stop_container<'a>(&'a self, _id: &'a str, _timeout: u64) -> RuntimeFuture<'a, ()> {
            Box::pin(async { Err(RuntimeError::Client("not used".to_string())) })
        }

        fn remove_container<'a>(&'a self, _id: &'a str, _force: bool) -> RuntimeFuture<'a, ()> {
            Box::pin(async { Err(RuntimeError::Client("not used".to_string())) })
        }

        fn logs<'a>(&'a self, _id: &'a str, _options: ContainerLogOptions) -> RuntimeLogStream<'a> {
            Box::pin(stream::empty())
        }
    }

    fn owned_runtime(spec: &WorkloadSpecV2) -> FakeRuntime {
        let details = ContainerDetails {
            id: "container-1".to_string(),
            name: Some(spec.container_name.clone()),
            image_id: Some(format!("sha256:{}", "a".repeat(64))),
            state: ContainerState::Running,
            exit_code: None,
            error: None,
            oom_killed: false,
            labels: spec.labels.clone(),
        };
        FakeRuntime {
            listed: vec![ContainerListEntry {
                id: details.id.clone(),
                names: vec![spec.container_name.clone()],
                image: Some(spec.requested_image.clone()),
                image_id: details.image_id.clone(),
                state: details.state,
                status: None,
                labels: details.labels.clone(),
            }],
            inspected: BTreeMap::from([(details.id.clone(), details)]),
            ..Default::default()
        }
    }

    #[test]
    fn planner_normalizes_the_frozen_single_gpu_contract() {
        let fixture = Fixture::new();
        let spec = fixture.spec();

        assert_eq!(spec.mode, WorkloadMode::Managed);
        assert_eq!(spec.service_port, MANAGED_PORT_START);
        assert_eq!(spec.gpus[0].uuid, "GPU-example");
        assert_eq!(spec.vllm.max_model_len, Some(2048));
        assert_eq!(spec.vllm.gpu_memory_utilization, Some(0.5));
        assert_eq!(spec.vllm.dtype, Some(VllmDtype::Auto));
        assert!(spec.vllm.enforce_eager);
        assert!(spec.vllm.extra_args.is_empty());
        assert_eq!(
            spec.labels.get(LABEL_MODEL_INSTANCE_ID).map(String::as_str),
            Some("301")
        );
        assert_eq!(spec.spec_hash.len(), 64);
    }

    #[test]
    fn planner_rejects_unknown_parameters_and_assignment_drift() {
        let mut fixture = Fixture::new();
        fixture
            .model
            .backend_parameters
            .as_mut()
            .expect("parameters")
            .push("--api-key=secret".to_string());
        assert!(
            fixture
                .planner
                .plan(&fixture.model, &fixture.instance)
                .expect_err("unknown flag")
                .to_string()
                .contains("unsupported backend parameter")
        );

        fixture.model.backend_parameters = Some(vec!["--tensor-parallel-size=1".to_string()]);
        fixture.instance.worker_id = Some(999);
        assert!(
            fixture
                .planner
                .plan(&fixture.model, &fixture.instance)
                .expect_err("assignment drift")
                .to_string()
                .contains("assigned to worker")
        );
    }

    #[test]
    fn planner_rejects_paths_outside_the_allowed_roots_and_busy_gpu() {
        let mut fixture = Fixture::new();
        let outside = tempfile::tempdir().expect("outside");
        fixture.instance.resolved_path = Some(outside.path().display().to_string());
        assert!(
            fixture
                .planner
                .plan(&fixture.model, &fixture.instance)
                .expect_err("outside root")
                .to_string()
                .contains("escapes")
        );

        let mut fixture = Fixture::new();
        fixture.planner.gpus[0].running_compute_processes = 1;
        assert!(
            fixture
                .planner
                .plan(&fixture.model, &fixture.instance)
                .expect_err("busy GPU")
                .to_string()
                .contains("active compute")
        );
    }

    #[tokio::test]
    async fn discovery_is_missing_or_idempotently_adopts_one_exact_container() {
        let fixture = Fixture::new();
        let spec = fixture.spec();
        let missing = fixture
            .planner
            .discover(&FakeRuntime::default(), &spec)
            .await
            .expect("missing discovery");
        assert_eq!(missing, OwnedContainer::Missing);

        let runtime = owned_runtime(&spec);
        let adopted = fixture
            .planner
            .discover(&runtime, &spec)
            .await
            .expect("adopt exact container");
        assert!(matches!(adopted, OwnedContainer::Present(_)));
        assert!(matches!(
            fixture
                .planner
                .discover(&runtime, &spec)
                .await
                .expect("repeat adoption"),
            OwnedContainer::Present(_)
        ));
        let filters = runtime.filters.lock().expect("filters");
        assert_eq!(
            filters[0].get(LABEL_MODEL_INSTANCE_ID).map(String::as_str),
            Some("301")
        );
        assert_eq!(filters[0].len(), 1);
    }

    #[tokio::test]
    async fn discovery_fails_closed_for_multiple_or_incomplete_ownership() {
        let fixture = Fixture::new();
        let spec = fixture.spec();
        let mut multiple = owned_runtime(&spec);
        multiple.listed.push(multiple.listed[0].clone());
        assert!(
            fixture
                .planner
                .discover(&multiple, &spec)
                .await
                .expect_err("multiple containers")
                .to_string()
                .contains("same workload identity")
        );

        let mut incomplete = owned_runtime(&spec);
        incomplete
            .inspected
            .get_mut("container-1")
            .expect("details")
            .labels
            .remove(LABEL_SPEC_HASH);
        assert!(
            fixture
                .planner
                .discover(&incomplete, &spec)
                .await
                .expect_err("missing spec hash")
                .to_string()
                .contains(LABEL_SPEC_HASH)
        );

        let mut conflicting = owned_runtime(&spec);
        conflicting
            .inspected
            .get_mut("container-1")
            .expect("details")
            .labels
            .insert(LABEL_SPEC_HASH.to_string(), "b".repeat(64));
        assert!(
            fixture
                .planner
                .discover(&conflicting, &spec)
                .await
                .expect_err("conflicting spec hash")
                .to_string()
                .contains("expected")
        );
    }

    #[tokio::test]
    async fn local_image_identity_must_match_the_frozen_digest() {
        let fixture = Fixture::new();
        let spec = fixture.spec();
        let runtime = FakeRuntime {
            image: Some(ImageIdentity {
                id: format!("sha256:{}", "a".repeat(64)),
                repo_digests: vec![IMAGE.to_string()],
                architecture: Some("amd64".to_string()),
                os: Some("linux".to_string()),
            }),
            ..Default::default()
        };
        fixture
            .planner
            .verify_local_image(&runtime, &spec)
            .await
            .expect("matching image");

        let wrong = FakeRuntime {
            image: Some(ImageIdentity {
                id: format!("sha256:{}", "b".repeat(64)),
                repo_digests: vec![],
                architecture: Some("amd64".to_string()),
                os: Some("linux".to_string()),
            }),
            ..Default::default()
        };
        assert!(
            fixture
                .planner
                .verify_local_image(&wrong, &spec)
                .await
                .expect_err("mismatched image")
                .to_string()
                .contains("identity")
        );
    }
}
