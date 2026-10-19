//! Process entry point for the transitional `gpustack-rs` CLI.

use std::process::ExitCode;

use clap::Parser;
use gpustack_cli::{Cli, execute};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match execute(cli) {
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
