//! CLI grammar, stable exit codes, and output rendering for `gpustack-rs`.

use std::fmt::Write as _;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use gpustack_client::http::ReqwestTransport;
use gpustack_client::{ClientError, ContextStore, ServerClient, load_context_token};
use gpustack_runtime::docker::{ContainerRuntime, DockerDaemonInfo, DockerRuntime, RuntimeError};
use gpustack_worker::lab::{
    LabCleanupPlan, LabDeployRequest, LabEnvironment, LabError, LabInferRequest, LabInferenceKind,
    LabInferenceResponse, LabLogLine, LabState, LabStatus,
};
use gpustack_worker::resources::{ResourceError, collect_gpu_inventory};
use gpustack_worker::worker_api::public_config_view;
use gpustack_worker::workload::{VllmDtype, VllmExtraArg, VllmOptions};
use gpustack_worker::{
    ConfigError, ConfigOverrides, ControlPlaneOnlySummary, DoctorReport, ManagedFileSummary,
    ManagedRuntimeSummary, ResourceApiRunError, ResourceApiSummary, WorkerConfig, WorkerRunError,
    load_config, run_control_plane_only, run_doctor, run_managed_file_only, run_managed_runtime,
    run_resource_api_only,
};
use serde::Serialize;
use thiserror::Error;

mod managed;

use managed::{
    ContextArgs, ContextSubcommand, InferenceArgs, ManagedError, ModelArgs, ModelFileArgs,
    ModelFileSubcommand, ModelSubcommand, ServerStreamSink, WorkerMaintenanceArgs,
    WorkerMaintenanceSubcommand, WorkerSelectorArgs, run_context_command, run_inference_command,
    run_model_command_with_stream, run_worker_get_command, run_worker_maintenance_command,
};

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_INTERNAL: u8 = 1;
pub const EXIT_CONFIG: u8 = 10;
pub const EXIT_CONTROL_PLANE: u8 = 30;
pub const EXIT_RUNTIME: u8 = 50;

#[derive(Debug, Parser)]
#[command(name = "gpustack-rs", version, about = "GPUStack Rust migration CLI")]
pub struct Cli {
    /// Worker runtime YAML configuration file.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    /// Stable human-readable or JSON output.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,
    /// Suppress successful command and progress output.
    #[arg(long, global = true)]
    pub quiet: bool,
    /// Disable terminal color output.
    #[arg(long, global = true)]
    pub no_color: bool,
    /// Request or wait timeout in seconds.
    #[arg(long, global = true, default_value_t = 30)]
    pub timeout: u64,
    /// Named Server context; defaults to the active context.
    #[arg(long, global = true)]
    pub context: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage local Server contexts and token-file references.
    Context(ContextArgs),
    /// Manage Server-owned model deployments.
    Model(ModelArgs),
    /// Invoke OpenAI-compatible inference through the active Server context.
    Inference(InferenceArgs),
    /// Print the Rust CLI build identity.
    Version,
    /// Run, inspect, or maintain a Worker.
    Worker(Box<WorkerArgs>),
}

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
    fn has_explicit_runtime_override(&self) -> bool {
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
    Run(WorkerRunArgs),
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

#[derive(Debug, Args)]
pub struct WorkerRunArgs {
    /// Run only identity, heartbeat, status, and desired-state control loops.
    #[arg(
        long,
        conflicts_with_all = ["resource_api_only", "managed_file_only", "managed_runtime"]
    )]
    pub control_plane_only: bool,
    /// Add host resources, the read-only Worker API, and metrics without reconciliation.
    #[arg(
        long,
        conflicts_with_all = ["control_plane_only", "managed_file_only", "managed_runtime"]
    )]
    pub resource_api_only: bool,
    /// Prepare assigned local model files and stop before container or inference startup.
    #[arg(
        long,
        conflicts_with_all = ["control_plane_only", "resource_api_only", "managed_runtime"]
    )]
    pub managed_file_only: bool,
    /// Enable Server-authoritative single-GPU Managed Docker and vLLM reconciliation.
    #[arg(
        long,
        conflicts_with_all = ["control_plane_only", "resource_api_only", "managed_file_only"]
    )]
    pub managed_runtime: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
}

