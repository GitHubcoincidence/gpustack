//! Worker command grammar and local runtime configuration overrides.

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use gpustack_worker::ConfigOverrides;
use gpustack_worker::lab::LabInferenceKind;
use gpustack_worker::workload::VllmDtype;

use crate::managed::{WorkerMaintenanceArgs, WorkerSelectorArgs};

#[derive(Debug, Args)]
pub struct WorkerArgs {
    #[command(flatten)]
    pub overrides: WorkerConfigArgs,
    #[command(subcommand)]
    pub command: WorkerCommand,
}

#[derive(Debug, Args, Default)]
pub struct WorkerConfigArgs {
    #[arg(long, global = true)]
    pub server_url: Option<String>,
    #[arg(long, global = true)]
    pub worker_name: Option<String>,
    #[arg(long, global = true)]
    pub worker_hostname: Option<String>,
    #[arg(long, global = true)]
    pub worker_ip: Option<String>,
    #[arg(long, global = true)]
    pub worker_ifname: Option<String>,
    #[arg(long, global = true)]
    pub worker_port: Option<u16>,
    #[arg(long, global = true)]
    pub metrics_port: Option<u16>,
    #[arg(long, global = true)]
    pub token_file: Option<PathBuf>,
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pub log_dir: Option<PathBuf>,
    #[arg(long = "model-root", global = true)]
    pub model_roots: Vec<PathBuf>,
    #[arg(long, global = true)]
    pub docker_socket: Option<PathBuf>,
}

impl From<WorkerConfigArgs> for ConfigOverrides {
    fn from(value: WorkerConfigArgs) -> Self {
        Self {
            server_url: value.server_url,
            worker_name: value.worker_name,
            worker_hostname: value.worker_hostname,
            worker_ip: value.worker_ip,
            worker_ifname: value.worker_ifname,
            worker_port: value.worker_port,
            metrics_port: value.metrics_port,
            token_file: value.token_file,
            data_dir: value.data_dir,
            log_dir: value.log_dir,
            model_roots: (!value.model_roots.is_empty()).then_some(value.model_roots),
            docker_socket: value.docker_socket,
        }
    }
}

impl WorkerConfigArgs {
    pub(crate) fn has_explicit_runtime_override(&self) -> bool {
        self.server_url.is_some()
            || self.worker_name.is_some()
            || self.worker_hostname.is_some()
            || self.worker_ip.is_some()
            || self.worker_ifname.is_some()
            || self.worker_port.is_some()
            || self.metrics_port.is_some()
            || self.token_file.is_some()
            || self.data_dir.is_some()
            || self.log_dir.is_some()
            || !self.model_roots.is_empty()
            || self.docker_socket.is_some()
    }
}

#[derive(Debug, Subcommand)]
pub enum WorkerCommand {
    Config(WorkerConfigCommand),
    Doctor,
    /// Get one Server Worker by numeric ID or exact name.
    Get(WorkerSelectorArgs),
    Gpu(WorkerGpuCommand),
    Lab(WorkerLabCommand),
    /// Change the exact Worker's scheduling maintenance state through Server.
    Maintenance(WorkerMaintenanceArgs),
    Runtime(WorkerRuntimeCommand),
    /// Run the complete production Worker lifecycle.
    Run,
    ValidateConfig,
}

#[derive(Debug, Args)]
pub struct WorkerRuntimeCommand {
    #[command(subcommand)]
    pub command: WorkerRuntimeSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum WorkerRuntimeSubcommand {
    Info,
}

#[derive(Debug, Args)]
pub struct WorkerLabCommand {
    #[command(subcommand)]
    pub command: WorkerLabSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum WorkerLabSubcommand {
    Deploy(LabDeployArgs),
    List,
    Status(LabNameArgs),
    Logs(LabLogsArgs),
    Infer(LabInferArgs),
    Stop(LabNameArgs),
    Remove(LabRemoveArgs),
    Cleanup(LabCleanupArgs),
}

#[derive(Debug, Args)]
pub struct LabDeployArgs {
    pub name: String,
    #[arg(long)]
    pub model_path: PathBuf,
    #[arg(long)]
    pub image: String,
    #[arg(long)]
    pub gpu: String,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub served_model_name: Option<String>,
    #[arg(long)]
    pub max_model_len: Option<u32>,
    #[arg(long)]
    pub gpu_memory_utilization: Option<f64>,
    #[arg(long, value_enum)]
    pub dtype: Option<CliVllmDtype>,
    #[arg(long)]
    pub trust_remote_code: bool,
    #[arg(long)]
    pub enforce_eager: bool,
    #[arg(long = "vllm-extra-arg", allow_hyphen_values = true)]
    pub extra_args: Vec<String>,
}

#[derive(Debug, Args)]
pub struct LabNameArgs {
    pub name: String,
}

#[derive(Debug, Args)]
pub struct LabLogsArgs {
    pub name: String,
    #[arg(long, default_value = "100")]
    pub tail: String,
    #[arg(long, default_value_t = 0)]
    pub since: i32,
    #[arg(long)]
    pub follow: bool,
}

#[derive(Debug, Args)]
pub struct LabInferArgs {
    pub name: String,
    #[arg(long, value_enum, default_value_t = CliInferenceKind::Chat)]
    pub kind: CliInferenceKind,
    #[arg(long)]
    pub prompt: String,
    #[arg(long)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Args)]
pub struct LabRemoveArgs {
    pub name: String,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct LabCleanupArgs {
    #[arg(long)]
    pub execute: bool,
    #[arg(long, requires = "execute")]
    pub yes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliInferenceKind {
    Chat,
    Completion,
}

impl From<CliInferenceKind> for LabInferenceKind {
    fn from(value: CliInferenceKind) -> Self {
        match value {
            CliInferenceKind::Chat => Self::Chat,
            CliInferenceKind::Completion => Self::Completion,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliVllmDtype {
    Auto,
    Half,
    Float16,
    Bfloat16,
    Float,
    Float32,
}

impl From<CliVllmDtype> for VllmDtype {
    fn from(value: CliVllmDtype) -> Self {
        match value {
            CliVllmDtype::Auto => Self::Auto,
            CliVllmDtype::Half => Self::Half,
            CliVllmDtype::Float16 => Self::Float16,
            CliVllmDtype::Bfloat16 => Self::Bfloat16,
            CliVllmDtype::Float => Self::Float,
            CliVllmDtype::Float32 => Self::Float32,
        }
    }
}

#[derive(Debug, Args)]
pub struct WorkerConfigCommand {
    #[command(subcommand)]
    pub command: WorkerConfigSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum WorkerConfigSubcommand {
    Get,
}

#[derive(Debug, Args)]
pub struct WorkerGpuCommand {
    #[command(subcommand)]
    pub command: WorkerGpuSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum WorkerGpuSubcommand {
    List,
}
