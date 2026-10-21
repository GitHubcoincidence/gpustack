use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gpustack_runtime::docker::{
    BindMountSpec, ContainerRestartPolicy, ContainerSpec, GpuDeviceRequest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const WORKLOAD_SCHEMA_VERSION: u8 = 1;
pub const LABEL_IMPLEMENTATION: &str = "io.gpustack.implementation";
pub const LABEL_SCHEMA_VERSION: &str = "io.gpustack.schema-version";
pub const LABEL_MODE: &str = "io.gpustack.mode";
pub const LABEL_OWNER_ID: &str = "io.gpustack.owner-id";
pub const LABEL_COMPONENT: &str = "io.gpustack.component";
pub const LABEL_SPEC_HASH: &str = "io.gpustack.spec-hash";
pub const LABEL_LAB_ID: &str = "io.gpustack.lab-id";
pub const LABEL_WORKER_UUID: &str = "io.gpustack.worker-uuid";
pub const LABEL_MODEL_INSTANCE_ID: &str = "io.gpustack.model-instance-id";

const LAB_BIND_ADDRESS: &str = "127.0.0.1";
const NETWORK_MODE: &str = "host";
const SHM_SIZE_BYTES: u64 = 10 * 1024 * 1024 * 1024;
const HEALTH_PATH: &str = "/v1/models";
const HEALTH_INTERVAL_SECS: u64 = 2;
const HEALTH_DEADLINE_SECS: u64 = 600;
const STOP_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadMode {
    Lab,
    Managed,
}

impl WorkloadMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lab => "lab",
            Self::Managed => "managed",
        }
    }

    fn container_prefix(self) -> &'static str {
        match self {
            Self::Lab => "gpustack-rs-lab",
            Self::Managed => "gpustack-rs-managed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadGpu {
    pub uuid: String,
    pub host_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadMount {
    pub source: String,
    pub target: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadHealthCheck {
    pub path: String,
    pub interval_secs: u64,
    pub deadline_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VllmDtype {
    Auto,
    Half,
    Float16,
    Bfloat16,
    Float,
    Float32,
}

impl VllmDtype {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Half => "half",
            Self::Float16 => "float16",
            Self::Bfloat16 => "bfloat16",
            Self::Float => "float",
            Self::Float32 => "float32",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VllmExtraArg {
    pub flag: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VllmOptions {
    pub max_model_len: Option<u32>,
    pub gpu_memory_utilization: Option<f64>,
    pub dtype: Option<VllmDtype>,
    pub trust_remote_code: bool,
    pub enforce_eager: bool,
    pub extra_args: Vec<VllmExtraArg>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkloadSpecV1 {
    pub schema_version: u8,
    pub mode: WorkloadMode,
    pub owner_id: String,
    pub deployment_id: String,
    pub container_name: String,
    pub model_path: String,
    pub served_model_name: String,
    pub requested_image: String,
    pub resolved_image_digest: String,
    pub gpus: Vec<WorkloadGpu>,
    pub service_host: String,
    pub service_port: u16,
    pub tensor_parallel_size: u32,
    pub vllm: VllmOptions,
    pub environment: BTreeMap<String, String>,
    pub mounts: Vec<WorkloadMount>,
    pub shm_size_bytes: u64,
    pub network_mode: String,
    pub privileged: bool,
    pub security_options: Vec<String>,
    pub health: WorkloadHealthCheck,
    pub labels: BTreeMap<String, String>,
    pub spec_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkloadBuildRequest {
    pub mode: WorkloadMode,
    pub owner_id: String,
    pub deployment_id: String,
    pub managed_worker_uuid: Option<String>,
    pub model_path: PathBuf,
    pub cache_path: PathBuf,
    pub served_model_name: String,
    pub requested_image: String,
    pub resolved_image_digest: String,
    pub gpus: Vec<WorkloadGpu>,
    pub service_host: String,
    pub service_port: u16,
    pub tensor_parallel_size: u32,
    pub vllm: VllmOptions,
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Error)]
pub enum WorkloadError {
    #[error("invalid workload request: {0}")]
    Invalid(String),
    #[error("cannot serialize normalized workload: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OwnershipError {
    #[error("container is missing ownership label {label}")]
    Missing { label: String },
    #[error("container ownership label {label} is {actual}, expected {expected}")]
    Mismatch {
        label: String,
        expected: String,
        actual: String,
    },
}

impl WorkloadSpecV1 {
    pub fn build(request: WorkloadBuildRequest) -> Result<Self, WorkloadError> {
        validate_request(&request)?;

        let model_path = path_string(&request.model_path, "model path")?;
        let cache_path = path_string(&request.cache_path, "cache path")?;
        let mut environment = request.environment;
        environment.insert("HF_HOME".to_string(), format!("{cache_path}/huggingface"));
        environment.insert("TORCH_HOME".to_string(), format!("{cache_path}/torch"));
        environment.insert("VLLM_CACHE_ROOT".to_string(), format!("{cache_path}/vllm"));
        environment.insert("XDG_CACHE_HOME".to_string(), format!("{cache_path}/xdg"));

        let mut mounts = vec![
            WorkloadMount {
                source: model_path.clone(),
                target: model_path.clone(),
                read_only: true,
            },
            WorkloadMount {
                source: cache_path.clone(),
                target: cache_path,
                read_only: false,
            },
        ];
        mounts.sort_by(|left, right| left.target.cmp(&right.target));

        let mut labels = BTreeMap::from([
            (LABEL_IMPLEMENTATION.to_string(), "rust".to_string()),
            (
                LABEL_SCHEMA_VERSION.to_string(),
                WORKLOAD_SCHEMA_VERSION.to_string(),
            ),
            (LABEL_MODE.to_string(), request.mode.as_str().to_string()),
            (LABEL_OWNER_ID.to_string(), request.owner_id.clone()),
            (LABEL_COMPONENT.to_string(), "vllm".to_string()),
        ]);
        match request.mode {
            WorkloadMode::Lab => {
                labels.insert(LABEL_LAB_ID.to_string(), request.deployment_id.clone());
            }
            WorkloadMode::Managed => {
                labels.insert(
                    LABEL_WORKER_UUID.to_string(),
                    request
                        .managed_worker_uuid
                        .clone()
                        .expect("managed UUID was validated"),
                );
                labels.insert(
                    LABEL_MODEL_INSTANCE_ID.to_string(),
                    request.deployment_id.clone(),
                );
            }
        }

        let mut spec = Self {
            schema_version: WORKLOAD_SCHEMA_VERSION,
            mode: request.mode,
            owner_id: request.owner_id,
            deployment_id: request.deployment_id.clone(),
            container_name: format!(
                "{}-{}",
                request.mode.container_prefix(),
                request.deployment_id
            ),
            model_path,
            served_model_name: request.served_model_name,
            requested_image: request.requested_image,
            resolved_image_digest: request.resolved_image_digest,
            gpus: request.gpus,
            service_host: request.service_host,
            service_port: request.service_port,
            tensor_parallel_size: request.tensor_parallel_size,
            vllm: request.vllm,
            environment,
            mounts,
            shm_size_bytes: SHM_SIZE_BYTES,
            network_mode: NETWORK_MODE.to_string(),
            privileged: false,
            security_options: vec!["no-new-privileges:true".to_string()],
            health: WorkloadHealthCheck {
                path: HEALTH_PATH.to_string(),
                interval_secs: HEALTH_INTERVAL_SECS,
                deadline_secs: HEALTH_DEADLINE_SECS,
            },
            labels,
            spec_hash: String::new(),
        };
        let spec_hash = spec.compute_spec_hash()?;
        spec.labels
            .insert(LABEL_SPEC_HASH.to_string(), spec_hash.clone());
        spec.spec_hash = spec_hash;
        Ok(spec)
    }

    pub fn vllm_command(&self) -> Vec<String> {
        let mut command = vec![
            "vllm".to_string(),
            "serve".to_string(),
            self.model_path.clone(),
            "--host".to_string(),
            self.service_host.clone(),
            "--port".to_string(),
            self.service_port.to_string(),
            "--served-model-name".to_string(),
            self.served_model_name.clone(),
            "--tensor-parallel-size".to_string(),
            self.tensor_parallel_size.to_string(),
        ];
        if let Some(value) = self.vllm.max_model_len {
            command.extend(["--max-model-len".to_string(), value.to_string()]);
        }
        if let Some(value) = self.vllm.gpu_memory_utilization {
            command.extend(["--gpu-memory-utilization".to_string(), value.to_string()]);
        }
        if let Some(value) = self.vllm.dtype {
            command.extend(["--dtype".to_string(), value.as_str().to_string()]);
        }
        if self.vllm.trust_remote_code {
            command.push("--trust-remote-code".to_string());
        }
        if self.vllm.enforce_eager {
            command.push("--enforce-eager".to_string());
        }
        for argument in &self.vllm.extra_args {
            command.push(argument.flag.clone());
            if let Some(value) = &argument.value {
                command.push(value.clone());
            }
        }
        command
    }

    pub fn container_spec(&self) -> ContainerSpec {
        ContainerSpec {
            name: self.container_name.clone(),
            image: self.resolved_image_digest.clone(),
            command: self.vllm_command(),
            environment: self.environment.clone(),
            labels: self.labels.clone(),
            mounts: self
                .mounts
                .iter()
                .map(|mount| BindMountSpec {
                    source: PathBuf::from(&mount.source),
                    target: PathBuf::from(&mount.target),
                    read_only: mount.read_only,
                })
                .collect(),
            network_mode: self.network_mode.clone(),
            shm_size_bytes: self.shm_size_bytes,
            privileged: self.privileged,
            security_options: self.security_options.clone(),
            gpu_request: Some(GpuDeviceRequest {
                driver: "nvidia".to_string(),
                device_ids: self.gpus.iter().map(|gpu| gpu.uuid.clone()).collect(),
                capabilities: vec![vec!["gpu".to_string()]],
            }),
            restart_policy: ContainerRestartPolicy::No,
            auto_remove: false,
            stop_timeout_secs: STOP_TIMEOUT_SECS,
        }
    }

    /// Mutations compare every identity-bearing label instead of trusting a name or local record.
    pub fn verify_ownership(
        &self,
        actual: &BTreeMap<String, String>,
    ) -> Result<(), OwnershipError> {
        for (label, expected) in &self.labels {
            let Some(actual) = actual.get(label) else {
                return Err(OwnershipError::Missing {
                    label: label.clone(),
                });
            };
            if actual != expected {
                return Err(OwnershipError::Mismatch {
                    label: label.clone(),
                    expected: expected.clone(),
                    actual: actual.clone(),
                });
            }
        }
        Ok(())
    }

    /// The hash input omits its own value so identity remains stable after the label is attached.
    fn compute_spec_hash(&self) -> Result<String, serde_json::Error> {
        let mut labels = self.labels.clone();
        labels.remove(LABEL_SPEC_HASH);
        let input = WorkloadHashInput {
            schema_version: self.schema_version,
            mode: self.mode,
            owner_id: &self.owner_id,
            deployment_id: &self.deployment_id,
            container_name: &self.container_name,
            model_path: &self.model_path,
            served_model_name: &self.served_model_name,
            requested_image: &self.requested_image,
            resolved_image_digest: &self.resolved_image_digest,
            gpus: &self.gpus,
            service_host: &self.service_host,
            service_port: self.service_port,
            tensor_parallel_size: self.tensor_parallel_size,
            vllm: &self.vllm,
            environment: &self.environment,
            mounts: &self.mounts,
            shm_size_bytes: self.shm_size_bytes,
            network_mode: &self.network_mode,
            privileged: self.privileged,
            security_options: &self.security_options,
            health: &self.health,
            labels,
        };
        let encoded = serde_json::to_vec(&input)?;
        Ok(hex_lower(&Sha256::digest(encoded)))
    }
}

#[derive(Serialize)]
struct WorkloadHashInput<'a> {
    schema_version: u8,
    mode: WorkloadMode,
    owner_id: &'a str,
    deployment_id: &'a str,
    container_name: &'a str,
    model_path: &'a str,
    served_model_name: &'a str,
    requested_image: &'a str,
    resolved_image_digest: &'a str,
    gpus: &'a [WorkloadGpu],
    service_host: &'a str,
    service_port: u16,
    tensor_parallel_size: u32,
    vllm: &'a VllmOptions,
    environment: &'a BTreeMap<String, String>,
    mounts: &'a [WorkloadMount],
    shm_size_bytes: u64,
    network_mode: &'a str,
    privileged: bool,
    security_options: &'a [String],
    health: &'a WorkloadHealthCheck,
    labels: BTreeMap<String, String>,
}

fn validate_request(request: &WorkloadBuildRequest) -> Result<(), WorkloadError> {
    validate_identifier("owner ID", &request.owner_id)?;
    validate_identifier("deployment ID", &request.deployment_id)?;
    if request.served_model_name.trim().is_empty() {
        return invalid("served model name must be non-empty");
    }
    validate_absolute_path("model path", &request.model_path)?;
    validate_absolute_path("cache path", &request.cache_path)?;
    if request.requested_image.trim().is_empty() {
        return invalid("requested image must be non-empty");
    }
    validate_digest(&request.resolved_image_digest)?;
    if request.gpus.len() != 1 || request.tensor_parallel_size != 1 {
        return invalid("M5 requires exactly one GPU and tensor parallel size 1");
    }
    if !request.gpus[0].uuid.starts_with("GPU-") {
        return invalid("GPU identity must use a stable NVIDIA UUID");
    }
    if request.service_port == 0 {
        return invalid("service port must be greater than zero");
    }
    match request.mode {
        WorkloadMode::Lab => {
            if request.service_host != LAB_BIND_ADDRESS {
                return invalid("Lab workloads must bind to 127.0.0.1");
            }
            if request.managed_worker_uuid.is_some() {
                return invalid("Lab workloads cannot carry a managed Worker UUID");
            }
        }
        WorkloadMode::Managed => {
            let Some(worker_uuid) = request.managed_worker_uuid.as_deref() else {
                return invalid("Managed workloads require a Worker UUID");
            };
            validate_identifier("Worker UUID", worker_uuid)?;
            if request.service_host.trim().is_empty() {
                return invalid("Managed workloads require a bind address");
            }
        }
    }
    if request.vllm.max_model_len == Some(0) {
        return invalid("maximum model length must be greater than zero");
    }
    if let Some(value) = request.vllm.gpu_memory_utilization
        && (!value.is_finite() || value <= 0.0 || value > 1.0)
    {
        return invalid("GPU memory utilization must be finite and in (0, 1]");
    }
    for (key, value) in &request.environment {
        if key.is_empty() || key.contains('=') || key.contains('\0') || value.contains('\0') {
            return invalid("environment entries must be non-empty and contain no NUL or '=' key");
        }
        if matches!(
            key.as_str(),
            "HF_HOME" | "TORCH_HOME" | "VLLM_CACHE_ROOT" | "XDG_CACHE_HOME"
        ) {
            return invalid("cache environment is controlled by the Worker");
        }
    }
    for argument in &request.vllm.extra_args {
        validate_extra_arg(argument)?;
    }
    Ok(())
}

fn validate_extra_arg(argument: &VllmExtraArg) -> Result<(), WorkloadError> {
    if !argument.flag.starts_with("--")
        || argument.flag.len() <= 2
        || argument.flag.contains('=')
        || argument.flag.chars().any(char::is_whitespace)
    {
        return invalid("extra vLLM arguments must use a standalone --flag");
    }
    const PROTECTED: &[&str] = &[
        "--host",
        "--port",
        "--served-model-name",
        "--tensor-parallel-size",
        "--pipeline-parallel-size",
        "--data-parallel-size",
        "--distributed-executor-backend",
        "--device",
        "--model",
        "--model-tag",
        "--download-dir",
        "--max-model-len",
        "--gpu-memory-utilization",
        "--dtype",
        "--trust-remote-code",
        "--enforce-eager",
        "--api-key",
        "--ssl-keyfile",
        "--ssl-certfile",
    ];
    if PROTECTED.contains(&argument.flag.as_str()) {
        return invalid("extra vLLM argument overrides a Worker-controlled option");
    }
    if argument
        .value
        .as_deref()
        .is_some_and(|value| value.starts_with("--") || value.contains('\0'))
    {
        return invalid("extra vLLM argument value cannot inject another flag or contain NUL");
    }
    Ok(())
}

fn validate_identifier(name: &str, value: &str) -> Result<(), WorkloadError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character))
    {
        return invalid(format!(
            "{name} must contain 1-64 ASCII letters, digits, '-', '_', or '.'"
        ));
    }
    Ok(())
}

fn validate_absolute_path(name: &str, path: &Path) -> Result<(), WorkloadError> {
    if !path.is_absolute() {
        return invalid(format!("{name} must be absolute"));
    }
    path_string(path, name).map(|_| ())
}

fn path_string(path: &Path, name: &str) -> Result<String, WorkloadError> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| WorkloadError::Invalid(format!("{name} must be valid UTF-8")))
}