#[derive(Debug)]
pub struct Execution {
    pub output: String,
    pub exit_code: u8,
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error(
        "worker run requires exactly one of --control-plane-only, --resource-api-only, --managed-file-only, or --managed-runtime"
    )]
    WorkerModeRequired,
    #[error(transparent)]
    Resource(#[from] ResourceError),
    #[error(transparent)]
    ResourceRun(#[from] ResourceApiRunError),
    #[error(transparent)]
    WorkerRun(#[from] WorkerRunError),
    #[error(transparent)]
    Lab(#[from] LabError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Managed(#[from] ManagedError),
    #[error("invalid CLI argument: {0}")]
    InvalidArgument(String),
    #[error("destructive Lab command requires confirmation or --yes")]
    ConfirmationRequired,
    #[error("failed to read interactive confirmation: {0}")]
    ConfirmationIo(String),
    #[error("failed to serialize command output")]
    Serialization(#[from] serde_json::Error),
}

impl CliError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Config(_)
            | Self::WorkerModeRequired
            | Self::InvalidArgument(_)
            | Self::ConfirmationRequired
            | Self::ConfirmationIo(_) => EXIT_CONFIG,
            Self::WorkerRun(_) => EXIT_CONTROL_PLANE,
            Self::Resource(_) => EXIT_RUNTIME,
            Self::ResourceRun(_) => EXIT_CONTROL_PLANE,
            Self::Lab(_) | Self::Runtime(_) => EXIT_RUNTIME,
            Self::Managed(ManagedError::Serialization(_)) => EXIT_INTERNAL,
            Self::Managed(error) if error.is_control_plane() => EXIT_CONTROL_PLANE,
            Self::Managed(_) => EXIT_CONFIG,
            Self::Serialization(_) => EXIT_INTERNAL,
        }
    }
}

/// Executes an already parsed command without directly accessing process streams.
pub async fn execute(cli: Cli) -> Result<Execution, CliError> {
    execute_inner(cli, None, None).await
}

/// Executes a command and emits Docker log frames as they arrive.
pub async fn execute_with_lab_log_sink(
    cli: Cli,
    sink: &mut dyn FnMut(&LabLogLine),
) -> Result<Execution, CliError> {
    execute_inner(cli, Some(sink), None).await
}

/// Executes a command while emitting local Lab logs and Server byte streams.
pub async fn execute_with_output_sinks(
    cli: Cli,
    lab_log_sink: &mut dyn FnMut(&LabLogLine),
    server_stream_sink: &mut dyn FnMut(&[u8]),
) -> Result<Execution, CliError> {
    execute_inner(cli, Some(lab_log_sink), Some(server_stream_sink)).await
}

async fn execute_inner(
    cli: Cli,
    lab_log_sink: Option<&mut dyn FnMut(&LabLogLine)>,
    server_stream_sink: Option<ServerStreamSink<'_>>,
) -> Result<Execution, CliError> {
    let quiet = cli.quiet;
    let output_format = cli.output;
    let context_name = cli.context;
    let timeout = Duration::from_secs(cli.timeout.max(1));
    let execution = match cli.command {
        Command::Context(arguments) => {
            if cli.config.is_some() || context_name.is_some() {
                return Err(CliError::InvalidArgument(
                    "context commands do not accept --config or --context".to_owned(),
                ));
            }
            let store = ContextStore::discover().map_err(ManagedError::from)?;
            run_context_command(output_format, &store, arguments)?
        }
        Command::Model(arguments) => {
            if cli.config.is_some() {
                return Err(CliError::InvalidArgument(
                    "--config is a Worker runtime option and does not apply to model commands"
                        .to_owned(),
                ));
            }
            let (client, token) = operator_client(context_name.as_deref(), timeout)?;
            let mut transition_sink = |transition: &managed::ModelTransition| {
                if !quiet {
                    eprintln!("{}", transition.event_line());
                }
            };
            run_model_command_with_stream(
                output_format,
                timeout,
                &client,
                &token,
                arguments,
                Some(&mut transition_sink),
                server_stream_sink,
            )
            .await?
        }
        Command::Inference(arguments) => {
            if cli.config.is_some() {
                return Err(CliError::InvalidArgument(
                    "--config is a Worker runtime option and does not apply to inference commands"
                        .to_owned(),
                ));
            }
            let (client, token) = operator_client(context_name.as_deref(), timeout)?;
            run_inference_command(
                output_format,
                &client,
                &token,
                arguments,
                server_stream_sink,
            )
            .await?
        }
        Command::Version => render_version(output_format)?,
        Command::Worker(worker) => {
            let WorkerArgs { overrides, command } = *worker;
            match command {
                WorkerCommand::Get(arguments) => {
                    if cli.config.is_some() || overrides.has_explicit_runtime_override() {
                        return Err(CliError::InvalidArgument(
                            "Worker runtime configuration does not apply to get; use --context"
                                .to_owned(),
                        ));
                    }
                    let (client, token) = operator_client(context_name.as_deref(), timeout)?;
                    run_worker_get_command(output_format, &client, &token, arguments).await?
                }
                WorkerCommand::Maintenance(arguments) => {
                    if cli.config.is_some() || overrides.has_explicit_runtime_override() {
                        return Err(CliError::InvalidArgument(
                            "Worker runtime configuration does not apply to maintenance; use --context"
                                .to_owned(),
                        ));
                    }
                    let (client, token) = operator_client(context_name.as_deref(), timeout)?;
                    run_worker_maintenance_command(output_format, &client, &token, arguments)
                        .await?
                }
                command => {
                    if context_name.is_some() {
                        return Err(CliError::InvalidArgument(
                            "--context applies only to model, inference, Worker get, and Worker maintenance commands"
                                .to_owned(),
                        ));
                    }
                    let loaded = load_config(cli.config.as_deref(), overrides.into())?;
                    match command {
                        WorkerCommand::Config(arguments) => match arguments.command {
                            WorkerConfigSubcommand::Get => render_public_config(output_format)?,
                        },
                        WorkerCommand::ValidateConfig => render_valid_config(
                            output_format,
                            &loaded.config,
                            loaded.source.as_deref(),
                        )?,
                        WorkerCommand::Doctor => {
                            render_doctor(output_format, &run_doctor(&loaded.config))?
                        }
                        WorkerCommand::Gpu(arguments) => match arguments.command {
                            WorkerGpuSubcommand::List => {
                                render_gpu_inventory(output_format, &collect_gpu_inventory()?)?
                            }
                        },
                        WorkerCommand::Lab(arguments) => {
                            run_lab_command(
                                output_format,
                                cli.timeout,
                                &loaded.config,
                                arguments,
                                lab_log_sink,
                            )
                            .await?
                        }
                        WorkerCommand::Get(_) => unreachable!("handled above"),
                        WorkerCommand::Maintenance(_) => unreachable!("handled above"),
                        WorkerCommand::Runtime(arguments) => {
                            run_runtime_command(
                                output_format,
                                cli.timeout,
                                &loaded.config,
                                arguments,
                            )
                            .await?
                        }
                        WorkerCommand::Run(arguments) => {
                            if !arguments.control_plane_only
                                && !arguments.resource_api_only
                                && !arguments.managed_file_only
                                && !arguments.managed_runtime
                            {
                                return Err(CliError::WorkerModeRequired);
                            }
                            if arguments.managed_runtime {
                                let summary = run_managed_runtime(&loaded.config, |event| {
                                    if !quiet {
                                        eprintln!("{event}");
                                    }
                                })
                                .await?;
                                render_managed_runtime_summary(output_format, &summary)?
                            } else if arguments.managed_file_only {
                                let summary = run_managed_file_only(&loaded.config, |event| {
                                    if !quiet {
                                        eprintln!("{event}");
                                    }
                                })
                                .await?;
                                render_managed_file_summary(output_format, &summary)?
                            } else if arguments.resource_api_only {
                                let summary = run_resource_api_only(&loaded.config, |event| {
                                    if !quiet {
                                        eprintln!("{event}");
                                    }
                                })
                                .await?;
                                render_resource_api_summary(output_format, &summary)?
                            } else {
                                let summary = run_control_plane_only(&loaded.config, |event| {
                                    if !quiet {
                                        eprintln!("{event}");
                                    }
                                })
                                .await?;
                                render_control_plane_summary(output_format, &summary)?
                            }
                        }
                    }
                }
            }
        }
    };

    if quiet && execution.exit_code == EXIT_SUCCESS {
        Ok(Execution {
            output: String::new(),
            ..execution
        })
    } else {
        Ok(execution)
    }
}

fn operator_client(
    context_name: Option<&str>,
    timeout: Duration,
) -> Result<(ServerClient<ReqwestTransport>, gpustack_client::SecretToken), ManagedError> {
    let store = ContextStore::discover()?;
    let context = store.resolve(context_name)?;
    let token = load_context_token(&context)?;
    let transport = ReqwestTransport::new().map_err(ClientError::Transport)?;
    let client = ServerClient::with_timeout(context.server_url, transport, timeout)?;
    Ok((client, token))
}

async fn run_runtime_command(
    output_format: OutputFormat,
    timeout_secs: u64,
    config: &WorkerConfig,
    arguments: WorkerRuntimeCommand,
) -> Result<Execution, CliError> {
    let runtime = DockerRuntime::connect(
        &config.docker_socket,
        Duration::from_secs(timeout_secs.max(1)),
    )?;
    match arguments.command {
        WorkerRuntimeSubcommand::Info => {
            let info = runtime.daemon_info().await?;
            render_runtime_info(output_format, &info)
        }
    }
}

impl Cli {
    /// Prompts for destructive local or Server operations that omit `--yes`.
    pub fn confirm_destructive_command(&mut self) -> Result<(), CliError> {
        let (prompt, confirmed) = match &mut self.command {
            Command::Context(ContextArgs {
                command: ContextSubcommand::Remove(arguments),
            }) if !arguments.yes => (
                format!(
                    "Remove context '{}'? Type 'yes' to continue: ",
                    arguments.name
                ),
                &mut arguments.yes,
            ),
            Command::Model(ModelArgs {
                command: ModelSubcommand::Delete(arguments),
            }) if !arguments.yes => (
                format!(
                    "Delete Managed model '{}'? Type 'yes' to continue: ",
                    arguments.model
                ),
                &mut arguments.yes,
            ),
            Command::Inference(_) => return Ok(()),
            Command::Model(ModelArgs {
                command:
                    ModelSubcommand::File(ModelFileArgs {
                        command: ModelFileSubcommand::Delete(arguments),
                    }),
            }) if !arguments.yes => (
                format!(
                    "Delete Server ModelFile metadata '{}' while preserving local files? Type 'yes' to continue: ",
                    arguments.model_file_id
                ),
                &mut arguments.yes,
            ),
            Command::Worker(worker) => match &mut worker.command {
                WorkerCommand::Maintenance(WorkerMaintenanceArgs {
                    command: WorkerMaintenanceSubcommand::Disable(arguments),
                }) if !arguments.yes => (
                    format!(
                        "Disable maintenance for Worker '{}'? Type 'yes' to continue: ",
                        arguments.worker
                    ),
                    &mut arguments.yes,
                ),
                WorkerCommand::Lab(WorkerLabCommand {
                    command: WorkerLabSubcommand::Remove(arguments),
                }) if !arguments.yes => {
                    let prompt = format!(
                        "Remove stopped/failed Lab deployment '{}'? Type 'yes' to continue: ",
                        arguments.name
                    );
                    (prompt, &mut arguments.yes)
                }
                WorkerCommand::Lab(WorkerLabCommand {
                    command: WorkerLabSubcommand::Cleanup(arguments),
                }) if arguments.execute && !arguments.yes => (
                    "Execute cleanup for all removable resources owned by this Lab owner? Type 'yes' to continue: "
                        .to_string(),
                    &mut arguments.yes,
                ),
                _ => return Ok(()),
            },
            _ => return Ok(()),
        };
        eprint!("{prompt}");
        std::io::Write::flush(&mut io::stderr())
            .map_err(|error| CliError::ConfirmationIo(error.to_string()))?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|error| CliError::ConfirmationIo(error.to_string()))?;
        if input.trim() != "yes" {
            return Err(CliError::ConfirmationRequired);
        }
        *confirmed = true;
        Ok(())
    }
}

