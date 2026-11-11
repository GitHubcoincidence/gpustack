use std::env;
use std::path::Path;
use std::process::ExitCode;

#[path = "support/openapi_audit.rs"]
mod openapi_audit;

fn main() -> ExitCode {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let Some(path) = arguments.next() else {
        eprintln!("usage: m8rs_openapi_audit <local-openapi.json>");
        return ExitCode::from(1);
    };
    if arguments.next().is_some() {
        eprintln!("usage: m8rs_openapi_audit <local-openapi.json>");
        return ExitCode::from(1);
    }

    match openapi_audit::audit_file(Path::new(&path)) {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(output) => {
                println!("{output}");
                ExitCode::from(report.exit_code())
            }
            Err(error) => {
                eprintln!("failed to serialize the audit report: {error}");
                ExitCode::from(1)
            }
        },
        Err(error) => {
            eprintln!("OpenAPI audit failed: {error}");
            ExitCode::from(1)
        }
    }
}
