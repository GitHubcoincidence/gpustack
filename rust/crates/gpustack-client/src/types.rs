//! GPUStack v2.2.3 wire models used by the R1 Worker control plane.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ClientError, SecretToken};

pub const SERVER_BASELINE_VERSION: &str = "v2.2.3";
pub const SERVER_BASELINE_GIT_COMMIT: &str = "1cfcbbf";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ServerVersion {
    pub version: String,
    pub git_commit: Option<String>,
}

impl ServerVersion {
    /// Rejects a Server whose release identity differs from the frozen R1 baseline.
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ErrorResponse {
    pub code: u16,
    pub reason: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Pagination {
    pub page: i64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    pub total: u64,
    #[serde(rename = "totalPage")]
    pub total_page: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    Worker,
    Direct,
    Delegated,
    Tunnel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuInfo {
    pub total: Option<u64>,
    pub utilization_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryInfo {
    pub allocated: Option<u64>,
    pub is_unified_memory: bool,
    pub total: Option<u64>,
    pub used: Option<u64>,
    pub utilization_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwapInfo {
    pub total: Option<u64>,
    pub used: Option<u64>,
    pub utilization_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatingSystemInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelInfo {
    pub architecture: String,
    pub name: String,
    pub release: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UptimeInfo {
    pub boot_time: String,
    pub uptime: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MountPointInfo {
    pub name: String,
    pub mount_point: String,
    pub mount_from: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub available: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuCoreInfo {
    pub total: Option<u64>,
    pub utilization_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuDeviceInfo {
    pub vendor: Option<String>,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub index: Option<u32>,
    pub device_index: u32,
    pub device_chip_index: u32,
    pub arch_family: Option<String>,
    pub name: String,
    pub uuid: Option<String>,
    pub driver_version: Option<String>,
    pub runtime_version: Option<String>,
    pub compute_capability: Option<String>,
    pub core: Option<GpuCoreInfo>,
    pub memory: Option<MemoryInfo>,
    pub temperature: Option<f64>,
    pub network: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResourceStatus {
    pub cpu: Option<CpuInfo>,
    pub filesystem: Option<Vec<MountPointInfo>>,
    pub gpu_devices: Option<Vec<GpuDeviceInfo>>,
    pub kernel: Option<KernelInfo>,
    pub memory: Option<MemoryInfo>,
    pub os: Option<OperatingSystemInfo>,
    pub rpc_servers: Option<Value>,
    pub swap: Option<SwapInfo>,
    pub uptime: Option<UptimeInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerMaintenance {
    pub enabled: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemReserved {
    pub ram: u64,
    pub vram: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerCreate {
    pub advertise_address: Option<String>,
    pub cluster_id: Option<i64>,
    pub external_id: Option<String>,
    pub hostname: String,
    pub ifname: String,
    pub ip: String,
    pub labels: BTreeMap<String, String>,
    pub machine_id: Option<String>,
    pub maintenance: Option<WorkerMaintenance>,
    pub metrics_port: Option<u16>,
    pub name: String,
    pub owner_principal_id: Option<i64>,
    pub port: u16,
    pub proxy_mode: ProxyMode,
    pub state_message: Option<String>,
    pub status: Option<WorkerResourceStatus>,
    pub system_reserved: Option<SystemReserved>,
    pub worker_uuid: String,
    pub worker_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerStatusReport {
    pub advertise_address: Option<String>,
    pub gateway_endpoint: Option<String>,
    pub hostname: String,
    pub ifname: String,
    pub ip: String,
    pub machine_id: Option<String>,
    pub metrics_port: Option<u16>,
    pub port: u16,
    pub proxy_mode: ProxyMode,
    pub state_message: Option<String>,
    pub status: Option<WorkerResourceStatus>,
    pub system_reserved: Option<SystemReserved>,
    pub worker_uuid: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerPublic {
    pub id: i64,
    pub cluster_id: Option<i64>,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub maintenance: Option<WorkerMaintenance>,
    pub me: Option<bool>,
    pub worker_version: Option<String>,
    pub status: Option<Value>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerUpdate {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub maintenance: Option<WorkerMaintenance>,
}

impl From<&WorkerPublic> for WorkerUpdate {
    fn from(worker: &WorkerPublic) -> Self {
        Self {
            name: worker.name.clone(),
            labels: worker.labels.clone(),
            maintenance: worker.maintenance.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WorkerRegistrationPublic {
    pub id: i64,
    pub name: String,
    pub token: SecretToken,
    pub worker_uuid: String,
    pub worker_version: Option<String>,
    pub worker_config: Option<Value>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelSource(pub String);

impl ModelSource {
    pub fn is_local_path(&self) -> bool {
        self.0 == "local_path"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelInstanceState(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelFileState(pub String);

impl ModelFileState {
    fn is_supported(&self) -> bool {
        matches!(self.0.as_str(), "downloading" | "ready" | "error")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPublic {
    pub id: i64,
    pub name: String,
    pub source: ModelSource,
    pub backend: Option<String>,
    pub backend_parameters: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    pub restart_on_error: Option<bool>,
    pub local_path: Option<String>,
    pub image_name: Option<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuSelector {
    pub gpu_ids: Vec<String>,
    pub gpus_per_replica: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCreate {
    pub cluster_id: Option<i64>,
    pub name: String,
    pub source: ModelSource,
    pub local_path: String,
    pub replicas: u32,
    pub backend: String,
    pub image_name: String,
    pub worker_selector: BTreeMap<String, String>,
    pub gpu_selector: GpuSelector,
    pub backend_parameters: Vec<String>,
    pub distributed_inference_across_workers: bool,
    pub enable_model_route: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelFilePublic {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub base_model: Option<String>,
    pub cleanup_on_delete: Option<bool>,
    pub download_progress: Option<f64>,
    pub huggingface_filename: Option<String>,
    pub huggingface_repo_id: Option<String>,
    pub is_lora: bool,
    pub local_dir: Option<String>,
    pub local_path: Option<String>,
    pub model_scope_file_path: Option<String>,
    pub model_scope_model_id: Option<String>,
    pub owner_principal_id: Option<i64>,
    pub resolved_paths: Vec<String>,
    pub size: Option<i64>,
    pub source: ModelSource,
    pub state: ModelFileState,
    pub state_message: Option<String>,
    pub worker_id: Option<i64>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Complete replacement body for a Worker-owned ModelFile.
pub struct ModelFileUpdate {
    pub base_model: Option<String>,
    pub cleanup_on_delete: Option<bool>,
    pub download_progress: Option<f64>,
    pub huggingface_filename: Option<String>,
    pub huggingface_repo_id: Option<String>,
    pub is_lora: bool,
    pub local_dir: Option<String>,
    pub local_path: Option<String>,
    pub model_scope_file_path: Option<String>,
    pub model_scope_model_id: Option<String>,
    pub resolved_paths: Vec<String>,
    pub size: Option<i64>,
    pub source: ModelSource,
    pub state: ModelFileState,
    pub state_message: Option<String>,
    pub worker_id: Option<i64>,
}

impl ModelFileUpdate {
    /// Prevents writes for another Worker or an unsupported R1 model source.
    pub fn validate_for_worker(&self, worker_id: i64) -> Result<(), ClientError> {
        if self.worker_id != Some(worker_id) {
            return Err(ClientError::Contract(format!(
                "model file is assigned to worker {:?}, not worker {worker_id}",
                self.worker_id
            )));
        }
        if !self.source.is_local_path() {
            return Err(ClientError::Contract(
                "R1 only accepts model source local_path".to_owned(),
            ));
        }
        if self.local_path.as_deref().is_none_or(str::is_empty) {
            return Err(ClientError::Contract(
                "local_path model file requires a non-empty local_path".to_owned(),
            ));
        }
        if !self.state.is_supported() {
            return Err(ClientError::Contract(format!(
                "unsupported ModelFile state {}",
                self.state.0
            )));
        }
        Ok(())
    }
}

impl From<&ModelFilePublic> for ModelFileUpdate {
    fn from(model_file: &ModelFilePublic) -> Self {
        Self {
            base_model: model_file.base_model.clone(),
            cleanup_on_delete: model_file.cleanup_on_delete,
            download_progress: model_file.download_progress,
            huggingface_filename: model_file.huggingface_filename.clone(),
            huggingface_repo_id: model_file.huggingface_repo_id.clone(),
            is_lora: model_file.is_lora,
            local_dir: model_file.local_dir.clone(),
            local_path: model_file.local_path.clone(),
            model_scope_file_path: model_file.model_scope_file_path.clone(),
            model_scope_model_id: model_file.model_scope_model_id.clone(),
            resolved_paths: model_file.resolved_paths.clone(),
            size: model_file.size,
            source: model_file.source.clone(),
            state: model_file.state.clone(),
            state_message: model_file.state_message.clone(),
            worker_id: model_file.worker_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInstancePublic {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub api_detected_backend_version: Option<String>,
    pub backend: Option<String>,
    pub backend_version: Option<String>,
    pub cluster_id: Option<i64>,
    pub computed_resource_claim: Option<Value>,
    pub distributed_servers: Option<Value>,
    pub download_progress: Option<f64>,
    pub draft_model_download_progress: Option<f64>,
    pub draft_model_resolved_path: Option<String>,
    pub draft_model_source: Option<Value>,
    pub gpu_addresses: Option<Vec<String>>,
    pub gpu_indexes: Option<Vec<u32>>,
    pub gpu_type: Option<String>,
    pub huggingface_filename: Option<String>,
    pub huggingface_repo_id: Option<String>,
    pub injected_backend_parameters: Option<Vec<String>>,
    pub last_restart_time: Option<String>,
    pub local_path: Option<String>,
    pub model_id: i64,
    pub model_name: String,
    pub model_scope_file_path: Option<String>,
    pub model_scope_model_id: Option<String>,
    pub mounted_loras: Option<Value>,
    pub name: String,
    pub owner_principal_id: i64,
    pub pid: Option<i64>,
    pub port: Option<u16>,
    pub ports: Option<Vec<u16>>,
    pub resolved_path: Option<String>,
    pub restart_count: Option<u32>,
    pub source: ModelSource,
    pub state: ModelInstanceState,
    pub state_message: Option<String>,
    pub worker_advertise_address: Option<String>,
    pub worker_id: Option<i64>,
    pub worker_ifname: Option<String>,
    pub worker_ip: Option<String>,
    pub worker_name: Option<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

impl ModelInstancePublic {
    /// Treats the Server's empty distributed envelope as a single-Worker placement.
    pub fn spans_workers(&self) -> bool {
        distributed_servers_span_workers(self.distributed_servers.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Complete replacement body for a Worker-owned ModelInstance.
pub struct ModelInstanceUpdate {
    pub api_detected_backend_version: Option<String>,
    pub backend: Option<String>,
    pub backend_version: Option<String>,
    pub cluster_id: Option<i64>,
    pub computed_resource_claim: Option<Value>,
    pub distributed_servers: Option<Value>,
    pub download_progress: Option<f64>,
    pub draft_model_download_progress: Option<f64>,
    pub draft_model_resolved_path: Option<String>,
    pub draft_model_source: Option<Value>,
    pub gpu_addresses: Option<Vec<String>>,
    pub gpu_indexes: Option<Vec<u32>>,
    pub gpu_type: Option<String>,
    pub huggingface_filename: Option<String>,
    pub huggingface_repo_id: Option<String>,
    pub injected_backend_parameters: Option<Vec<String>>,
    pub last_restart_time: Option<String>,
    pub local_path: Option<String>,
    pub model_id: i64,
    pub model_name: String,
    pub model_scope_file_path: Option<String>,
    pub model_scope_model_id: Option<String>,
    pub mounted_loras: Option<Value>,
    pub name: String,
    pub owner_principal_id: i64,
    pub pid: Option<i64>,
    pub port: Option<u16>,
    pub ports: Option<Vec<u16>>,
    pub resolved_path: Option<String>,
    pub restart_count: Option<u32>,
    pub source: ModelSource,
    pub state: ModelInstanceState,
    pub state_message: Option<String>,
    pub worker_advertise_address: Option<String>,
    pub worker_id: Option<i64>,
    pub worker_ifname: Option<String>,
    pub worker_ip: Option<String>,
    pub worker_name: Option<String>,
}

impl ModelInstanceUpdate {
    /// Prevents writes for another Worker or an unsupported R1 workload shape.
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
        if distributed_servers_span_workers(self.distributed_servers.as_ref()) {
            return Err(ClientError::Contract(
                "R1 does not support cross-Worker distributed inference".to_owned(),
            ));
        }
        Ok(())
    }
}

fn distributed_servers_span_workers(distributed_servers: Option<&Value>) -> bool {
    let Some(Value::Object(distributed_servers)) = distributed_servers else {
        return distributed_servers.is_some();
    };
    match distributed_servers.get("subordinate_workers") {
        None | Some(Value::Null) => false,
        Some(Value::Array(workers)) => !workers.is_empty(),
        Some(_) => true,
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
