//! Process entry point for the transitional `gpustack-rs` CLI.

use std::process::ExitCode;

use clap::Parser;
use gpustack_cli::{Cli, OutputFormat, execute_with_output_sinks};

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
    let mut server_stream_sink = |chunk: &[u8]| {
        if !quiet {
            let mut stdout = std::io::stdout().lock();
            let _ = std::io::Write::write_all(&mut stdout, chunk);
            let _ = std::io::Write::flush(&mut stdout);
        }
    };
    match execute_with_output_sinks(cli, &mut log_sink, &mut server_stream_sink).await {
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
