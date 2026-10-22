//! Isolated local Lab orchestration built on the shared workload and runtime contracts.

mod adapters;
mod service;
mod store;

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

use gpustack_runtime::docker::{ContainerLogOptions, RuntimeError, RuntimeLogLine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::workload::{VllmOptions, WorkloadError, WorkloadSpecV2};

pub use adapters::{
    FsModelPathResolver, KernelUuidSource, NvmlLabGpuProbe, ReqwestLabHttpClient, SystemLabClock,
    TcpPortProbe,
};
pub use service::{LabEnvironment, LabService};
pub use store::FsLabStore;

pub const LAB_SCHEMA_VERSION: u8 = 1;
pub const LAB_PORT_START: u16 = 42_000;
pub const LAB_PORT_END: u16 = 42_063;
pub const LAB_MODE: &str = "lab";

pub type LabFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, LabError>> + Send + 'a>>;
pub type ClockFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LabState {
    Created,
    Preparing,
    Starting,
    Running,
    Failed,
    Stopped,
}

impl LabState {
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::Created | Self::Preparing | Self::Starting | Self::Running
        )
    }

    #[must_use]
    pub const fn is_removable(self) -> bool {
        matches!(self, Self::Failed | Self::Stopped)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabOwner {
    pub schema_version: u8,
    pub owner_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabDeployment {
    pub schema_version: u8,
    pub lab_id: String,
    pub name: String,
    pub state: LabState,
    pub spec: WorkloadSpecV2,
    pub container_id: Option<String>,
    pub last_error: Option<String>,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LabDeployRequest {
    pub name: String,
    pub model_path: PathBuf,
    pub image: String,
    pub gpu: String,
    pub port: Option<u16>,
    pub served_model_name: Option<String>,
    pub vllm: VllmOptions,
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LabStatus {
    pub mode: &'static str,
    pub lab_id: String,
    pub name: String,
    pub state: LabState,
    pub container_state: Option<String>,
    pub container_id: Option<String>,
    pub spec_hash: String,
    pub image_digest: String,
    pub gpu_uuid: String,
    pub endpoint: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LabLogLine {
    pub stream: &'static str,
    pub message: String,
}

impl From<RuntimeLogLine> for LabLogLine {
    fn from(value: RuntimeLogLine) -> Self {
        use gpustack_runtime::docker::RuntimeLogStreamKind;

        let stream = match value.stream {
            RuntimeLogStreamKind::Stdout => "stdout",
            RuntimeLogStreamKind::Stderr => "stderr",
            RuntimeLogStreamKind::Stdin => "stdin",
            RuntimeLogStreamKind::Console => "console",
        };
        Self {
            stream,
            message: String::from_utf8_lossy(&value.message).into_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabInferenceKind {
    Chat,
    Completion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabInferRequest {
    pub name: String,
    pub kind: LabInferenceKind,
    pub prompt: String,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LabInferenceResponse {
    pub mode: &'static str,
    pub lab_id: String,
    pub endpoint: String,
    pub response: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LabCleanupPlan {
    pub mode: &'static str,
    pub dry_run: bool,
    pub deployment_ids: Vec<String>,
    pub container_ids: Vec<String>,
    pub cache_ids: Vec<String>,
    pub blocked: Vec<String>,
    pub removed_deployments: usize,
    pub removed_containers: usize,
    pub removed_caches: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabGpu {
    pub host_index: u32,
    pub uuid: String,
    pub running_compute_processes: usize,
}

pub trait LabStore: Send + Sync {
    fn lock(&self) -> Result<Box<dyn LabStoreSession>, LabError>;
}

/// A session owns the process-wide Lab lock for its complete lifetime.
pub trait LabStoreSession: Send {
    fn managed_lock_active(&self) -> Result<bool, LabError>;
    fn load_owner(&self) -> Result<Option<LabOwner>, LabError>;
    fn save_owner(&mut self, owner: &LabOwner) -> Result<(), LabError>;
    fn list_deployments(&self) -> Result<Vec<LabDeployment>, LabError>;
    fn save_deployment(&mut self, deployment: &LabDeployment) -> Result<(), LabError>;
    fn remove_deployment(&mut self, lab_id: &str) -> Result<(), LabError>;
    fn cache_path(&self, lab_id: &str) -> Result<PathBuf, LabError>;
    fn prepare_cache(&mut self, lab_id: &str) -> Result<PathBuf, LabError>;
    fn list_cache_ids(&self) -> Result<Vec<String>, LabError>;
    fn remove_cache(&mut self, lab_id: &str) -> Result<(), LabError>;
}

pub trait ModelPathResolver: Send + Sync {
    fn resolve(&self, roots: &[PathBuf], requested: &Path) -> Result<PathBuf, LabError>;
}

pub trait LabGpuProbe: Send + Sync {
    fn resolve(&self, selector: &str) -> Result<LabGpu, LabError>;
}

pub trait PortProbe: Send + Sync {
    fn is_available(&self, host: &str, port: u16) -> Result<bool, LabError>;
}

pub trait LabHttpClient: Send + Sync {
    fn models<'a>(&'a self, endpoint: &'a str) -> LabFuture<'a, Vec<String>>;
    fn infer<'a>(
        &'a self,
        endpoint: &'a str,
        model: &'a str,
        request: &'a LabInferRequest,
    ) -> LabFuture<'a, Value>;
}

pub trait LabClock: Send + Sync {
    fn now_unix(&self) -> u64;
    fn sleep(&self, duration: Duration) -> ClockFuture<'_>;
}

pub trait LabIdSource: Send + Sync {
    fn next_id(&self) -> Result<String, LabError>;
}

#[derive(Debug, Error)]
pub enum LabError {
    #[error("invalid Lab request: {0}")]
    Invalid(String),
    #[error("Lab deployment not found: {0}")]
    NotFound(String),
    #[error("Lab conflict: {0}")]
    Conflict(String),
    #[error("Lab operation is unsafe: {0}")]
    Unsafe(String),
    #[error("Lab state store is busy; another Lab command holds the lock")]
    Busy,
    #[error("Lab filesystem {action} failed for {path}: {message}")]
    Filesystem {
        action: &'static str,
        path: PathBuf,
        message: String,
    },
    #[error("Lab GPU probe failed: {0}")]
    Gpu(String),
    #[error("Lab HTTP operation failed: {0}")]
    Http(String),
    #[error("Lab deployment {lab_id} failed: {message}")]
    DeploymentFailed { lab_id: String, message: String },
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Workload(#[from] WorkloadError),
    #[error("failed to serialize Lab state: {0}")]
    Serialize(#[from] serde_json::Error),
}

impl LabError {
    pub(crate) fn filesystem(
        action: &'static str,
        path: impl Into<PathBuf>,
        error: impl std::fmt::Display,
    ) -> Self {
        Self::Filesystem {
            action,
            path: path.into(),
            message: error.to_string(),
        }
    }
}

#[must_use]
pub fn log_options(follow: bool, since_unix: i32, tail: String) -> ContainerLogOptions {
    ContainerLogOptions {
        follow,
        since_unix,
        tail,
        timestamps: true,
    }
}
