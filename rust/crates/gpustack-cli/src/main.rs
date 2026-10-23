//! Process entry point for the transitional `gpustack-rs` CLI.

use std::process::ExitCode;

use clap::Parser;
use gpustack_cli::{Cli, OutputFormat, execute_with_lab_log_sink};

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let mut cli = Cli::parse();
    if let Err(error) = cli.confirm_destructive_command() {
        eprintln!("error: {error}");
        return ExitCode::from(error.exit_code());
    }
    let output_format = cli.output;
    let quiet = cli.quiet;
    let mut log_sink = |line: &gpustack_worker::lab::LabLogLine| {
        if quiet {
            return;
        }
        match output_format {
            OutputFormat::Json => {
                if let Ok(encoded) = serde_json::to_string(line) {
                    println!("{encoded}");
                }
            }
            OutputFormat::Table => {
                println!("[{}] {}", line.stream, line.message.trim_end());
            }
        }
        let _ = std::io::Write::flush(&mut std::io::stdout());
    };
    match execute_with_lab_log_sink(cli, &mut log_sink).await {
        Ok(execution) => {
            print!("{}", execution.output);
            ExitCode::from(execution.exit_code)
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
