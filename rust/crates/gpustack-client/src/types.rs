//! GPUStack v2.2.3 wire models used by the R1 Worker control plane.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ClientError, SecretToken};

/// Server release accepted by the frozen R1 compatibility gate.
pub const SERVER_BASELINE_VERSION: &str = "v2.2.3";
/// Short source commit accepted by the frozen R1 compatibility gate.
pub const SERVER_BASELINE_GIT_COMMIT: &str = "1cfcbbf";

/// Public Server build identity returned by `/version`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ServerVersion {
    /// Semantic GPUStack release string.
    pub version: String,
    /// Short source commit when the Server image reports one.
    pub git_commit: Option<String>,
}

impl ServerVersion {
    /// Rejects a Server whose release or commit differs from the R1 baseline.
    pub fn verify_baseline(&self) -> Result<(), ClientError> {
        if self.version != SERVER_BASELINE_VERSION {
            return Err(ClientError::Contract(format!(
                "expected Server version {SERVER_BASELINE_VERSION}, got {}",
                self.version
            )));
        }
        if self.git_commit.as_deref() != Some(SERVER_BASELINE_GIT_COMMIT) {
            return Err(ClientError::Contract(format!(
                "expected Server commit {SERVER_BASELINE_GIT_COMMIT}, got {}",
                self.git_commit.as_deref().unwrap_or("<missing>")
            )));
        }
        Ok(())
    }
}

/// Standard structured error body returned by GPUStack APIs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ErrorResponse {
    /// GPUStack application error code.
    pub code: u16,
    /// Stable error category.
    pub reason: String,
    /// Operator-facing error description.
    pub message: String,
}

/// Pagination metadata attached to a collection response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Pagination {
    /// Requested page number; `-1` denotes the authoritative full list.
    pub page: i64,
    /// Maximum items per page.
    #[serde(rename = "perPage")]
    pub per_page: u64,
    /// Total number of matching resources.
    pub total: u64,
    /// Total number of pages.
    #[serde(rename = "totalPage")]
    pub total_page: u64,
}

/// Collection payload paired with Server pagination metadata.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Paginated<T> {
    /// Resources returned by the query.
    pub items: Vec<T>,
    /// Pagination state reported by Server.
    pub pagination: Pagination,
}

/// Network path used to route inference traffic to a Worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    /// Route through the Worker's proxy endpoint.
    Worker,
    /// Route directly to the inference endpoint.
    Direct,
    /// Route through a delegated proxy.
    Delegated,
    /// Route through a managed tunnel.
    Tunnel,
}

/// CPU measurements included in Worker status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuInfo {
    /// Total logical CPU capacity reported by the collector.
    pub total: Option<u64>,
    /// Current utilization percentage.
    pub utilization_rate: Option<f64>,
}

/// Host memory measurements included in Worker status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryInfo {
    /// Bytes reserved for model workloads.
    pub allocated: Option<u64>,
    /// Whether CPU and GPU share a unified memory pool.
    pub is_unified_memory: bool,
    /// Total memory in bytes.
    pub total: Option<u64>,
    /// Used memory in bytes.
    pub used: Option<u64>,
    /// Current utilization percentage.
    pub utilization_rate: Option<f64>,
}

/// Host swap measurements included in Worker status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwapInfo {
    /// Total swap space in bytes.
    pub total: Option<u64>,
    /// Used swap space in bytes.
    pub used: Option<u64>,
    /// Current utilization percentage.
    pub utilization_rate: Option<f64>,
}

/// Operating-system identity included in Worker status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatingSystemInfo {
    /// Distribution or operating-system name.
    pub name: String,
    /// Distribution or operating-system version.
    pub version: String,
}

/// Kernel identity included in Worker status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelInfo {
    /// Kernel machine architecture.
    pub architecture: String,
    /// Kernel family name.
    pub name: String,
    /// Kernel release identifier.
    pub release: String,
    /// Full kernel version string.
    pub version: String,
}

