//! CLI grammar, stable exit codes, and output rendering for `gpustack-rs`.

use std::fmt::Write as _;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use gpustack_worker::{
    ConfigError, ConfigOverrides, DoctorReport, WorkerConfig, load_config, run_doctor,
};
use serde::Serialize;
use thiserror::Error;

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_INTERNAL: u8 = 1;
pub const EXIT_CONFIG: u8 = 10;
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
    Worker(WorkerArgs),
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
    pub worker_ip: Option<String>,
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
            worker_ip: value.worker_ip,
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
    Doctor,
    ValidateConfig,
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
    #[error("failed to serialize command output")]
    Serialization(#[from] serde_json::Error),
}

impl CliError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Config(_) => EXIT_CONFIG,
            Self::Serialization(_) => EXIT_INTERNAL,
        }
    }
}

/// Executes an already parsed command without directly accessing process streams.
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
