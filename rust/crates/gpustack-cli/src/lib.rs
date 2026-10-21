//! CLI grammar, stable exit codes, and output rendering for `gpustack-rs`.

use std::fmt::Write as _;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use gpustack_worker::resources::{ResourceError, collect_gpu_inventory};
use gpustack_worker::worker_api::public_config_view;
use gpustack_worker::{
    ConfigError, ConfigOverrides, ControlPlaneOnlySummary, DoctorReport, ResourceApiRunError,
    ResourceApiSummary, WorkerConfig, WorkerRunError, load_config, run_control_plane_only,
    run_doctor, run_resource_api_only,
};
use serde::Serialize;
use thiserror::Error;

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_INTERNAL: u8 = 1;
pub const EXIT_CONFIG: u8 = 10;
pub const EXIT_CONTROL_PLANE: u8 = 30;
pub const EXIT_RUNTIME: u8 = 50;

#[derive(Debug, Parser)]
#[command(name = "gpustack-rs", version, about = "GPUStack Rust migration CLI")]
pub struct Cli {
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,
    #[arg(long, global = true)]
    pub quiet: bool,
    #[arg(long, global = true)]
    pub no_color: bool,
    #[arg(long, global = true, default_value_t = 30)]
    pub timeout: u64,
    #[arg(long, global = true)]
    pub context: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Version,
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

#[derive(Debug, Subcommand)]
pub enum WorkerCommand {
    Config(WorkerConfigCommand),
    Doctor,
    Gpu(WorkerGpuCommand),
    Run(WorkerRunArgs),
    ValidateConfig,
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
    #[arg(long, conflicts_with = "resource_api_only")]
    pub control_plane_only: bool,
    #[arg(long, conflicts_with = "control_plane_only")]
    pub resource_api_only: bool,
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
    #[error("worker run currently requires --control-plane-only")]
    ControlPlaneOnlyRequired,
    #[error(transparent)]
    Resource(#[from] ResourceError),
    #[error(transparent)]
    ResourceRun(#[from] ResourceApiRunError),
    #[error(transparent)]
    WorkerRun(#[from] WorkerRunError),
    #[error("failed to serialize command output")]
    Serialization(#[from] serde_json::Error),
}

impl CliError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Config(_) | Self::ControlPlaneOnlyRequired => EXIT_CONFIG,
            Self::WorkerRun(_) => EXIT_CONTROL_PLANE,
            Self::Resource(_) => EXIT_RUNTIME,
            Self::ResourceRun(_) => EXIT_CONTROL_PLANE,
            Self::Serialization(_) => EXIT_INTERNAL,
        }
    }
}

/// Executes an already parsed command without directly accessing process streams.
pub async fn execute(cli: Cli) -> Result<Execution, CliError> {
    let quiet = cli.quiet;
    let output_format = cli.output;
    let execution = match cli.command {
        Command::Version => render_version(output_format)?,
        Command::Worker(worker) => {
            let loaded = load_config(cli.config.as_deref(), worker.overrides.into())?;
            match worker.command {
                WorkerCommand::Config(arguments) => match arguments.command {
                    WorkerConfigSubcommand::Get => render_public_config(output_format)?,
                },
                WorkerCommand::ValidateConfig => {
                    render_valid_config(output_format, &loaded.config, loaded.source.as_deref())?
                }
                WorkerCommand::Doctor => render_doctor(output_format, &run_doctor(&loaded.config))?,
                WorkerCommand::Gpu(arguments) => match arguments.command {
                    WorkerGpuSubcommand::List => {
                        render_gpu_inventory(output_format, &collect_gpu_inventory()?)?
                    }
                },
                WorkerCommand::Run(arguments) => {
                    if !arguments.control_plane_only && !arguments.resource_api_only {
                        return Err(CliError::ControlPlaneOnlyRequired);
                    }
                    if arguments.resource_api_only {
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
    async fn worker_run_requires_explicit_control_plane_only_mode() {
        let cli = Cli::try_parse_from(["gpustack-rs", "worker", "run"]).expect("parse CLI");

        let error = execute(cli).await.expect_err("restricted mode is required");

        assert!(matches!(error, CliError::ControlPlaneOnlyRequired));
        assert_eq!(error.exit_code(), EXIT_CONFIG);
    }
}