/// Host boot identity and elapsed uptime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UptimeInfo {
    /// Server-compatible boot timestamp.
    pub boot_time: String,
    /// Elapsed uptime in seconds.
    pub uptime: f64,
}

/// Resource snapshot sent with Worker registration and status reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResourceStatus {
    /// CPU measurements.
    pub cpu: Option<CpuInfo>,
    /// Filesystem measurements retained as version-specific JSON.
    pub filesystem: Option<Vec<Value>>,
    /// GPU measurements retained as version-specific JSON.
    pub gpu_devices: Option<Vec<Value>>,
    /// Kernel identity.
    pub kernel: Option<KernelInfo>,
    /// Memory measurements.
    pub memory: Option<MemoryInfo>,
    /// Operating-system identity.
    pub os: Option<OperatingSystemInfo>,
    /// RPC server topology retained as version-specific JSON.
    pub rpc_servers: Option<Value>,
    /// Swap measurements.
    pub swap: Option<SwapInfo>,
    /// Boot time and uptime.
    pub uptime: Option<UptimeInfo>,
}

/// Strict Worker registration body accepted by GPUStack v2.2.3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerCreate {
    /// Address advertised to control-plane peers when distinct from `ip`.
    pub advertise_address: Option<String>,
    /// Optional cluster assignment.
    pub cluster_id: Option<i64>,
    /// Optional identity supplied by an external inventory.
    pub external_id: Option<String>,
    /// Hostname reported by the Worker.
    pub hostname: String,
    /// Network interface associated with the Worker address.
    pub ifname: String,
    /// Worker control-plane IP address.
    pub ip: String,
    /// Scheduler labels attached to the Worker.
    pub labels: BTreeMap<String, String>,
    /// Stable host machine identifier when available.
    pub machine_id: Option<String>,
    /// Maintenance state retained as version-specific JSON.
    pub maintenance: Option<Value>,
    /// Prometheus metrics port.
    pub metrics_port: Option<u16>,
    /// Unique Worker name.
    pub name: String,
    /// Optional owning principal selected by Server policy.
    pub owner_principal_id: Option<i64>,
    /// Worker API port.
    pub port: u16,
    /// Inference traffic routing mode.
    pub proxy_mode: ProxyMode,
    /// Optional operator-facing Worker state detail.
    pub state_message: Option<String>,
    /// Initial host resource snapshot.
    pub status: Option<WorkerResourceStatus>,
    /// Reserved host resources retained as version-specific JSON.
    pub system_reserved: Option<Value>,
    /// Stable Worker installation identifier.
    pub worker_uuid: String,
    /// Worker software version.
    pub worker_version: String,
}

/// Strict Worker status replacement body accepted by GPUStack v2.2.3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerStatusReport {
    /// Address advertised to control-plane peers when distinct from `ip`.
    pub advertise_address: Option<String>,
    /// Optional gateway endpoint managed for this Worker.
    pub gateway_endpoint: Option<String>,
    /// Hostname reported by the Worker.
    pub hostname: String,
    /// Network interface associated with the Worker address.
    pub ifname: String,
    /// Worker control-plane IP address.
    pub ip: String,
    /// Stable host machine identifier when available.
    pub machine_id: Option<String>,
    /// Prometheus metrics port.
    pub metrics_port: Option<u16>,
    /// Worker API port.
    pub port: u16,
    /// Inference traffic routing mode.
    pub proxy_mode: ProxyMode,
    /// Optional operator-facing Worker state detail.
    pub state_message: Option<String>,
    /// Current host resource snapshot.
    pub status: Option<WorkerResourceStatus>,
    /// Reserved host resources retained as version-specific JSON.
    pub system_reserved: Option<Value>,
    /// Stable Worker installation identifier.
    pub worker_uuid: String,
}

