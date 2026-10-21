//! Read-only checks for local Worker deployment prerequisites.

use std::path::Path;

use gpustack_runtime::{CheckStatus, DiagnosticCheck, PathKind, inspect_path};
use serde::Serialize;

use crate::config::WorkerConfig;

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub healthy: bool,
    pub checks: Vec<DiagnosticCheck>,
}

#[must_use]
/// Checks local prerequisites without contacting Docker Engine or Server.
pub fn run_doctor(config: &WorkerConfig) -> DoctorReport {
    let mut checks = Vec::new();
    checks.push(match &config.server_url {
        Some(url) => DiagnosticCheck::pass(
            "server_url",
            format!("configured with {} scheme", url.scheme()),
        ),
        None => DiagnosticCheck::warn("server_url", "not configured"),
    });
    checks.push(inspect_path(
        "docker_socket",
        &config.docker_socket,
        PathKind::UnixSocket,
        true,
    ));
    checks.push(inspect_or_defer_directory("data_dir", &config.data_dir));
    checks.push(inspect_or_defer_directory("log_dir", &config.log_dir));

    if config.model_roots.is_empty() {
        checks.push(DiagnosticCheck::warn(
            "model_roots",
            "no local model root is configured",
        ));
    } else {
        checks.extend(config.model_roots.iter().enumerate().map(|(index, path)| {
            inspect_path(
                &format!("model_root[{index}]"),
                path,
                PathKind::Directory,
                true,
            )
        }));
    }

    if let Some(token_file) = &config.token_file {
        checks.push(inspect_path("token_file", token_file, PathKind::File, true));
        checks.push(check_token_permissions(token_file));
    } else {
        checks.push(DiagnosticCheck::warn(
            "token_file",
            "not configured; Managed mode will require a credential file",
        ));
    }

    let healthy = checks.iter().all(|check| check.status != CheckStatus::Fail);
    DoctorReport { healthy, checks }
}

fn inspect_or_defer_directory(name: &str, path: &Path) -> DiagnosticCheck {
    if path.exists() {
        inspect_path(name, path, PathKind::Directory, true)
    } else {
        DiagnosticCheck::warn(
            name,
            format!(
                "{} does not exist yet; the Worker will create it",
                path.display()
            ),
        )
    }
}

#[cfg(unix)]
fn check_token_permissions(path: &Path) -> DiagnosticCheck {
    use std::os::unix::fs::PermissionsExt;

    match path.metadata() {
        Ok(metadata) if metadata.permissions().mode() & 0o077 == 0 => DiagnosticCheck::pass(
            "token_file_permissions",
            "credential file permissions are restricted",
        ),
        Ok(_) => DiagnosticCheck::fail(
            "token_file_permissions",
            "credential file must not grant permissions to group or other users",
        ),
        Err(error) => DiagnosticCheck::fail(
            "token_file_permissions",
            format!(
                "cannot inspect credential file permissions: {}",
                error.kind()
            ),
        ),
    }
}

#[cfg(not(unix))]
fn check_token_permissions(_path: &Path) -> DiagnosticCheck {
    DiagnosticCheck::warn(
        "token_file_permissions",
        "credential permission validation is only supported on Unix",
    )
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::net::UnixListener;

    use super::*;

    #[test]
    fn doctor_accepts_available_local_prerequisites() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let socket_path = directory.path().join("docker.sock");
        let _listener = UnixListener::bind(&socket_path).expect("bind test socket");
        let config = WorkerConfig {
            server_url: None,
            worker_name: None,
            worker_hostname: None,
            worker_ip: None,
            worker_ifname: None,
            worker_port: crate::config::DEFAULT_WORKER_PORT,
            metrics_port: crate::config::DEFAULT_METRICS_PORT,
            token_file: None,
            data_dir: directory.path().join("data"),
            log_dir: directory.path().join("logs"),
            model_roots: vec![directory.path().to_path_buf()],
            docker_socket: socket_path,
        };

        let report = run_doctor(&config);

        assert!(report.healthy);
        assert!(
            report
                .checks
                .iter()
                .all(|check| check.status != CheckStatus::Fail)
        );
    }
}