fn validate_digest(value: &str) -> Result<(), WorkloadError> {
    let digest = value
        .rsplit_once("@sha256:")
        .map(|(_, digest)| digest)
        .or_else(|| value.strip_prefix("sha256:"));
    if digest.is_none_or(|digest| {
        digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        return invalid("resolved image must contain an immutable SHA-256 digest");
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, WorkloadError> {
    Err(WorkloadError::Invalid(message.into()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str =
        "runner@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn request() -> WorkloadBuildRequest {
        WorkloadBuildRequest {
            mode: WorkloadMode::Lab,
            owner_id: "owner-1".to_string(),
            deployment_id: "deployment-1".to_string(),
            managed_worker_uuid: None,
            model_path: PathBuf::from("/models/Qwen-7B"),
            cache_path: PathBuf::from("/state/cache/deployment-1"),
            served_model_name: "qwen-7b".to_string(),
            requested_image: "runner:fixed".to_string(),
            resolved_image_digest: DIGEST.to_string(),
            gpus: vec![WorkloadGpu {
                uuid: "GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3".to_string(),
                host_index: 0,
            }],
            service_host: LAB_BIND_ADDRESS.to_string(),
            service_port: 42000,
            tensor_parallel_size: 1,
            vllm: VllmOptions {
                max_model_len: Some(8192),
                gpu_memory_utilization: Some(0.8),
                dtype: Some(VllmDtype::Auto),
                trust_remote_code: true,
                enforce_eager: false,
                extra_args: vec![VllmExtraArg {
                    flag: "--disable-log-requests".to_string(),
                    value: None,
                }],
            },
            environment: BTreeMap::from([
                ("B_KEY".to_string(), "b".to_string()),
                ("A_KEY".to_string(), "a".to_string()),
            ]),
        }
    }

    #[test]
    fn normalized_hash_is_stable_and_attached_as_an_ownership_label() {
        let first = WorkloadSpecV1::build(request()).expect("build workload");
        let mut reversed = request();
        reversed.environment = BTreeMap::from([
            ("A_KEY".to_string(), "a".to_string()),
            ("B_KEY".to_string(), "b".to_string()),
        ]);
        let second = WorkloadSpecV1::build(reversed).expect("build workload");

        assert_eq!(first.spec_hash, second.spec_hash);
        assert_eq!(first.spec_hash.len(), 64);
        assert_eq!(first.labels.get(LABEL_SPEC_HASH), Some(&first.spec_hash));
    }

    #[test]
    fn identity_changes_change_the_spec_hash() {
        let first = WorkloadSpecV1::build(request()).expect("build workload");
        let mut changed = request();
        changed.service_port = 42001;
        let changed = WorkloadSpecV1::build(changed).expect("build changed workload");

        assert_ne!(first.spec_hash, changed.spec_hash);
    }

    #[test]
    fn spec_hash_does_not_hash_itself() {
        let mut spec = WorkloadSpecV1::build(request()).expect("build workload");
        let original = spec.spec_hash.clone();
        spec.labels
            .insert(LABEL_SPEC_HASH.to_string(), "not-the-real-hash".to_string());

        assert_eq!(spec.compute_spec_hash().expect("recompute hash"), original);
    }

    #[test]
    fn vllm_command_has_one_controlled_model_and_endpoint() {
        let spec = WorkloadSpecV1::build(request()).expect("build workload");

        assert_eq!(
            spec.vllm_command(),
            vec![
                "vllm",
                "serve",
                "/models/Qwen-7B",
                "--host",
                "127.0.0.1",
                "--port",
                "42000",
                "--served-model-name",
                "qwen-7b",
                "--tensor-parallel-size",
                "1",
                "--max-model-len",
                "8192",
                "--gpu-memory-utilization",
                "0.8",
                "--dtype",
                "auto",
                "--trust-remote-code",
                "--disable-log-requests",
            ]
        );
    }

    #[test]
    fn protected_raw_vllm_flags_fail_closed() {
        let mut request = request();
        request.vllm.extra_args = vec![VllmExtraArg {
            flag: "--port".to_string(),
            value: Some("9999".to_string()),
        }];

        let error = WorkloadSpecV1::build(request).expect_err("port override must fail");

        assert!(error.to_string().contains("Worker-controlled"));
    }

    #[test]
    fn ownership_requires_every_identity_label_and_hash() {
        let spec = WorkloadSpecV1::build(request()).expect("build workload");
        let mut actual = spec.labels.clone();
        spec.verify_ownership(&actual).expect("matching labels");

        actual.insert(LABEL_OWNER_ID.to_string(), "other-owner".to_string());
        assert!(matches!(
            spec.verify_ownership(&actual),
            Err(OwnershipError::Mismatch { label, .. }) if label == LABEL_OWNER_ID
        ));

        actual = spec.labels.clone();
        actual.remove(LABEL_SPEC_HASH);
        assert!(matches!(
            spec.verify_ownership(&actual),
            Err(OwnershipError::Missing { label }) if label == LABEL_SPEC_HASH
        ));
    }

    #[test]
    fn container_plan_uses_digest_uuid_and_least_privilege_defaults() {
        let spec = WorkloadSpecV1::build(request()).expect("build workload");
        let container = spec.container_spec();

        assert_eq!(container.image, DIGEST);
        assert_eq!(container.network_mode, "host");
        assert_eq!(container.shm_size_bytes, SHM_SIZE_BYTES);
        assert!(!container.privileged);
        assert!(!container.auto_remove);
        assert_eq!(container.security_options, vec!["no-new-privileges:true"]);
        assert_eq!(
            container.gpu_request.expect("GPU request").device_ids,
            vec!["GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3"]
        );
        let model_mount = container
            .mounts
            .iter()
            .find(|mount| mount.source == Path::new("/models/Qwen-7B"))
            .expect("model mount");
        assert!(model_mount.read_only);
    }

    #[test]
    fn m5_rejects_relative_paths_mutable_images_and_multiple_gpus() {
        let mut relative = request();
        relative.model_path = PathBuf::from("models/Qwen-7B");
        assert!(WorkloadSpecV1::build(relative).is_err());

        let mut mutable = request();
        mutable.resolved_image_digest = "runner:latest".to_string();
        assert!(WorkloadSpecV1::build(mutable).is_err());

        let mut multi = request();
        multi.gpus.push(WorkloadGpu {
            uuid: "GPU-second".to_string(),
            host_index: 1,
        });
        multi.tensor_parallel_size = 2;
        assert!(WorkloadSpecV1::build(multi).is_err());
    }

    #[test]
    fn managed_mode_adds_worker_and_model_instance_labels() {
        let mut request = request();
        request.mode = WorkloadMode::Managed;
        request.managed_worker_uuid = Some("worker-uuid-1".to_string());
        request.service_host = "10.213.80.90".to_string();
        let spec = WorkloadSpecV1::build(request).expect("managed workload");

        assert_eq!(
            spec.labels.get(LABEL_WORKER_UUID).map(String::as_str),
            Some("worker-uuid-1")
        );
        assert_eq!(
            spec.labels.get(LABEL_MODEL_INSTANCE_ID).map(String::as_str),
            Some("deployment-1")
        );
        assert!(!spec.labels.contains_key(LABEL_LAB_ID));
    }
}
