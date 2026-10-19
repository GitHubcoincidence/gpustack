//! CLI grammar, stable exit codes, and output rendering for `gpustack-rs`.

use std::fmt::Write as _;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use gpustack_worker::{
    ConfigError, ConfigOverrides, DoctorReport, WorkerConfig, load_config, run_doctor,
};
use serde::Serialize;
use thiserror::Error;

/// Successful command exit status.
pub const EXIT_SUCCESS: u8 = 0;
/// Unexpected internal or serialization failure exit status.
pub const EXIT_INTERNAL: u8 = 1;
/// Invalid or unusable configuration exit status.
pub const EXIT_CONFIG: u8 = 10;
/// Local runtime prerequisite failure exit status.
pub const EXIT_RUNTIME: u8 = 50;

/// Top-level command-line arguments shared by every subcommand.
#[derive(Debug, Parser)]
#[command(name = "gpustack-rs", version, about = "GPUStack Rust migration CLI")]
pub struct Cli {
    /// Optional YAML configuration path.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    /// Stable human-readable or JSON output format.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,
    /// Suppresses successful output while retaining errors and exit status.
    #[arg(long, global = true)]
    pub quiet: bool,
    /// Disables colored output for automation compatibility.
    #[arg(long, global = true)]
    pub no_color: bool,
    /// Default operation timeout in seconds.
    #[arg(long, global = true, default_value_t = 30)]
    pub timeout: u64,
    /// Named Server context selected by the caller.
    #[arg(long, global = true)]
    pub context: Option<String>,
    /// Operation selected by the caller.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level CLI command families.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Prints binary and toolchain identity.
    Version,
    /// Runs Worker-specific commands.
    Worker(WorkerArgs),
}

/// Worker configuration overrides and selected operation.
#[derive(Debug, Args)]
pub struct WorkerArgs {
    /// Configuration values supplied directly on the command line.
    #[command(flatten)]
    pub overrides: WorkerConfigArgs,
    /// Worker operation to execute.
    #[command(subcommand)]
    pub command: WorkerCommand,
}

/// CLI configuration values that override environment, YAML, and defaults.
#[derive(Debug, Args, Default)]
pub struct WorkerConfigArgs {
    /// GPUStack Server base URL.
    #[arg(long, global = true)]
    pub server_url: Option<String>,
    /// Worker identity name presented to Server.
    #[arg(long, global = true)]
    pub worker_name: Option<String>,
    /// Worker address presented to Server.
    #[arg(long, global = true)]
    pub worker_ip: Option<String>,
    /// Absolute path to the Worker credential file.
    #[arg(long, global = true)]
    pub token_file: Option<PathBuf>,
    /// Persistent Worker state directory.
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
    /// Worker log directory.
    #[arg(long, global = true)]
    pub log_dir: Option<PathBuf>,
    /// Local model directories visible to inference containers.
    #[arg(long = "model-root", global = true)]
    pub model_roots: Vec<PathBuf>,
    /// Docker Engine Unix socket path.
    #[arg(long, global = true)]
    pub docker_socket: Option<PathBuf>,
}

impl From<WorkerConfigArgs> for ConfigOverrides {
    fn from(value: WorkerConfigArgs) -> Self {
        Self {
            server_url: value.server_url,
            worker_name: value.worker_name,
            worker_ip: value.worker_ip,
            token_file: value.token_file,
            data_dir: value.data_dir,
            log_dir: value.log_dir,
            model_roots: (!value.model_roots.is_empty()).then_some(value.model_roots),
            docker_socket: value.docker_socket,
        }
    }
}

/// Worker commands implemented by the migration CLI.
#[derive(Debug, Subcommand)]
pub enum WorkerCommand {
    /// Performs read-only local prerequisite checks.
    Doctor,
    /// Loads, merges, and validates configuration.
    ValidateConfig,
}

/// Output encoding selected for a command result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable key/value or row output.
    Table,
    /// Machine-readable JSON output.
    Json,
}

/// Rendered command output paired with its stable process exit code.
#[derive(Debug)]
pub struct Execution {
    /// Complete standard-output payload.
    pub output: String,
    /// Process exit status defined by this crate's public constants.
    pub exit_code: u8,
}

/// Failures that occur before a command can produce an `Execution`.
#[derive(Debug, Error)]
pub enum CliError {
    /// Configuration loading or validation failed.
    #[error("{0}")]
    Config(#[from] ConfigError),
    /// A structured command result could not be serialized.
    #[error("failed to serialize command output")]
    Serialization(#[from] serde_json::Error),
}

impl CliError {
    /// Maps a CLI failure to the stable process exit-code contract.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Config(_) => EXIT_CONFIG,
            Self::Serialization(_) => EXIT_INTERNAL,
        }
    }
}

/// Executes a parsed command without directly reading or writing process streams.
pub fn execute(cli: Cli) -> Result<Execution, CliError> {
    let quiet = cli.quiet;
    let output_format = cli.output;
    let execution = match cli.command {
        Command::Version => render_version(output_format)?,
        Command::Worker(worker) => {
            let loaded = load_config(cli.config.as_deref(), worker.overrides.into())?;
            match worker.command {
                WorkerCommand::ValidateConfig => {
                    render_valid_config(output_format, &loaded.config, loaded.source.as_deref())?
                }
                WorkerCommand::Doctor => render_doctor(output_format, &run_doctor(&loaded.config))?,
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
    fn version_supports_stable_json_output() {
        let cli =
            Cli::try_parse_from(["gpustack-rs", "--output", "json", "version"]).expect("parse CLI");

        let execution = execute(cli).expect("execute version");

        assert_eq!(execution.exit_code, EXIT_SUCCESS);
        let value: serde_json::Value =
            serde_json::from_str(&execution.output).expect("parse JSON output");
        assert_eq!(value["name"], "gpustack-rs");
        assert_eq!(value["rust_toolchain"], "1.99.1");
    }
}