/// Tolerant public Worker response used for identity resolution.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WorkerPublic {
    /// Server-assigned Worker identifier.
    pub id: i64,
    /// Unique Worker name.
    pub name: String,
    /// Whether this resource matches the authenticating Worker principal.
    pub me: Option<bool>,
    /// Reported Worker software version.
    pub worker_version: Option<String>,
    /// Server's current Worker resource snapshot.
    pub status: Option<Value>,
    /// Response fields not required by R1 but preserved for compatibility.
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

/// Successful Worker registration response including the issued credential.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WorkerRegistrationPublic {
    /// Server-assigned Worker identifier.
    pub id: i64,
    /// Registered Worker name.
    pub name: String,
    /// Worker bearer token that must be persisted securely by M3-C.
    pub token: SecretToken,
    /// Stable Worker installation identifier accepted by Server.
    pub worker_uuid: String,
    /// Registered Worker software version.
    pub worker_version: Option<String>,
    /// Optional Server-supplied Worker configuration.
    pub worker_config: Option<Value>,
    /// Response fields not required by R1 but preserved for compatibility.
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

/// Open string-backed model source value from the Server wire protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelSource(pub String);

impl ModelSource {
    /// Returns whether the source is the only R1-supported `local_path` value.
    pub fn is_local_path(&self) -> bool {
        self.0 == "local_path"
    }
}

/// Open string-backed ModelInstance lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelInstanceState(pub String);

/// Model definition needed to resolve local R1 assignments.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ModelPublic {
    /// Server-assigned Model identifier.
    pub id: i64,
    /// Operator-facing Model name.
    pub name: String,
    /// Model artifact source.
    pub source: ModelSource,
    /// Requested inference backend.
    pub backend: Option<String>,
    /// Local host path for `local_path` models.
    pub local_path: Option<String>,
    /// Optional runtime image selected by Server.
    pub image_name: Option<String>,
    /// Response fields not required by R1 but preserved for compatibility.
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

/// Tolerant ModelInstance response used for full sync and watch events.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ModelInstancePublic {
    /// Server-assigned ModelInstance identifier.
    pub id: i64,
    /// Server creation timestamp.
    pub created_at: String,
    /// Most recent Server update timestamp.
    pub updated_at: String,
    /// Backend version detected from the live inference API.
    pub api_detected_backend_version: Option<String>,
    /// Selected inference backend.
    pub backend: Option<String>,
    /// Configured backend version.
    pub backend_version: Option<String>,
    /// Optional cluster assignment.
    pub cluster_id: Option<i64>,
    /// Scheduler-computed resource claim.
    pub computed_resource_claim: Option<Value>,
    /// Cross-Worker topology, which R1 rejects when present.
    pub distributed_servers: Option<Value>,
    /// Primary model download percentage.
    pub download_progress: Option<f64>,
    /// Speculative draft-model download percentage.
    pub draft_model_download_progress: Option<f64>,
    /// Resolved draft-model artifact path.
    pub draft_model_resolved_path: Option<String>,
    /// Draft-model source retained as version-specific JSON.
    pub draft_model_source: Option<Value>,
    /// Stable GPU addresses assigned by the scheduler.
    pub gpu_addresses: Option<Vec<String>>,
    /// Local GPU indexes assigned by the scheduler.
    pub gpu_indexes: Option<Vec<u32>>,
    /// GPU product type expected by the assignment.
    pub gpu_type: Option<String>,
    /// Optional Hugging Face artifact filename.
    pub huggingface_filename: Option<String>,
    /// Optional Hugging Face repository identifier.
    pub huggingface_repo_id: Option<String>,
    /// Backend arguments injected by Server policy.
    pub injected_backend_parameters: Option<Vec<String>>,
    /// Timestamp of the most recent workload restart.
    pub last_restart_time: Option<String>,
    /// Configured local model directory.
    pub local_path: Option<String>,
    /// Parent Model identifier.
    pub model_id: i64,
    /// Parent Model name.
    pub model_name: String,
    /// Optional ModelScope artifact path.
    pub model_scope_file_path: Option<String>,
    /// Optional ModelScope model identifier.
    pub model_scope_model_id: Option<String>,
    /// Mounted LoRA adapters retained as version-specific JSON.
    pub mounted_loras: Option<Value>,
    /// Unique ModelInstance name.
    pub name: String,
    /// Principal that owns the assignment.
    pub owner_principal_id: i64,
    /// Inference process identifier reported by the Worker.
    pub pid: Option<i64>,
    /// Primary inference service port.
    pub port: Option<u16>,
    /// All inference service ports.
    pub ports: Option<Vec<u16>>,
    /// Resolved primary model artifact path.
    pub resolved_path: Option<String>,
    /// Number of workload restarts observed by the Worker.
    pub restart_count: Option<u32>,
    /// Model artifact source.
    pub source: ModelSource,
    /// Current lifecycle state.
    pub state: ModelInstanceState,
    /// Operator-facing lifecycle detail.
    pub state_message: Option<String>,
    /// Assigned Worker's advertised address.
    pub worker_advertise_address: Option<String>,
    /// Assigned Worker identifier.
    pub worker_id: Option<i64>,
    /// Assigned Worker's network interface.
    pub worker_ifname: Option<String>,
    /// Assigned Worker's IP address.
    pub worker_ip: Option<String>,
    /// Assigned Worker name.
    pub worker_name: Option<String>,
    /// Response fields not required by R1 but preserved for compatibility.
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

