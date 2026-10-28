//! Runtime primitives shared by isolated Lab and Server-managed workloads.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use futures_util::StreamExt;
use gpustack_runtime::docker::{
    ContainerDetails, ContainerRuntime, ContainerState, ImageIdentity, RuntimeError,
};
use thiserror::Error;

use crate::workload::WorkloadSpecV2;

pub type VllmHealthFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<String>, String>> + Send + 'a>>;

pub trait VllmHealthProbe: Send + Sync {
    fn models<'a>(&'a self, endpoint: &'a str) -> VllmHealthFuture<'a>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VllmHealthState {
    Starting,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VllmHealthObservation {
    pub container: ContainerDetails,
    pub state: VllmHealthState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedContainer {
    Missing,
    Present(ContainerDetails),
}

#[derive(Debug, Error)]
pub enum RuntimeCoreError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error("runtime ownership check failed: {0}")]
    Unsafe(String),
}

pub async fn inspect_optional(
    runtime: &dyn ContainerRuntime,
    container_id: &str,
) -> Result<Option<ContainerDetails>, RuntimeCoreError> {
    match runtime.inspect_container(container_id).await {
        Ok(details) => Ok(Some(details)),
        Err(RuntimeError::NotFound { .. }) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub async fn resolve_immutable_image(
    runtime: &dyn ContainerRuntime,
    requested: &str,
) -> Result<String, RuntimeCoreError> {
    let requested_digest = image_digest(requested).ok_or_else(|| {
        RuntimeCoreError::Unsafe("runner image must use an immutable SHA-256 digest".to_string())
    })?;
    let identity = runtime.inspect_image(requested).await?;
    if !image_matches(&identity, requested, requested_digest) {
        return Err(RuntimeCoreError::Unsafe(
            "local image identity does not match the requested immutable digest".to_string(),
        ));
    }
    if identity.os.as_deref() != Some("linux")
        || !matches!(identity.architecture.as_deref(), Some("amd64" | "x86_64"))
    {
        return Err(RuntimeCoreError::Unsafe(
            "runner image must target Linux amd64".to_string(),
        ));
    }
    Ok(requested.to_string())
}

/// Discovers by a deliberately broad identity filter, then requires the complete tuple.
pub async fn discover_owned_container(
    runtime: &dyn ContainerRuntime,
    discovery_labels: &BTreeMap<String, String>,
    expected: &WorkloadSpecV2,
) -> Result<OwnedContainer, RuntimeCoreError> {
    let candidates = runtime.list_containers(discovery_labels).await?;
    if candidates.len() > 1 {
        return Err(RuntimeCoreError::Unsafe(format!(
            "{} containers claim the same workload identity",
            candidates.len()
        )));
    }
    let Some(candidate) = candidates.first() else {
        return Ok(OwnedContainer::Missing);
    };
    let details = runtime.inspect_container(&candidate.id).await?;
    expected
        .verify_ownership(&details.labels)
        .map_err(|error| RuntimeCoreError::Unsafe(error.to_string()))?;
    if details.name.as_deref() != Some(expected.container_name.as_str()) {
        return Err(RuntimeCoreError::Unsafe(
            "owned container name does not match the normalized workload".to_string(),
        ));
    }
    Ok(OwnedContainer::Present(details))
}

pub async fn observe_vllm_health(
    runtime: &dyn ContainerRuntime,
    probe: &dyn VllmHealthProbe,
    spec: &WorkloadSpecV2,
    container_id: &str,
) -> Result<VllmHealthObservation, RuntimeCoreError> {
    let details = runtime.inspect_container(container_id).await?;
    spec.verify_ownership(&details.labels)
        .map_err(|error| RuntimeCoreError::Unsafe(error.to_string()))?;
    if matches!(details.state, ContainerState::Exited | ContainerState::Dead) {
        let message = container_failure_diagnostic(runtime, &details).await;
        return Ok(VllmHealthObservation {
            container: details,
            state: VllmHealthState::Failed,
            message: Some(message),
        });
    }
    if details.state != ContainerState::Running {
        return Ok(VllmHealthObservation {
            container: details,
            state: VllmHealthState::Starting,
            message: None,
        });
    }

    let endpoint = workload_endpoint(spec);
    match probe.models(&endpoint).await {
        Ok(models) if models.iter().any(|model| model == &spec.served_model_name) => {
            Ok(VllmHealthObservation {
                container: details,
                state: VllmHealthState::Ready,
                message: None,
            })
        }
        Ok(_) => Ok(VllmHealthObservation {
            container: details,
            state: VllmHealthState::Starting,
            message: Some(format!(
                "expected model '{}' is absent from /v1/models",
                spec.served_model_name
            )),
        }),
        Err(error) => Ok(VllmHealthObservation {
            container: details,
            state: VllmHealthState::Starting,
            message: Some(redact_diagnostic(&error)),
        }),
    }
}

#[must_use]
pub fn workload_endpoint(spec: &WorkloadSpecV2) -> String {
    format!("http://{}:{}", spec.service_host, spec.service_port)
}

#[must_use]
pub fn redact_diagnostic(message: &str) -> String {
    let lowered = message.to_ascii_lowercase();
    if ["token", "password", "api-key", "api_key", "authorization"]
        .iter()
        .any(|needle| lowered.contains(needle))
    {
        return "diagnostic omitted because it may contain a secret".to_string();
    }
    message
        .chars()
        .filter(|character| !character.is_control() || *character == ' ')
        .take(512)
        .collect()
}

async fn container_failure_diagnostic(
    runtime: &dyn ContainerRuntime,
    details: &ContainerDetails,
) -> String {
    let reason = if details.oom_killed {
        "container was OOM-killed".to_string()
    } else if let Some(error) = &details.error {
        redact_diagnostic(error)
    } else {
        "container exited before vLLM became ready".to_string()
    };
    let mut diagnostic = match details.exit_code {
        Some(code) => format!("{reason} (exit code {code})"),
        None => reason,
    };
    let mut logs = runtime.logs(
        &details.id,
        gpustack_runtime::docker::ContainerLogOptions {
            follow: false,
            since_unix: 0,
            tail: "20".to_string(),
            timestamps: false,
        },
    );
    let mut tail = String::new();
    while tail.len() < 4096 {
        match logs.next().await {
            Some(Ok(line)) => {
                let remaining = 4096usize.saturating_sub(tail.len());
                tail.extend(
                    String::from_utf8_lossy(&line.message)
                        .chars()
                        .take(remaining),
                );
            }
            Some(Err(_)) | None => break,
        }
    }
    let tail = redact_diagnostic(tail.trim());
    if !tail.is_empty() {
        diagnostic.push_str("; log tail: ");
        diagnostic.push_str(&tail);
    }
    redact_diagnostic(&diagnostic)
}

fn image_digest(value: &str) -> Option<&str> {
    value
        .rsplit_once("@sha256:")
        .map(|(_, digest)| digest)
        .or_else(|| value.strip_prefix("sha256:"))
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn image_matches(identity: &ImageIdentity, requested: &str, requested_digest: &str) -> bool {
    identity.id.strip_prefix("sha256:") == Some(requested_digest)
        || identity.repo_digests.iter().any(|digest_value| {
            digest_value == requested || image_digest(digest_value) == Some(requested_digest)
        })
}
