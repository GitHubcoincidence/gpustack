use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

use bollard::container::LogOutput;
use bollard::errors::Error as BollardError;
use bollard::models::{
    ContainerCreateBody, ContainerInspectResponse, ContainerSummary, DeviceRequest, HostConfig,
    ImageInspect, Mount, MountType, RestartPolicy, RestartPolicyNameEnum, SystemVersion,
};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, ListContainersOptionsBuilder, LogsOptionsBuilder,
    RemoveContainerOptionsBuilder, StopContainerOptionsBuilder,
};
use bollard::{API_DEFAULT_VERSION, Docker};
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type RuntimeFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, RuntimeError>> + Send + 'a>>;
pub type RuntimeLogStream<'a> =
    Pin<Box<dyn Stream<Item = Result<RuntimeLogLine, RuntimeError>> + Send + 'a>>;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("cannot initialize Docker client: {0}")]
    Client(String),
    #[error("Docker {operation} failed: {source}")]
    Request {
        operation: &'static str,
        #[source]
        source: BollardError,
    },
    #[error("Docker {operation} target was not found")]
    NotFound { operation: &'static str },
    #[error("Docker {operation} response is missing {field}")]
    MissingField {
        operation: &'static str,
        field: &'static str,
    },
    #[error("invalid container specification: {0}")]
    InvalidSpec(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerDaemonInfo {
    pub ping: String,
    pub version: String,
    pub api_version: String,
    pub min_api_version: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageIdentity {
    pub id: String,
    pub repo_digests: Vec<String>,
    pub architecture: Option<String>,
    pub os: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerState {
    Created,
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Stopping,
    Unknown,
}

impl ContainerState {
    fn from_docker(value: &str) -> Self {
        match value {
            "created" => Self::Created,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "restarting" => Self::Restarting,
            "removing" => Self::Removing,
            "exited" => Self::Exited,
            "dead" => Self::Dead,
            "stopping" => Self::Stopping,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerListEntry {
    pub id: String,
    pub names: Vec<String>,
    pub image: Option<String>,
    pub image_id: Option<String>,
    pub state: ContainerState,
    pub status: Option<String>,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerDetails {
    pub id: String,
    pub name: Option<String>,
    pub image_id: Option<String>,
    pub state: ContainerState,
    pub exit_code: Option<i64>,
    pub error: Option<String>,
    pub oom_killed: bool,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerCreateResult {
    pub id: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindMountSpec {
    pub source: PathBuf,
    pub target: PathBuf,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuDeviceRequest {
    pub driver: String,
    pub device_ids: Vec<String>,
    pub capabilities: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerRestartPolicy {
    No,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerSpec {
    pub name: String,
    pub image: String,
    pub command: Vec<String>,
    pub user: Option<String>,
    pub environment: BTreeMap<String, String>,
    pub labels: BTreeMap<String, String>,
    pub mounts: Vec<BindMountSpec>,
    pub network_mode: String,
    pub shm_size_bytes: u64,
    pub privileged: bool,
    pub security_options: Vec<String>,
    pub gpu_request: Option<GpuDeviceRequest>,
    pub restart_policy: ContainerRestartPolicy,
    pub auto_remove: bool,
    pub stop_timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerLogOptions {
    pub follow: bool,
    pub since_unix: i32,
    pub tail: String,
    pub timestamps: bool,
}

impl Default for ContainerLogOptions {
    fn default() -> Self {
        Self {
            follow: false,
            since_unix: 0,
            tail: "all".to_string(),
            timestamps: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLogStreamKind {
    Stdout,
    Stderr,
    Stdin,
    Console,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLogLine {
    pub stream: RuntimeLogStreamKind,
    pub message: Vec<u8>,
}

/// This boundary keeps Docker I/O replaceable so offline reconciliation tests never need a daemon.
pub trait ContainerRuntime: Send + Sync {
    fn daemon_info(&self) -> RuntimeFuture<'_, DockerDaemonInfo>;
    fn inspect_image<'a>(&'a self, image: &'a str) -> RuntimeFuture<'a, ImageIdentity>;
    fn list_containers<'a>(
        &'a self,
        labels: &'a BTreeMap<String, String>,
    ) -> RuntimeFuture<'a, Vec<ContainerListEntry>>;
    fn create_container<'a>(
        &'a self,
        spec: &'a ContainerSpec,
    ) -> RuntimeFuture<'a, ContainerCreateResult>;
    fn start_container<'a>(&'a self, id: &'a str) -> RuntimeFuture<'a, ()>;
    fn inspect_container<'a>(&'a self, id: &'a str) -> RuntimeFuture<'a, ContainerDetails>;
    fn stop_container<'a>(&'a self, id: &'a str, timeout_secs: u64) -> RuntimeFuture<'a, ()>;
    fn remove_container<'a>(&'a self, id: &'a str, force: bool) -> RuntimeFuture<'a, ()>;
    fn logs<'a>(&'a self, id: &'a str, options: ContainerLogOptions) -> RuntimeLogStream<'a>;
}

#[derive(Clone)]
pub struct DockerRuntime {
    client: Docker,
}

impl DockerRuntime {
    pub fn connect(socket: &Path, timeout: Duration) -> Result<Self, RuntimeError> {
        let socket = socket
            .to_str()
            .ok_or_else(|| RuntimeError::Client("Docker socket path is not UTF-8".to_string()))?;
        let timeout = timeout.as_secs().max(1);
        let client = Docker::connect_with_socket(socket, timeout, API_DEFAULT_VERSION)
            .map_err(|error| RuntimeError::Client(error.to_string()))?;
        Ok(Self { client })
    }
}

impl ContainerRuntime for DockerRuntime {
    fn daemon_info(&self) -> RuntimeFuture<'_, DockerDaemonInfo> {
        Box::pin(async move {
            let ping = self
                .client
                .ping()
                .await
                .map_err(|source| request_error("ping", source))?;
            let version = self
                .client
                .version()
                .await
                .map_err(|source| request_error("version", source))?;
            map_daemon_info(ping, version)
        })
    }

    fn inspect_image<'a>(&'a self, image: &'a str) -> RuntimeFuture<'a, ImageIdentity> {
        Box::pin(async move {
            let inspected = self
                .client
                .inspect_image(image)
                .await
                .map_err(|source| request_error("image inspect", source))?;
            map_image_identity(inspected)
        })
    }

    fn list_containers<'a>(
        &'a self,
        labels: &'a BTreeMap<String, String>,
    ) -> RuntimeFuture<'a, Vec<ContainerListEntry>> {
        Box::pin(async move {
            let mut filters = HashMap::new();
            filters.insert(
                "label".to_string(),
                labels
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>(),
            );
            let options = ListContainersOptionsBuilder::new()
                .all(true)
                .filters(&filters)
                .build();
            self.client
                .list_containers(Some(options))
                .await
                .map_err(|source| request_error("container list", source))?
                .into_iter()
                .map(map_container_summary)
                .collect()
        })
    }

    fn create_container<'a>(
        &'a self,
        spec: &'a ContainerSpec,
    ) -> RuntimeFuture<'a, ContainerCreateResult> {
        Box::pin(async move {
            let body = build_create_body(spec)?;
            let options = CreateContainerOptionsBuilder::new()
                .name(&spec.name)
                .build();
            let response = self
                .client
                .create_container(Some(options), body)
                .await
                .map_err(|source| request_error("container create", source))?;
            Ok(ContainerCreateResult {
                id: response.id,
                warnings: response.warnings,
            })
        })
    }

    fn start_container<'a>(&'a self, id: &'a str) -> RuntimeFuture<'a, ()> {
        Box::pin(async move {
            self.client
                .start_container(id, None)
                .await
                .map_err(|source| request_error("container start", source))
        })
    }

    fn inspect_container<'a>(&'a self, id: &'a str) -> RuntimeFuture<'a, ContainerDetails> {
        Box::pin(async move {
            let response = self
                .client
                .inspect_container(id, None)
                .await
                .map_err(|source| request_error("container inspect", source))?;
            map_container_inspect(response)
        })
    }

    fn stop_container<'a>(&'a self, id: &'a str, timeout_secs: u64) -> RuntimeFuture<'a, ()> {
        Box::pin(async move {
            let timeout = i32::try_from(timeout_secs).map_err(|_| {
                RuntimeError::InvalidSpec("stop timeout exceeds Docker API range".to_string())
            })?;
            let options = StopContainerOptionsBuilder::new().t(timeout).build();
            self.client
                .stop_container(id, Some(options))
                .await
                .map_err(|source| request_error("container stop", source))
        })
    }

    fn remove_container<'a>(&'a self, id: &'a str, force: bool) -> RuntimeFuture<'a, ()> {
        Box::pin(async move {
            let options = RemoveContainerOptionsBuilder::new().force(force).build();
            self.client
                .remove_container(id, Some(options))
                .await
                .map_err(|source| request_error("container remove", source))
        })
    }

    fn logs<'a>(&'a self, id: &'a str, options: ContainerLogOptions) -> RuntimeLogStream<'a> {
        let options = LogsOptionsBuilder::new()
            .follow(options.follow)
            .stdout(true)
            .stderr(true)
            .since(options.since_unix)
            .timestamps(options.timestamps)
            .tail(&options.tail)
            .build();
        Box::pin(self.client.logs(id, Some(options)).map(|result| {
            result
                .map(map_log_output)
                .map_err(|source| request_error("container logs", source))
        }))
    }
}

fn request_error(operation: &'static str, source: BollardError) -> RuntimeError {
    if matches!(
        source,
        BollardError::DockerResponseServerError {
            status_code: 404,
            ..
        }
    ) {
        RuntimeError::NotFound { operation }
    } else {
        RuntimeError::Request { operation, source }
    }
}

fn map_daemon_info(ping: String, version: SystemVersion) -> Result<DockerDaemonInfo, RuntimeError> {
    Ok(DockerDaemonInfo {
        ping,
        version: required(version.version, "version", "Version")?,
        api_version: required(version.api_version, "version", "ApiVersion")?,
        min_api_version: version.min_api_version,
        os: version.os,
        arch: version.arch,
    })
}

fn map_image_identity(image: ImageInspect) -> Result<ImageIdentity, RuntimeError> {
    Ok(ImageIdentity {
        id: required(image.id, "image inspect", "Id")?,
        repo_digests: image.repo_digests.unwrap_or_default(),
        architecture: image.architecture,
        os: image.os,
    })
}

fn map_container_summary(summary: ContainerSummary) -> Result<ContainerListEntry, RuntimeError> {
    Ok(ContainerListEntry {
        id: required(summary.id, "container list", "Id")?,
        names: summary
            .names
            .unwrap_or_default()
            .into_iter()
            .map(|name| name.trim_start_matches('/').to_string())
            .collect(),
        image: summary.image,
        image_id: summary.image_id,
        state: summary
            .state
            .map(|state| ContainerState::from_docker(state.as_ref()))
            .unwrap_or(ContainerState::Unknown),
        status: summary.status,
        labels: summary.labels.unwrap_or_default().into_iter().collect(),
    })
}

fn map_container_inspect(
    response: ContainerInspectResponse,
) -> Result<ContainerDetails, RuntimeError> {
    let state = response.state.unwrap_or_default();
    let labels = response
        .config
        .and_then(|config| config.labels)
        .unwrap_or_default()
        .into_iter()
        .collect();
    Ok(ContainerDetails {
        id: required(response.id, "container inspect", "Id")?,
        name: response
            .name
            .map(|name| name.trim_start_matches('/').to_string()),
        image_id: response.image,
        state: state
            .status
            .map(|status| ContainerState::from_docker(status.as_ref()))
            .unwrap_or(ContainerState::Unknown),
        exit_code: state.exit_code,
        error: state.error.filter(|error| !error.is_empty()),
        oom_killed: state.oom_killed.unwrap_or(false),
        labels,
    })
}

fn required<T>(
    value: Option<T>,
    operation: &'static str,
    field: &'static str,
) -> Result<T, RuntimeError> {
    value.ok_or(RuntimeError::MissingField { operation, field })
}

fn build_create_body(spec: &ContainerSpec) -> Result<ContainerCreateBody, RuntimeError> {
    validate_container_spec(spec)?;
    let mounts = spec
        .mounts
        .iter()
        .map(|mount| Mount {
            target: Some(mount.target.display().to_string()),
            source: Some(mount.source.display().to_string()),
            typ: Some(MountType::BIND),
            read_only: Some(mount.read_only),
            ..Default::default()
        })
        .collect();
    let device_requests = spec.gpu_request.as_ref().map(|request| {
        vec![DeviceRequest {
            driver: Some(request.driver.clone()),
            device_ids: Some(request.device_ids.clone()),
            capabilities: Some(request.capabilities.clone()),
            ..Default::default()
        }]
    });
    let restart_policy = match spec.restart_policy {
        ContainerRestartPolicy::No => RestartPolicyNameEnum::NO,
    };
    let shm_size = i64::try_from(spec.shm_size_bytes).map_err(|_| {
        RuntimeError::InvalidSpec("shared memory size exceeds Docker API range".to_string())
    })?;
    let stop_timeout = i64::try_from(spec.stop_timeout_secs).map_err(|_| {
        RuntimeError::InvalidSpec("stop timeout exceeds Docker API range".to_string())
    })?;
    Ok(ContainerCreateBody {
        image: Some(spec.image.clone()),
        cmd: Some(spec.command.clone()),
        user: spec.user.clone(),
        env: Some(
            spec.environment
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect(),
        ),
        labels: Some(spec.labels.clone().into_iter().collect()),
        stop_timeout: Some(stop_timeout),
        host_config: Some(HostConfig {
            network_mode: Some(spec.network_mode.clone()),
            restart_policy: Some(RestartPolicy {
                name: Some(restart_policy),
                maximum_retry_count: Some(0),
            }),
            auto_remove: Some(spec.auto_remove),
            mounts: Some(mounts),
            device_requests,
            privileged: Some(spec.privileged),
            security_opt: Some(spec.security_options.clone()),
            shm_size: Some(shm_size),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn validate_container_spec(spec: &ContainerSpec) -> Result<(), RuntimeError> {
    if spec.name.is_empty() || spec.image.is_empty() || spec.command.is_empty() {
        return Err(RuntimeError::InvalidSpec(
            "name, image, and command must be non-empty".to_string(),
        ));
    }
    if spec.network_mode.is_empty() {
        return Err(RuntimeError::InvalidSpec(
            "network mode must be non-empty".to_string(),
        ));
    }
    if spec
        .user
        .as_deref()
        .is_some_and(|user| user.trim().is_empty() || user.contains('\0'))
    {
        return Err(RuntimeError::InvalidSpec(
            "container user must be non-empty and contain no NUL".to_string(),
        ));
    }
    if spec.shm_size_bytes == 0 {
        return Err(RuntimeError::InvalidSpec(
            "shared memory size must be greater than zero".to_string(),
        ));
    }
    for mount in &spec.mounts {
        if !mount.source.is_absolute() || !mount.target.is_absolute() {
            return Err(RuntimeError::InvalidSpec(
                "bind mount paths must be absolute".to_string(),
            ));
        }
    }
    if let Some(request) = &spec.gpu_request
        && (request.driver.is_empty()
            || request.device_ids.is_empty()
            || request.capabilities.is_empty())
    {
        return Err(RuntimeError::InvalidSpec(
            "GPU request must include driver, device IDs, and capabilities".to_string(),
        ));
    }
    Ok(())
}

fn map_log_output(output: LogOutput) -> RuntimeLogLine {
    match output {
        LogOutput::StdOut { message } => RuntimeLogLine {
            stream: RuntimeLogStreamKind::Stdout,
            message: message.to_vec(),
        },
        LogOutput::StdErr { message } => RuntimeLogLine {
            stream: RuntimeLogStreamKind::Stderr,
            message: message.to_vec(),
        },
        LogOutput::StdIn { message } => RuntimeLogLine {
            stream: RuntimeLogStreamKind::Stdin,
            message: message.to_vec(),
        },
        LogOutput::Console { message } => RuntimeLogLine {
            stream: RuntimeLogStreamKind::Console,
            message: message.to_vec(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec() -> ContainerSpec {
        ContainerSpec {
            name: "gpustack-rs-lab-demo".to_string(),
            image: "runner@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            command: vec!["vllm".to_string(), "serve".to_string()],
            user: Some("1004:1004".to_string()),
            environment: BTreeMap::from([
                ("B_KEY".to_string(), "b".to_string()),
                ("A_KEY".to_string(), "a".to_string()),
            ]),
            labels: BTreeMap::from([(
                "io.gpustack.implementation".to_string(),
                "rust".to_string(),
            )]),
            mounts: vec![BindMountSpec {
                source: PathBuf::from("/models/Qwen-7B"),
                target: PathBuf::from("/models/Qwen-7B"),
                read_only: true,
            }],
            network_mode: "host".to_string(),
            shm_size_bytes: 10 * 1024 * 1024 * 1024,
            privileged: false,
            security_options: vec!["no-new-privileges:true".to_string()],
            gpu_request: Some(GpuDeviceRequest {
                driver: "nvidia".to_string(),
                device_ids: vec!["GPU-example".to_string()],
                capabilities: vec![vec!["gpu".to_string()]],
            }),
            restart_policy: ContainerRestartPolicy::No,
            auto_remove: false,
            stop_timeout_secs: 30,
        }
    }

    #[test]
    fn create_body_preserves_the_frozen_security_contract() {
        let body = build_create_body(&sample_spec()).expect("valid container plan");

        assert_eq!(
            body.env,
            Some(vec!["A_KEY=a".to_string(), "B_KEY=b".to_string()])
        );
        assert_eq!(body.user.as_deref(), Some("1004:1004"));
        let host = body.host_config.expect("host config");
        assert_eq!(host.network_mode.as_deref(), Some("host"));
        assert_eq!(host.shm_size, Some(10 * 1024 * 1024 * 1024));
        assert_eq!(host.privileged, Some(false));
        assert_eq!(
            host.security_opt,
            Some(vec!["no-new-privileges:true".to_string()])
        );
        assert_eq!(host.auto_remove, Some(false));
        assert_eq!(
            host.restart_policy.and_then(|policy| policy.name),
            Some(RestartPolicyNameEnum::NO)
        );
        let gpu = host
            .device_requests
            .expect("device requests")
            .pop()
            .expect("GPU request");
        assert_eq!(gpu.driver.as_deref(), Some("nvidia"));
        assert_eq!(gpu.device_ids, Some(vec!["GPU-example".to_string()]));
        let mount = host.mounts.expect("mounts").pop().expect("model mount");
        assert_eq!(mount.typ, Some(MountType::BIND));
        assert_eq!(mount.read_only, Some(true));
    }

    #[test]
    fn relative_mounts_fail_before_any_runtime_request() {
        let mut spec = sample_spec();
        spec.mounts[0].source = PathBuf::from("models/Qwen-7B");

        let error = build_create_body(&spec).expect_err("relative source must fail");

        assert!(error.to_string().contains("absolute"));
    }

    #[test]
    fn docker_log_frames_keep_stream_identity_and_raw_bytes() {
        let line = map_log_output(LogOutput::StdErr {
            message: "failure\n".into(),
        });

        assert_eq!(line.stream, RuntimeLogStreamKind::Stderr);
        assert_eq!(line.message, b"failure\n");
    }

    #[test]
    fn missing_required_image_identity_fails_closed() {
        let error = map_image_identity(ImageInspect::default()).expect_err("missing ID");

        assert!(error.to_string().contains("Id"));
    }

    #[test]
    fn docker_404_is_a_typed_missing_resource_result() {
        let error = request_error(
            "container inspect",
            BollardError::DockerResponseServerError {
                status_code: 404,
                message: "missing".to_string(),
            },
        );

        assert!(matches!(
            error,
            RuntimeError::NotFound {
                operation: "container inspect"
            }
        ));
    }
}
