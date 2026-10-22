use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use futures_util::StreamExt;
use gpustack_runtime::docker::{
    ContainerDetails, ContainerLogOptions, ContainerRestartPolicy, ContainerRuntime, ContainerSpec,
    ContainerState, DockerDaemonInfo, DockerRuntime, ImageIdentity, RuntimeError,
};
use gpustack_worker::workload::{
    LABEL_COMPONENT, LABEL_IMPLEMENTATION, LABEL_MODE, LABEL_OWNER_ID, LABEL_SCHEMA_VERSION,
    LABEL_SPEC_HASH, OwnershipError, WORKLOAD_SCHEMA_VERSION, verify_ownership_labels,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

const LABEL_M5D_RUN_ID: &str = "io.gpustack.m5d-run-id";
const READINESS_MARKER: &str = "m5d-ready";
const MAX_LOG_BYTES: usize = 4096;
const PROBE_SLEEP_SECS: u64 = 120;
const STOP_TIMEOUT_SECS: u64 = 5;

#[derive(Debug)]
struct ProbeArgs {
    docker_socket: PathBuf,
    image: String,
    run_id: String,
    protected_container: String,
    execute: bool,
}

#[derive(Debug, Error)]
enum ProbeError {
    #[error("invalid M5-D probe argument: {0}")]
    Invalid(String),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Ownership(#[from] OwnershipError),
    #[error("M5-D probe invariant failed: {0}")]
    Invariant(String),
    #[error("failed to serialize M5-D probe identity or report: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("probe failed: {primary}; ownership-safe cleanup also failed: {cleanup}")]
    Cleanup {
        primary: Box<ProbeError>,
        cleanup: Box<ProbeError>,
    },
}

#[derive(Debug, Serialize)]
struct DiagnosticIdentity<'a> {
    schema_version: u8,
    mode: &'static str,
    owner_id: &'a str,
    run_id: &'a str,
    component: &'static str,
    image: &'a str,
    command: &'a [String],
    network_mode: &'static str,
    privileged: bool,
    gpu: bool,
    mounts: usize,
}

#[derive(Debug, Serialize)]
struct ProbeReport {
    mode: &'static str,
    run_id: String,
    daemon: DockerDaemonInfo,
    image: ImageIdentity,
    container_name: String,
    container_id: String,
    created_state: ContainerState,
    running_state: ContainerState,
    exited_state: ContainerState,
    readiness_log_seen: bool,
    correct_ownership_accepted: bool,
    wrong_owner_rejected: bool,
    protected_container: String,
    protected_container_rejected: bool,
    zero_residual: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let result = parse_args(std::env::args().skip(1)).and_then(|arguments| {
        if arguments.execute {
            Ok(arguments)
        } else {
            Err(ProbeError::Invalid(
                "--execute is required; the probe never mutates Docker by default".to_string(),
            ))
        }
    });
    let result = match result {
        Ok(arguments) => execute_probe(arguments).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(encoded) => {
                println!("{encoded}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn execute_probe(arguments: ProbeArgs) -> Result<ProbeReport, ProbeError> {
    validate_run_id(&arguments.run_id)?;
    immutable_digest(&arguments.image)?;
    let runtime = DockerRuntime::connect(&arguments.docker_socket, Duration::from_secs(10))?;
    let daemon = runtime.daemon_info().await?;
    let image = runtime.inspect_image(&arguments.image).await?;
    validate_image_identity(&arguments.image, &image)?;
    let spec = build_probe_spec(&arguments.image, &arguments.run_id)?;
    let protected_before = find_container(&runtime, &arguments.protected_container).await?;

    if find_container_optional(&runtime, &spec.name)
        .await?
        .is_some()
    {
        return Err(ProbeError::Invariant(format!(
            "container name already exists: {}",
            spec.name
        )));
    }
    let filters = ownership_filters(&spec.labels);
    if !runtime.list_containers(&filters).await?.is_empty() {
        return Err(ProbeError::Invariant(
            "a container already uses this M5-D ownership tuple".to_string(),
        ));
    }

    let created = runtime.create_container(&spec).await?;
    let lifecycle = run_lifecycle(
        &runtime,
        &spec,
        &created.id,
        daemon,
        image,
        &arguments.protected_container,
        &protected_before,
    )
    .await;
    let cleanup = cleanup_owned_container(&runtime, &created.id, &spec.labels).await;
    match (lifecycle, cleanup) {
        (Ok(report), Ok(false)) => Ok(report),
        (Ok(_), Ok(true)) => Err(ProbeError::Invariant(
            "normal lifecycle returned before removing its container".to_string(),
        )),
        (Ok(_), Err(cleanup)) => Err(cleanup),
        (Err(primary), Ok(_)) => Err(primary),
        (Err(primary), Err(cleanup)) => Err(ProbeError::Cleanup {
            primary: Box::new(primary),
            cleanup: Box::new(cleanup),
        }),
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_lifecycle(
    runtime: &dyn ContainerRuntime,
    spec: &ContainerSpec,
    container_id: &str,
    daemon: DockerDaemonInfo,
    image: ImageIdentity,
    protected_name: &str,
    protected_before: &ContainerDetails,
) -> Result<ProbeReport, ProbeError> {
    let created = runtime.inspect_container(container_id).await?;
    verify_ownership_labels(&spec.labels, &created.labels)?;
    require_state("created container", created.state, ContainerState::Created)?;

    runtime.start_container(container_id).await?;
    let running = runtime.inspect_container(container_id).await?;
    verify_ownership_labels(&spec.labels, &running.labels)?;
    require_state("started container", running.state, ContainerState::Running)?;
    wait_for_readiness_log(runtime, container_id).await?;

    let mut wrong_owner = spec.labels.clone();
    wrong_owner.insert(LABEL_OWNER_ID.to_string(), "m5d-wrong-owner".to_string());
    if verify_ownership_labels(&wrong_owner, &running.labels).is_ok() {
        return Err(ProbeError::Invariant(
            "wrong owner unexpectedly passed complete label verification".to_string(),
        ));
    }
    let after_rejection = runtime.inspect_container(container_id).await?;
    require_state(
        "container after wrong-owner rejection",
        after_rejection.state,
        ContainerState::Running,
    )?;

    if verify_ownership_labels(&spec.labels, &protected_before.labels).is_ok() {
        return Err(ProbeError::Invariant(format!(
            "protected container unexpectedly matched M5-D ownership: {protected_name}"
        )));
    }
    let protected_after = runtime.inspect_container(&protected_before.id).await?;
    if protected_after.id != protected_before.id || protected_after.state != protected_before.state
    {
        return Err(ProbeError::Invariant(format!(
            "protected container changed during rejection check: {protected_name}"
        )));
    }

    runtime
        .stop_container(container_id, STOP_TIMEOUT_SECS)
        .await?;
    let exited = runtime.inspect_container(container_id).await?;
    verify_ownership_labels(&spec.labels, &exited.labels)?;
    require_state("stopped container", exited.state, ContainerState::Exited)?;
    runtime.remove_container(container_id, false).await?;
    require_missing(runtime, container_id).await?;
    if !runtime
        .list_containers(&ownership_filters(&spec.labels))
        .await?
        .is_empty()
    {
        return Err(ProbeError::Invariant(
            "M5-D ownership tuple still selects a container after removal".to_string(),
        ));
    }

    Ok(ProbeReport {
        mode: "m5d-no-gpu-diagnostic",
        run_id: spec
            .labels
            .get(LABEL_M5D_RUN_ID)
            .cloned()
            .unwrap_or_default(),
        daemon,
        image,
        container_name: spec.name.clone(),
        container_id: container_id.to_string(),
        created_state: created.state,
        running_state: running.state,
        exited_state: exited.state,
        readiness_log_seen: true,
        correct_ownership_accepted: true,
        wrong_owner_rejected: true,
        protected_container: protected_name.to_string(),
        protected_container_rejected: true,
        zero_residual: true,
    })
}

async fn cleanup_owned_container(
    runtime: &dyn ContainerRuntime,
    container_id: &str,
    expected_labels: &BTreeMap<String, String>,
) -> Result<bool, ProbeError> {
    let details = match runtime.inspect_container(container_id).await {
        Ok(details) => details,
        Err(RuntimeError::NotFound { .. }) => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    // Failure cleanup remains fail-closed: an ID captured from create is not ownership proof.
    verify_ownership_labels(expected_labels, &details.labels)?;
    match details.state {
        ContainerState::Running => {
            runtime
                .stop_container(container_id, STOP_TIMEOUT_SECS)
                .await?;
        }
        ContainerState::Created | ContainerState::Exited | ContainerState::Dead => {}
        state => {
            return Err(ProbeError::Invariant(format!(
                "refusing automatic cleanup while owned container is {state:?}"
            )));
        }
    }
    runtime.remove_container(container_id, false).await?;
    require_missing(runtime, container_id).await?;
    Ok(true)
}

async fn find_container(
    runtime: &dyn ContainerRuntime,
    name: &str,
) -> Result<ContainerDetails, ProbeError> {
    find_container_optional(runtime, name)
        .await?
        .ok_or_else(|| ProbeError::Invariant(format!("container was not found: {name}")))
}

async fn find_container_optional(
    runtime: &dyn ContainerRuntime,
    name: &str,
) -> Result<Option<ContainerDetails>, ProbeError> {
    let entries = runtime.list_containers(&BTreeMap::new()).await?;
    let matches = entries
        .into_iter()
        .filter(|entry| entry.names.iter().any(|candidate| candidate == name))
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(ProbeError::Invariant(format!(
            "multiple containers report the same name {name}: {}",
            matches.len()
        )));
    }
    match matches.first() {
        Some(entry) => Ok(Some(runtime.inspect_container(&entry.id).await?)),
        None => Ok(None),
    }
}

async fn wait_for_readiness_log(
    runtime: &dyn ContainerRuntime,
    container_id: &str,
) -> Result<(), ProbeError> {
    for _ in 0..20 {
        let mut logs = runtime.logs(
            container_id,
            ContainerLogOptions {
                follow: false,
                since_unix: 0,
                tail: "all".to_string(),
                timestamps: false,
            },
        );
        let mut collected = Vec::new();
        while let Some(line) = logs.next().await {
            let line = line?;
            let remaining = MAX_LOG_BYTES.saturating_sub(collected.len());
            collected.extend(line.message.into_iter().take(remaining));
            if collected.len() >= MAX_LOG_BYTES {
                break;
            }
        }
        if String::from_utf8_lossy(&collected).contains(READINESS_MARKER) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(ProbeError::Invariant(
        "diagnostic container did not emit the readiness marker".to_string(),
    ))
}

async fn require_missing(
    runtime: &dyn ContainerRuntime,
    container_id: &str,
) -> Result<(), ProbeError> {
    match runtime.inspect_container(container_id).await {
        Err(RuntimeError::NotFound { .. }) => Ok(()),
        Ok(_) => Err(ProbeError::Invariant(
            "container still exists after removal".to_string(),
        )),
        Err(error) => Err(error.into()),
    }
}

fn build_probe_spec(image: &str, run_id: &str) -> Result<ContainerSpec, ProbeError> {
    validate_run_id(run_id)?;
    immutable_digest(image)?;
    let owner_id = format!("m5d-{run_id}");
    let command = vec![
        "/bin/sh".to_string(),
        "-ceu".to_string(),
        format!("printf '{READINESS_MARKER}\\n'; sleep {PROBE_SLEEP_SECS}"),
    ];
    let identity = DiagnosticIdentity {
        schema_version: WORKLOAD_SCHEMA_VERSION,
        mode: "diagnostic",
        owner_id: &owner_id,
        run_id,
        component: "runtime-smoke",
        image,
        command: &command,
        network_mode: "none",
        privileged: false,
        gpu: false,
        mounts: 0,
    };
    let spec_hash = hex_lower(&Sha256::digest(serde_json::to_vec(&identity)?));
    let labels = BTreeMap::from([
        (LABEL_IMPLEMENTATION.to_string(), "rust".to_string()),
        (
            LABEL_SCHEMA_VERSION.to_string(),
            WORKLOAD_SCHEMA_VERSION.to_string(),
        ),
        (LABEL_MODE.to_string(), "diagnostic".to_string()),
        (LABEL_OWNER_ID.to_string(), owner_id),
        (LABEL_COMPONENT.to_string(), "runtime-smoke".to_string()),
        (LABEL_SPEC_HASH.to_string(), spec_hash),
        (LABEL_M5D_RUN_ID.to_string(), run_id.to_string()),
    ]);
    Ok(ContainerSpec {
        name: format!("gpustack-rs-m5d-{run_id}"),
        image: image.to_string(),
        command,
        user: None,
        environment: BTreeMap::new(),
        labels,
        mounts: Vec::new(),
        network_mode: "none".to_string(),
        shm_size_bytes: 64 * 1024 * 1024,
        privileged: false,
        security_options: vec!["no-new-privileges:true".to_string()],
        gpu_request: None,
        restart_policy: ContainerRestartPolicy::No,
        auto_remove: false,
        stop_timeout_secs: STOP_TIMEOUT_SECS,
    })
}

fn ownership_filters(labels: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    [LABEL_OWNER_ID, LABEL_M5D_RUN_ID]
        .into_iter()
        .filter_map(|label| {
            labels
                .get(label)
                .map(|value| (label.to_string(), value.clone()))
        })
        .collect()
}

fn require_state(
    operation: &str,
    actual: ContainerState,
    expected: ContainerState,
) -> Result<(), ProbeError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ProbeError::Invariant(format!(
            "{operation} state is {actual:?}, expected {expected:?}"
        )))
    }
}

fn validate_image_identity(requested: &str, image: &ImageIdentity) -> Result<(), ProbeError> {
    let requested_digest = immutable_digest(requested)?;
    let matches = image.id.strip_prefix("sha256:") == Some(requested_digest)
        || image.repo_digests.iter().any(|candidate| {
            candidate == requested
                || candidate
                    .rsplit_once("@sha256:")
                    .is_some_and(|(_, digest)| digest == requested_digest)
        });
    if !matches {
        return Err(ProbeError::Invariant(
            "cached image identity does not match the requested digest".to_string(),
        ));
    }
    if image.os.as_deref() != Some("linux")
        || !matches!(image.architecture.as_deref(), Some("amd64" | "x86_64"))
    {
        return Err(ProbeError::Invariant(
            "M5-D probe image must target Linux amd64".to_string(),
        ));
    }
    Ok(())
}

fn immutable_digest(image: &str) -> Result<&str, ProbeError> {
    let (repository, digest) = image.rsplit_once("@sha256:").ok_or_else(|| {
        ProbeError::Invalid("--image must use repository@sha256:<64 hex>".to_string())
    })?;
    if repository.is_empty()
        || digest.len() != 64
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ProbeError::Invalid(
            "--image must use repository@sha256:<64 hex>".to_string(),
        ));
    }
    Ok(digest)
}

fn validate_run_id(run_id: &str) -> Result<(), ProbeError> {
    if run_id.is_empty()
        || run_id.len() > 32
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ProbeError::Invalid(
            "--run-id must contain 1-32 lowercase ASCII letters, digits, or '-'".to_string(),
        ));
    }
    Ok(())
}