async fn run_lab_command(
    output_format: OutputFormat,
    timeout_secs: u64,
    config: &WorkerConfig,
    arguments: WorkerLabCommand,
    lab_log_sink: Option<&mut dyn FnMut(&LabLogLine)>,
) -> Result<Execution, CliError> {
    match &arguments.command {
        WorkerLabSubcommand::Remove(arguments) if !arguments.yes => {
            return Err(CliError::ConfirmationRequired);
        }
        WorkerLabSubcommand::Cleanup(arguments) if arguments.execute && !arguments.yes => {
            return Err(CliError::ConfirmationRequired);
        }
        _ => {}
    }
    let environment = LabEnvironment::new(config, Duration::from_secs(timeout_secs.max(1)))?;
    let service = environment.service();
    match arguments.command {
        WorkerLabSubcommand::Deploy(arguments) => {
            let status = service.deploy(build_lab_deploy_request(arguments)?).await?;
            render_lab_status(output_format, &status)
        }
        WorkerLabSubcommand::List => {
            let statuses = service.list().await?;
            render_lab_statuses(output_format, &statuses)
        }
        WorkerLabSubcommand::Status(arguments) => {
            let status = service.status(&arguments.name).await?;
            render_lab_status(output_format, &status)
        }
        WorkerLabSubcommand::Logs(arguments) => {
            if let Some(sink) = lab_log_sink {
                service
                    .stream_logs(
                        &arguments.name,
                        arguments.follow,
                        arguments.since,
                        arguments.tail,
                        |line| sink(&line),
                    )
                    .await?;
                return Ok(Execution {
                    output: String::new(),
                    exit_code: EXIT_SUCCESS,
                });
            }
            let lines = service
                .logs(
                    &arguments.name,
                    arguments.follow,
                    arguments.since,
                    arguments.tail,
                )
                .await?;
            render_lab_logs(output_format, &lines)
        }
        WorkerLabSubcommand::Infer(arguments) => {
            let response = service
                .infer(&LabInferRequest {
                    name: arguments.name,
                    kind: arguments.kind.into(),
                    prompt: arguments.prompt,
                    max_tokens: arguments.max_tokens,
                })
                .await?;
            render_lab_inference(output_format, &response)
        }
        WorkerLabSubcommand::Stop(arguments) => {
            let status = service.stop(&arguments.name).await?;
            render_lab_status(output_format, &status)
        }
        WorkerLabSubcommand::Remove(arguments) => {
            let status = service.remove(&arguments.name).await?;
            render_lab_status(output_format, &status)
        }
        WorkerLabSubcommand::Cleanup(arguments) => {
            let plan = service.cleanup(arguments.execute).await?;
            render_lab_cleanup(output_format, &plan)
        }
    }
}