/// Strict full replacement body for a Worker-owned ModelInstance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelInstanceUpdate {
    /// Backend version detected from the live inference API.
    pub api_detected_backend_version: Option<String>,
    /// Selected inference backend; R1 requires `vLLM`.
    pub backend: Option<String>,
    /// Configured backend version.
    pub backend_version: Option<String>,
    /// Optional cluster assignment.
    pub cluster_id: Option<i64>,
    /// Scheduler-computed resource claim.
    pub computed_resource_claim: Option<Value>,
    /// Cross-Worker topology; R1 requires this to be absent.
    pub distributed_servers: Option<Value>,
    /// Primary model download percentage.
    pub download_progress: Option<f64>,
    /// Speculative draft-model download percentage.
    pub draft_model_download_progress: Option<f64>,
    /// Resolved draft-model artifact path.
    pub draft_model_resolved_path: Option<String>,
    /// Draft-model source retained as version-specific JSON.
    pub draft_model_source: Option<Value>,
    /// Stable GPU addresses assigned by the scheduler.
    pub gpu_addresses: Option<Vec<String>>,
    /// Local GPU indexes assigned by the scheduler.
    pub gpu_indexes: Option<Vec<u32>>,
    /// GPU product type expected by the assignment.
    pub gpu_type: Option<String>,
    /// Optional Hugging Face artifact filename.
    pub huggingface_filename: Option<String>,
    /// Optional Hugging Face repository identifier.
    pub huggingface_repo_id: Option<String>,
    /// Backend arguments injected by Server policy.
    pub injected_backend_parameters: Option<Vec<String>>,
    /// Timestamp of the most recent workload restart.
    pub last_restart_time: Option<String>,
    /// Configured local model directory; required by R1.
    pub local_path: Option<String>,
    /// Parent Model identifier.
    pub model_id: i64,
    /// Parent Model name.
    pub model_name: String,
    /// Optional ModelScope artifact path.
    pub model_scope_file_path: Option<String>,
    /// Optional ModelScope model identifier.
    pub model_scope_model_id: Option<String>,
    /// Mounted LoRA adapters retained as version-specific JSON.
    pub mounted_loras: Option<Value>,
    /// Unique ModelInstance name.
    pub name: String,
    /// Principal that owns the assignment.
    pub owner_principal_id: i64,
    /// Inference process identifier reported by the Worker.
    pub pid: Option<i64>,
    /// Primary inference service port.
    pub port: Option<u16>,
    /// All inference service ports.
    pub ports: Option<Vec<u16>>,
    /// Resolved primary model artifact path.
    pub resolved_path: Option<String>,
    /// Number of workload restarts observed by the Worker.
    pub restart_count: Option<u32>,
    /// Model artifact source; R1 requires `local_path`.
    pub source: ModelSource,
    /// Current lifecycle state.
    pub state: ModelInstanceState,
    /// Operator-facing lifecycle detail.
    pub state_message: Option<String>,
    /// Assigned Worker's advertised address.
    pub worker_advertise_address: Option<String>,
    /// Assigned Worker identifier used by the ownership guard.
    pub worker_id: Option<i64>,
    /// Assigned Worker's network interface.
    pub worker_ifname: Option<String>,
    /// Assigned Worker's IP address.
    pub worker_ip: Option<String>,
    /// Assigned Worker name.
    pub worker_name: Option<String>,
}