fn parse_args(arguments: impl IntoIterator<Item = String>) -> Result<ProbeArgs, ProbeError> {
    let mut docker_socket = None;
    let mut image = None;
    let mut run_id = None;
    let mut protected_container = "gpustack-worker".to_string();
    let mut execute = false;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--docker-socket" => docker_socket = arguments.next().map(PathBuf::from),
            "--image" => image = arguments.next(),
            "--run-id" => run_id = arguments.next(),
            "--protected-container" => {
                protected_container = arguments.next().ok_or_else(|| {
                    ProbeError::Invalid("--protected-container requires a value".to_string())
                })?;
            }
            "--execute" if !execute => execute = true,
            _ => {
                return Err(ProbeError::Invalid(format!(
                    "unknown or repeated argument: {argument}"
                )));
            }
        }
    }
    let docker_socket = docker_socket
        .ok_or_else(|| ProbeError::Invalid("--docker-socket is required".to_string()))?;
    let image = image.ok_or_else(|| ProbeError::Invalid("--image is required".to_string()))?;
    let run_id = run_id.ok_or_else(|| ProbeError::Invalid("--run-id is required".to_string()))?;
    if protected_container.is_empty() {
        return Err(ProbeError::Invalid(
            "--protected-container must not be empty".to_string(),
        ));
    }
    Ok(ProbeArgs {
        docker_socket,
        image,
        run_id,
        protected_container,
        execute,
    })
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[(byte >> 4) as usize]));
        output.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMAGE: &str = "quay.io/gpustack/runner@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn execution_gate_is_required_before_runtime_initialization() {
        let arguments = parse_args(
            [
                "--docker-socket",
                "/var/run/docker.sock",
                "--image",
                IMAGE,
                "--run-id",
                "offline-test",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .expect("parse offline arguments");

        assert!(!arguments.execute);
    }

    #[test]
    fn diagnostic_spec_has_no_gpu_mount_network_or_privilege() {
        let spec = build_probe_spec(IMAGE, "offline-test").expect("build diagnostic spec");

        assert_eq!(spec.network_mode, "none");
        assert!(spec.mounts.is_empty());
        assert!(spec.gpu_request.is_none());
        assert!(!spec.privileged);
        assert!(!spec.auto_remove);
        assert_eq!(spec.restart_policy, ContainerRestartPolicy::No);
        assert_eq!(
            spec.security_options,
            vec!["no-new-privileges:true".to_string()]
        );
        assert!(spec.command.join(" ").contains(READINESS_MARKER));
    }

    #[test]
    fn complete_diagnostic_tuple_rejects_the_wrong_owner() {
        let spec = build_probe_spec(IMAGE, "offline-test").expect("build diagnostic spec");
        verify_ownership_labels(&spec.labels, &spec.labels).expect("matching tuple");
        let mut wrong = spec.labels.clone();
        wrong.insert(LABEL_OWNER_ID.to_string(), "another-owner".to_string());

        assert!(matches!(
            verify_ownership_labels(&wrong, &spec.labels),
            Err(OwnershipError::Mismatch { label, .. }) if label == LABEL_OWNER_ID
        ));
    }
}