fn build_lab_deploy_request(arguments: LabDeployArgs) -> Result<LabDeployRequest, CliError> {
    let extra_args = arguments
        .extra_args
        .into_iter()
        .map(|argument| {
            let (flag, value) = argument
                .split_once('=')
                .map_or((argument.as_str(), None), |(flag, value)| {
                    (flag, Some(value.to_string()))
                });
            if !flag.starts_with("--") {
                return Err(CliError::InvalidArgument(
                    "--vllm-extra-arg must contain --flag or --flag=value".to_string(),
                ));
            }
            Ok(VllmExtraArg {
                flag: flag.to_string(),
                value,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LabDeployRequest {
        name: arguments.name,
        model_path: arguments.model_path,
        image: arguments.image,
        gpu: arguments.gpu,
        port: arguments.port,
        served_model_name: arguments.served_model_name,
        vllm: VllmOptions {
            max_model_len: arguments.max_model_len,
            gpu_memory_utilization: arguments.gpu_memory_utilization,
            dtype: arguments.dtype.map(Into::into),
            trust_remote_code: arguments.trust_remote_code,
            enforce_eager: arguments.enforce_eager,
            extra_args,
        },
        environment: Default::default(),
    })
}

fn render_lab_status(
    output_format: OutputFormat,
    status: &LabStatus,
) -> Result<Execution, CliError> {
    if output_format == OutputFormat::Json {
        return Ok(Execution {
            output: to_json(status)?,
            exit_code: EXIT_SUCCESS,
        });
    }
    render_lab_statuses(output_format, std::slice::from_ref(status))
}

fn render_lab_statuses(
    output_format: OutputFormat,
    statuses: &[LabStatus],
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(&statuses)?,
        OutputFormat::Table => {
            let mut output =
                String::from("MODE\tNAME\tSTATE\tCONTAINER\tGPU\tENDPOINT\tSPEC_HASH\tERROR\n");
            for status in statuses {
                writeln!(
                    output,
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    status.mode,
                    status.name,
                    lab_state_name(status.state),
                    status.container_id.as_deref().unwrap_or("none"),
                    status.gpu_uuid,
                    status.endpoint,
                    status.spec_hash,
                    status.last_error.as_deref().unwrap_or("none")
                )
                .expect("write to String");
            }
            output
        }
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn render_lab_logs(
    output_format: OutputFormat,
    lines: &[LabLogLine],
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(&lines)?,
        OutputFormat::Table => lines
            .iter()
            .map(|line| format!("[{}] {}", line.stream, line.message))
            .collect(),
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn render_lab_inference(
    output_format: OutputFormat,
    response: &LabInferenceResponse,
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(response)?,
        OutputFormat::Table => format!(
            "mode: {}\nlab_id: {}\nendpoint: {}\nresponse: {}\n",
            response.mode,
            response.lab_id,
            response.endpoint,
            serde_json::to_string_pretty(&response.response)?
        ),
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn render_lab_cleanup(
    output_format: OutputFormat,
    plan: &LabCleanupPlan,
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(plan)?,
        OutputFormat::Table => format!(
            concat!(
                "mode: {}\n",
                "dry_run: {}\n",
                "deployments: {}\n",
                "containers: {}\n",
                "caches: {}\n",
                "blocked: {}\n",
                "removed_deployments: {}\n",
                "removed_containers: {}\n",
                "removed_caches: {}\n"
            ),
            plan.mode,
            plan.dry_run,
            plan.deployment_ids.join(","),
            plan.container_ids.join(","),
            plan.cache_ids.join(","),
            plan.blocked.join("; "),
            plan.removed_deployments,
            plan.removed_containers,
            plan.removed_caches,
        ),
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn lab_state_name(state: LabState) -> &'static str {
    match state {
        LabState::Created => "CREATED",
        LabState::Preparing => "PREPARING",
        LabState::Starting => "STARTING",
        LabState::Running => "RUNNING",
        LabState::Failed => "FAILED",
        LabState::Stopped => "STOPPED",
    }
}

fn render_public_config(output_format: OutputFormat) -> Result<Execution, CliError> {
    let config = public_config_view();
    let output = match output_format {
        OutputFormat::Json => to_json(&config)?,
        OutputFormat::Table => {
            let value = serde_json::to_value(config)?;
            let object = value.as_object().expect("config view serializes as object");
            let mut output = String::new();
            for (name, value) in object {
                writeln!(output, "{name}: {value}").expect("write to String");
            }
            output
        }
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn render_gpu_inventory(
    output_format: OutputFormat,
    gpus: &[gpustack_client::types::GpuDeviceInfo],
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(&gpus)?,
        OutputFormat::Table => {
            let mut output = String::from("INDEX\tNAME\tUUID\tMEMORY_TOTAL_BYTES\n");
            for gpu in gpus {
                writeln!(
                    output,
                    "{}\t{}\t{}\t{}",
                    gpu.device_index,
                    gpu.name,
                    gpu.uuid.as_deref().unwrap_or("unknown"),
                    gpu.memory
                        .as_ref()
                        .and_then(|memory| memory.total)
                        .map_or_else(|| "unknown".to_owned(), |value| value.to_string())
                )
                .expect("write to String");
            }
            output
        }
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn render_runtime_info(
    output_format: OutputFormat,
    info: &DockerDaemonInfo,
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(info)?,
        OutputFormat::Table => format!(
            concat!(
                "ping: {}\n",
                "version: {}\n",
                "api_version: {}\n",
                "min_api_version: {}\n",
                "os: {}\n",
                "arch: {}\n"
            ),
            info.ping,
            info.version,
            info.api_version,
            info.min_api_version.as_deref().unwrap_or("unknown"),
            info.os.as_deref().unwrap_or("unknown"),
            info.arch.as_deref().unwrap_or("unknown"),
        ),
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

#[derive(Serialize)]
struct VersionInfo<'a> {
    name: &'a str,
    version: &'a str,
    git_commit: &'a str,
    target: String,
    rust_toolchain: &'a str,
}

fn render_version(output_format: OutputFormat) -> Result<Execution, CliError> {
    let info = VersionInfo {
        name: "gpustack-rs",
        version: env!("CARGO_PKG_VERSION"),
        git_commit: option_env!("GPUSTACK_GIT_COMMIT").unwrap_or("unknown"),
        target: option_env!("GPUSTACK_BUILD_TARGET").map_or_else(
            || format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            str::to_owned,
        ),
        rust_toolchain: "1.99.1",
    };
    let output = match output_format {
        OutputFormat::Json => to_json(&info)?,
        OutputFormat::Table => format!(
            "name: {}\nversion: {}\ngit_commit: {}\ntarget: {}\nrust_toolchain: {}\n",
            info.name, info.version, info.git_commit, info.target, info.rust_toolchain
        ),
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

#[derive(Serialize)]
struct ConfigValidation<'a> {
    valid: bool,
    source: Option<String>,
    config: &'a WorkerConfig,
}

fn render_valid_config(
    output_format: OutputFormat,
    config: &WorkerConfig,
    source: Option<&std::path::Path>,
) -> Result<Execution, CliError> {
    let source = source.map(|path| path.display().to_string());
    let result = ConfigValidation {
        valid: true,
        source,
        config,
    };
    let output = match output_format {
        OutputFormat::Json => to_json(&result)?,
        OutputFormat::Table => render_config_table(&result),
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn render_config_table(result: &ConfigValidation<'_>) -> String {
    let config = result.config;
    let mut output = String::new();
    writeln!(output, "valid: true").expect("write to String");
    writeln!(
        output,
        "source: {}",
        result.source.as_deref().unwrap_or("defaults")
    )
    .expect("write to String");
    writeln!(
        output,
        "server_url: {}",
        config
            .server_url
            .as_ref()
            .map_or("not configured", |url| url.as_str())
    )
    .expect("write to String");
    writeln!(
        output,
        "worker_name: {}",
        config.worker_name.as_deref().unwrap_or("not configured")
    )
    .expect("write to String");
    writeln!(
        output,
        "worker_hostname: {}",
        config
            .worker_hostname
            .as_deref()
            .unwrap_or("not configured")
    )
    .expect("write to String");
    writeln!(
        output,
        "worker_ifname: {}",
        config.worker_ifname.as_deref().unwrap_or("not configured")
    )
    .expect("write to String");
    writeln!(
        output,
        "worker_ip: {}",
        config
            .worker_ip
            .map_or_else(|| "not configured".to_owned(), |ip| ip.to_string())
    )
    .expect("write to String");
    writeln!(output, "worker_port: {}", config.worker_port).expect("write to String");
    writeln!(output, "metrics_port: {}", config.metrics_port).expect("write to String");
    writeln!(output, "data_dir: {}", config.data_dir.display()).expect("write to String");
    writeln!(output, "log_dir: {}", config.log_dir.display()).expect("write to String");
    writeln!(output, "docker_socket: {}", config.docker_socket.display()).expect("write to String");
    writeln!(output, "model_roots: {}", config.model_roots.len()).expect("write to String");
    writeln!(
        output,
        "token_file: {}",
        config.token_file.as_ref().map_or_else(
            || "not configured".to_owned(),
            |path| path.display().to_string()
        )
    )
    .expect("write to String");
    output
}

fn render_doctor(
    output_format: OutputFormat,
    report: &DoctorReport,
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(report)?,
        OutputFormat::Table => {
            let mut output = format!("healthy: {}\n", report.healthy);
            for check in &report.checks {
                writeln!(
                    output,
                    "{:?}\t{}\t{}",
                    check.status, check.name, check.message
                )
                .expect("write to String");
            }
            output
        }
    };
    Ok(Execution {
        output,
        exit_code: if report.healthy {
            EXIT_SUCCESS
        } else {
            EXIT_RUNTIME
        },
    })
}

fn render_control_plane_summary(
    output_format: OutputFormat,
    summary: &ControlPlaneOnlySummary,
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(summary)?,
        OutputFormat::Table => format!(
            concat!(
                "mode: {}\n",
                "server_version: {}\n",
                "server_git_commit: {}\n",
                "worker_id: {}\n",
                "worker_name: {}\n",
                "worker_uuid: {}\n",
                "identity_source: {}\n",
                "health_address: {}\n",
                "shutdown_signal: {}\n",
                "desired_models: {}\n",
                "desired_model_instances: {}\n",
                "desired_model_files: {}\n",
                "desired_revision: {}\n",
                "heartbeat_ok: {}\n",
                "status_ok: {}\n",
                "full_sync_ok: {}\n",
                "watch_connected: {}\n",
                "reconnect_attempts: {}\n"
            ),
            summary.mode,
            summary.server_version,
            summary.server_git_commit,
            summary.worker_id,
            summary.worker_name,
            summary.worker_uuid,
            summary.identity_source,
            summary.health_address,
            summary.shutdown_signal,
            summary.desired_models,
            summary.desired_model_instances,
            summary.desired_model_files,
            summary.desired_revision,
            summary.heartbeat_ok,
            summary.status_ok,
            summary.full_sync_ok,
            summary.watch_connected,
            summary.reconnect_attempts,
        ),
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn render_resource_api_summary(
    output_format: OutputFormat,
    summary: &ResourceApiSummary,
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(summary)?,
        OutputFormat::Table => format!(
            concat!(
                "mode: {}\n",
                "server_version: {}\n",
                "server_git_commit: {}\n",
                "worker_id: {}\n",
                "worker_name: {}\n",
                "worker_uuid: {}\n",
                "worker_address: {}\n",
                "metrics_address: {}\n",
                "gpu_count: {}\n",
                "shutdown_signal: {}\n",
                "heartbeat_ok: {}\n",
                "status_ok: {}\n",
                "full_sync_ok: {}\n",
                "watch_connected: {}\n",
                "reconnect_attempts: {}\n"
            ),
            summary.mode,
            summary.server_version,
            summary.server_git_commit,
            summary.worker_id,
            summary.worker_name,
            summary.worker_uuid,
            summary.worker_address,
            summary.metrics_address,
            summary.gpu_count,
            summary.shutdown_signal,
            summary.heartbeat_ok,
            summary.status_ok,
            summary.full_sync_ok,
            summary.watch_connected,
            summary.reconnect_attempts,
        ),
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn render_managed_file_summary(
    output_format: OutputFormat,
    summary: &ManagedFileSummary,
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(summary)?,
        OutputFormat::Table => format!(
            concat!(
                "mode: {}\n",
                "server_version: {}\n",
                "server_git_commit: {}\n",
                "worker_id: {}\n",
                "worker_name: {}\n",
                "worker_uuid: {}\n",
                "worker_address: {}\n",
                "metrics_address: {}\n",
                "gpu_count: {}\n",
                "shutdown_signal: {}\n",
                "desired_models: {}\n",
                "desired_model_instances: {}\n",
                "desired_model_files: {}\n",
                "desired_revision: {}\n",
                "reconciliations: {}\n",
                "initialized_instances: {}\n",
                "errored_instances: {}\n",
                "ready_model_files: {}\n",
                "errored_model_files: {}\n",
                "noops: {}\n",
                "failures: {}\n",
                "heartbeat_ok: {}\n",
                "status_ok: {}\n",
                "full_sync_ok: {}\n",
                "watch_connected: {}\n",
                "reconnect_attempts: {}\n"
            ),
            summary.mode,
            summary.server_version,
            summary.server_git_commit,
            summary.worker_id,
            summary.worker_name,
            summary.worker_uuid,
            summary.worker_address,
            summary.metrics_address,
            summary.gpu_count,
            summary.shutdown_signal,
            summary.desired_models,
            summary.desired_model_instances,
            summary.desired_model_files,
            summary.desired_revision,
            summary.reconciliations,
            summary.initialized_instances,
            summary.errored_instances,
            summary.ready_model_files,
            summary.errored_model_files,
            summary.noops,
            summary.failures,
            summary.heartbeat_ok,
            summary.status_ok,
            summary.full_sync_ok,
            summary.watch_connected,
            summary.reconnect_attempts,
        ),
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn render_managed_runtime_summary(
    output_format: OutputFormat,
    summary: &ManagedRuntimeSummary,
) -> Result<Execution, CliError> {
    let output = match output_format {
        OutputFormat::Json => to_json(summary)?,
        OutputFormat::Table => {
            let mut output = render_managed_file_summary(output_format, &summary.file)?.output;
            write!(
                output,
                concat!(
                    "runtime_reconciliations: {}\n",
                    "runtime_starting: {}\n",
                    "runtime_running: {}\n",
                    "runtime_errored: {}\n",
                    "runtime_deleted: {}\n",
                    "runtime_noops: {}\n",
                    "runtime_failures: {}\n"
                ),
                summary.runtime_reconciliations,
                summary.runtime_starting,
                summary.runtime_running,
                summary.runtime_errored,
                summary.runtime_deleted,
                summary.runtime_noops,
                summary.runtime_failures,
            )
            .expect("writing to String cannot fail");
            output
        }
    };
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

fn to_json(value: &impl Serialize) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value).map(|output| format!("{output}\n"))
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn command_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn managed_failures_keep_stable_exit_code_categories() {
        let invalid = CliError::Managed(ManagedError::InvalidArgument("bad input".to_owned()));
        let missing = CliError::Managed(ManagedError::NotFound("Model", "missing".to_owned()));
        let client = CliError::Managed(ManagedError::Client(ClientError::Contract(
            "unexpected response".to_owned(),
        )));

        assert_eq!(invalid.exit_code(), EXIT_CONFIG);
        assert_eq!(missing.exit_code(), EXIT_CONTROL_PLANE);
        assert_eq!(client.exit_code(), EXIT_CONTROL_PLANE);
    }

    #[tokio::test]
    async fn command_specific_global_options_are_not_silently_ignored() {
        for command in [
            vec!["gpustack-rs", "--context", "unused", "context", "list"],
            vec!["gpustack-rs", "--config", "/unused", "model", "list"],
            vec![
                "gpustack-rs",
                "--context",
                "unused",
                "worker",
                "gpu",
                "list",
            ],
            vec![
                "gpustack-rs",
                "worker",
                "maintenance",
                "enable",
                "target",
                "--server-url",
                "http://127.0.0.1",
            ],
        ] {
            let cli = Cli::try_parse_from(command).expect("parse CLI");
            let error = execute(cli).await.expect_err("irrelevant option rejected");
            assert!(matches!(error, CliError::InvalidArgument(_)));
            assert_eq!(error.exit_code(), EXIT_CONFIG);
        }
    }

    #[tokio::test]
    async fn worker_config_get_returns_only_the_public_whitelist() {
        let cli =
            Cli::try_parse_from(["gpustack-rs", "--output", "json", "worker", "config", "get"])
                .expect("parse CLI");

        let execution = execute(cli).await.expect("execute config get");

        assert!(execution.output.contains("disable_builtin_observability"));
        assert!(!execution.output.contains("token"));
        assert!(!execution.output.contains("docker"));
    }

    #[test]
    fn worker_gpu_list_and_resource_mode_are_explicit_commands() {
        Cli::try_parse_from(["gpustack-rs", "worker", "gpu", "list"]).expect("parse GPU list");
        Cli::try_parse_from(["gpustack-rs", "worker", "run", "--resource-api-only"])
            .expect("parse resource mode");
        assert!(
            Cli::try_parse_from([
                "gpustack-rs",
                "worker",
                "run",
                "--control-plane-only",
                "--resource-api-only",
            ])
            .is_err()
        );
    }

    #[test]
    fn worker_runtime_info_has_stable_offline_rendering() {
        Cli::try_parse_from(["gpustack-rs", "worker", "runtime", "info"])
            .expect("parse runtime info");
        let info = DockerDaemonInfo {
            ping: "OK".to_string(),
            version: "29.1.3".to_string(),
            api_version: "1.52".to_string(),
            min_api_version: Some("1.24".to_string()),
            os: Some("linux".to_string()),
            arch: Some("amd64".to_string()),
        };

        let execution =
            render_runtime_info(OutputFormat::Json, &info).expect("render runtime info");
        let value: serde_json::Value =
            serde_json::from_str(&execution.output).expect("parse runtime info");

        assert_eq!(value["ping"], "OK");
        assert_eq!(value["version"], "29.1.3");
        assert_eq!(value["api_version"], "1.52");
        assert_eq!(value["os"], "linux");
        assert_eq!(value["arch"], "amd64");
    }

    #[test]
    fn complete_lab_command_surface_parses_without_touching_real_resources() {
        for command in [
            vec![
                "gpustack-rs",
                "worker",
                "lab",
                "deploy",
                "demo",
                "--model-path",
                "/models/demo",
                "--image",
                "runner@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "--gpu",
                "0",
                "--max-model-len",
                "4096",
            ],
            vec!["gpustack-rs", "worker", "lab", "list"],
            vec!["gpustack-rs", "worker", "lab", "status", "demo"],
            vec!["gpustack-rs", "worker", "lab", "logs", "demo", "--follow"],
            vec![
                "gpustack-rs",
                "worker",
                "lab",
                "infer",
                "demo",
                "--prompt",
                "hello",
            ],
            vec!["gpustack-rs", "worker", "lab", "stop", "demo"],
            vec!["gpustack-rs", "worker", "lab", "remove", "demo", "--yes"],
            vec!["gpustack-rs", "worker", "lab", "cleanup"],
            vec![
                "gpustack-rs",
                "worker",
                "lab",
                "cleanup",
                "--execute",
                "--yes",
            ],
        ] {
            Cli::try_parse_from(command).expect("parse Lab command");
        }
    }

    #[test]
    fn lab_deploy_parses_qwen_chat_template_as_one_extra_argument() {
        let template = include_str!("../../../fixtures/m5e/qwen-7b-chatml.jinja")
            .trim_end_matches(['\r', '\n']);
        let raw_extra_argument = format!("--vllm-extra-arg=--chat-template={template}");
        let cli = Cli::try_parse_from([
            "gpustack-rs",
            "worker",
            "lab",
            "deploy",
            "qwen-7b-m5e",
            "--model-path",
            "/models/Qwen-7B",
            "--image",
            "runner@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--gpu",
            "GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3",
            raw_extra_argument.as_str(),
        ])
        .expect("parse Qwen Lab deploy");
        let Command::Worker(worker) = cli.command else {
            panic!("worker command");
        };
        let WorkerCommand::Lab(lab) = worker.command else {
            panic!("Lab command");
        };
        let WorkerLabSubcommand::Deploy(arguments) = lab.command else {
            panic!("Lab deploy command");
        };

        let request = build_lab_deploy_request(arguments).expect("build Lab deploy request");

        assert_eq!(
            request.vllm.extra_args,
            vec![VllmExtraArg {
                flag: "--chat-template".to_string(),
                value: Some(template.to_string()),
            }]
        );
    }

    #[tokio::test]
    async fn destructive_lab_commands_require_explicit_confirmation_in_library_mode() {
        let cli = Cli::try_parse_from(["gpustack-rs", "worker", "lab", "remove", "demo"])
            .expect("parse remove");

        let error = execute(cli).await.expect_err("confirmation is required");

        assert!(matches!(error, CliError::ConfirmationRequired));
    }

    #[tokio::test]
    async fn version_supports_stable_json_output() {
        let cli =
            Cli::try_parse_from(["gpustack-rs", "--output", "json", "version"]).expect("parse CLI");

        let execution = execute(cli).await.expect("execute version");

        assert_eq!(execution.exit_code, EXIT_SUCCESS);
        let value: serde_json::Value =
            serde_json::from_str(&execution.output).expect("parse JSON output");
        assert_eq!(value["name"], "gpustack-rs");
        assert_eq!(value["rust_toolchain"], "1.99.1");
    }

    #[tokio::test]
    async fn worker_run_requires_an_explicit_mode() {
        let cli = Cli::try_parse_from(["gpustack-rs", "worker", "run"]).expect("parse CLI");

        let error = execute(cli).await.expect_err("restricted mode is required");

        assert!(matches!(error, CliError::WorkerModeRequired));
        assert_eq!(error.exit_code(), EXIT_CONFIG);
    }

    #[test]
    fn managed_file_only_is_explicit_and_mutually_exclusive() {
        let cli = Cli::try_parse_from(["gpustack-rs", "worker", "run", "--managed-file-only"])
            .expect("parse managed-file-only");
        let Command::Worker(worker) = cli.command else {
            panic!("worker command")
        };
        let WorkerCommand::Run(arguments) = worker.command else {
            panic!("run command")
        };
        assert!(arguments.managed_file_only);
        assert!(!arguments.control_plane_only);
        assert!(!arguments.resource_api_only);
        assert!(!arguments.managed_runtime);

        for conflicting in [
            "--control-plane-only",
            "--resource-api-only",
            "--managed-runtime",
        ] {
            assert!(
                Cli::try_parse_from([
                    "gpustack-rs",
                    "worker",
                    "run",
                    "--managed-file-only",
                    conflicting,
                ])
                .is_err(),
                "{conflicting} must conflict with --managed-file-only"
            );
        }
    }

    #[test]
    fn managed_runtime_is_explicit_and_mutually_exclusive() {
        let cli = Cli::try_parse_from(["gpustack-rs", "worker", "run", "--managed-runtime"])
            .expect("parse managed-runtime");
        let Command::Worker(worker) = cli.command else {
            panic!("worker command")
        };
        let WorkerCommand::Run(arguments) = worker.command else {
            panic!("run command")
        };
        assert!(arguments.managed_runtime);
        assert!(!arguments.control_plane_only);
        assert!(!arguments.resource_api_only);
        assert!(!arguments.managed_file_only);

        for conflicting in [
            "--control-plane-only",
            "--resource-api-only",
            "--managed-file-only",
        ] {
            assert!(
                Cli::try_parse_from([
                    "gpustack-rs",
                    "worker",
                    "run",
                    "--managed-runtime",
                    conflicting,
                ])
                .is_err(),
                "{conflicting} must conflict with --managed-runtime"
            );
        }
    }
}