impl ModelInstanceUpdate {
    /// Enforces assignment ownership and the accepted R1 deployment subset.
    ///
    /// Validation happens before serialization so this client cannot report
    /// state for another Worker or silently accept unsupported workload shapes.
    pub fn validate_for_worker(&self, worker_id: i64) -> Result<(), ClientError> {
        if self.worker_id != Some(worker_id) {
            return Err(ClientError::Contract(format!(
                "model instance is assigned to worker {:?}, not worker {worker_id}",
                self.worker_id
            )));
        }
        if !self.source.is_local_path() {
            return Err(ClientError::Contract(
                "R1 only accepts model source local_path".to_owned(),
            ));
        }
        if self.backend.as_deref() != Some("vLLM") {
            return Err(ClientError::Contract("R1 requires backend vLLM".to_owned()));
        }
        if self.local_path.as_deref().is_none_or(str::is_empty) {
            return Err(ClientError::Contract(
                "local_path model instance requires a non-empty local_path".to_owned(),
            ));
        }
        if self.distributed_servers.is_some() {
            return Err(ClientError::Contract(
                "R1 does not support cross-Worker distributed inference".to_owned(),
            ));
        }
        Ok(())
    }
}

impl From<&ModelInstancePublic> for ModelInstanceUpdate {
    fn from(instance: &ModelInstancePublic) -> Self {
        Self {
            api_detected_backend_version: instance.api_detected_backend_version.clone(),
            backend: instance.backend.clone(),
            backend_version: instance.backend_version.clone(),
            cluster_id: instance.cluster_id,
            computed_resource_claim: instance.computed_resource_claim.clone(),
            distributed_servers: instance.distributed_servers.clone(),
            download_progress: instance.download_progress,
            draft_model_download_progress: instance.draft_model_download_progress,
            draft_model_resolved_path: instance.draft_model_resolved_path.clone(),
            draft_model_source: instance.draft_model_source.clone(),
            gpu_addresses: instance.gpu_addresses.clone(),
            gpu_indexes: instance.gpu_indexes.clone(),
            gpu_type: instance.gpu_type.clone(),
            huggingface_filename: instance.huggingface_filename.clone(),
            huggingface_repo_id: instance.huggingface_repo_id.clone(),
            injected_backend_parameters: instance.injected_backend_parameters.clone(),
            last_restart_time: instance.last_restart_time.clone(),
            local_path: instance.local_path.clone(),
            model_id: instance.model_id,
            model_name: instance.model_name.clone(),
            model_scope_file_path: instance.model_scope_file_path.clone(),
            model_scope_model_id: instance.model_scope_model_id.clone(),
            mounted_loras: instance.mounted_loras.clone(),
            name: instance.name.clone(),
            owner_principal_id: instance.owner_principal_id,
            pid: instance.pid,
            port: instance.port,
            ports: instance.ports.clone(),
            resolved_path: instance.resolved_path.clone(),
            restart_count: instance.restart_count,
            source: instance.source.clone(),
            state: instance.state.clone(),
            state_message: instance.state_message.clone(),
            worker_advertise_address: instance.worker_advertise_address.clone(),
            worker_id: instance.worker_id,
            worker_ifname: instance.worker_ifname.clone(),
            worker_ip: instance.worker_ip.clone(),
            worker_name: instance.worker_name.clone(),
        }
    }
}
