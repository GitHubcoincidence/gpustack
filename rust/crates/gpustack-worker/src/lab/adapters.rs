use std::fs;
use std::net::TcpListener;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nvml_wrapper::Nvml;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::runtime_core::{VllmHealthFuture, VllmHealthProbe};

use super::{
    ClockFuture, LabClock, LabError, LabFuture, LabGpu, LabGpuProbe, LabHttpClient, LabIdSource,
    LabInferRequest, LabInferenceKind, ModelPathResolver, PortProbe,
};

#[derive(Debug, Default)]
pub struct FsModelPathResolver;

impl ModelPathResolver for FsModelPathResolver {
    fn resolve(&self, roots: &[PathBuf], requested: &Path) -> Result<PathBuf, LabError> {
        if roots.is_empty() {
            return Err(LabError::Invalid(
                "at least one --model-root is required for Lab deployment".to_string(),
            ));
        }
        if !requested.is_absolute()
            || requested
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            return Err(LabError::Invalid(
                "Lab model path must be absolute and normalized".to_string(),
            ));
        }
        let canonical = fs::canonicalize(requested).map_err(|error| {
            LabError::filesystem("canonicalize Lab model path", requested, error)
        })?;
        if !canonical.is_dir() {
            return Err(LabError::Invalid(
                "Lab model path must resolve to a directory".to_string(),
            ));
        }
        for root in roots {
            let canonical_root = fs::canonicalize(root).map_err(|error| {
                LabError::filesystem("canonicalize configured model root", root, error)
            })?;
            if requested.starts_with(root) && canonical.starts_with(&canonical_root) {
                return Ok(canonical);
            }
        }
        Err(LabError::Unsafe(format!(
            "model path is outside every configured model root: {}",
            requested.display()
        )))
    }
}

#[derive(Debug, Default)]
pub struct NvmlLabGpuProbe;

impl LabGpuProbe for NvmlLabGpuProbe {
    fn resolve(&self, selector: &str) -> Result<LabGpu, LabError> {
        let nvml = Nvml::init().map_err(|error| LabError::Gpu(error.to_string()))?;
        let count = nvml
            .device_count()
            .map_err(|error| LabError::Gpu(error.to_string()))?;
        let index_selector = selector.parse::<u32>().ok();
        let mut selected = None;
        for index in 0..count {
            let device = nvml
                .device_by_index(index)
                .map_err(|error| LabError::Gpu(error.to_string()))?;
            let uuid = device
                .uuid()
                .map_err(|error| LabError::Gpu(error.to_string()))?;
            if index_selector == Some(index) || selector == uuid {
                if selected.is_some() {
                    return Err(LabError::Gpu(
                        "GPU selector matched more than one device".to_string(),
                    ));
                }
                let processes = device
                    .running_compute_processes()
                    .map_err(|error| LabError::Gpu(error.to_string()))?;
                selected = Some(LabGpu {
                    host_index: index,
                    uuid,
                    running_compute_processes: processes.len(),
                });
            }
        }
        selected.ok_or_else(|| LabError::Gpu(format!("GPU selector was not found: {selector}")))
    }
}

#[derive(Debug, Default)]
pub struct TcpPortProbe;

impl PortProbe for TcpPortProbe {
    fn is_available(&self, host: &str, port: u16) -> Result<bool, LabError> {
        match TcpListener::bind((host, port)) {
            Ok(listener) => {
                drop(listener);
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => Ok(false),
            Err(error) => Err(LabError::Invalid(format!(
                "cannot test Lab port {host}:{port}: {error}"
            ))),
        }
    }
}

#[derive(Debug, Default)]
pub struct SystemLabClock;

impl LabClock for SystemLabClock {
    fn now_unix(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn sleep(&self, duration: Duration) -> ClockFuture<'_> {
        Box::pin(tokio::time::sleep(duration))
    }
}

#[derive(Debug, Default)]
pub struct KernelUuidSource;

impl LabIdSource for KernelUuidSource {
    fn next_id(&self) -> Result<String, LabError> {
        let path = Path::new("/proc/sys/kernel/random/uuid");
        let value = fs::read_to_string(path)
            .map_err(|error| LabError::filesystem("read kernel UUID", path, error))?;
        let value = value.trim();
        if value.len() != 36
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
        {
            return Err(LabError::Unsafe(
                "kernel UUID source returned an invalid identifier".to_string(),
            ));
        }
        Ok(value.to_string())
    }
}

#[derive(Clone)]
pub struct ReqwestLabHttpClient {
    client: reqwest::Client,
}

impl ReqwestLabHttpClient {
    pub fn new(timeout: Duration) -> Result<Self, LabError> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(timeout)
            .build()
            .map_err(|error| LabError::Http(error.to_string()))?;
        Ok(Self { client })
    }
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

impl VllmHealthProbe for ReqwestLabHttpClient {
    fn models<'a>(&'a self, endpoint: &'a str) -> VllmHealthFuture<'a> {
        Box::pin(async move {
            let response = self
                .client
                .get(format!("{endpoint}/v1/models"))
                .send()
                .await
                .map_err(|error| error.to_string())?;
            if response.status() != StatusCode::OK {
                return Err(format!(
                    "vLLM models endpoint returned HTTP {}",
                    response.status().as_u16()
                ));
            }
            let response = response
                .json::<ModelsResponse>()
                .await
                .map_err(|error| error.to_string())?;
            Ok(response.data.into_iter().map(|model| model.id).collect())
        })
    }
}

impl LabHttpClient for ReqwestLabHttpClient {
    fn infer<'a>(
        &'a self,
        endpoint: &'a str,
        model: &'a str,
        request: &'a LabInferRequest,
    ) -> LabFuture<'a, Value> {
        Box::pin(async move {
            let (path, body) = match request.kind {
                LabInferenceKind::Chat => (
                    "/v1/chat/completions",
                    json!({
                        "model": model,
                        "messages": [{"role": "user", "content": request.prompt}],
                        "max_tokens": request.max_tokens,
                    }),
                ),
                LabInferenceKind::Completion => (
                    "/v1/completions",
                    json!({
                        "model": model,
                        "prompt": request.prompt,
                        "max_tokens": request.max_tokens,
                    }),
                ),
            };
            let response = self
                .client
                .post(format!("{endpoint}{path}"))
                .json(&body)
                .send()
                .await
                .map_err(|error| LabError::Http(error.to_string()))?;
            let status = response.status();
            if status != StatusCode::OK {
                return Err(LabError::Http(format!(
                    "vLLM inference endpoint returned HTTP {}",
                    status.as_u16()
                )));
            }
            response
                .json::<Value>()
                .await
                .map_err(|error| LabError::Http(error.to_string()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_path_resolver_rejects_symlink_escape() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("models");
        let outside = directory.path().join("outside");
        fs::create_dir_all(&root).expect("model root");
        fs::create_dir_all(&outside).expect("outside directory");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("escape")).expect("symlink");
            let error = FsModelPathResolver
                .resolve(std::slice::from_ref(&root), &root.join("escape"))
                .expect_err("escape must fail");
            assert!(matches!(error, LabError::Unsafe(_)));
        }
    }

    #[test]
    fn model_path_resolver_accepts_a_canonical_directory_inside_a_root() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("models");
        let model = root.join("demo");
        fs::create_dir_all(&model).expect("model directory");

        let resolved = FsModelPathResolver
            .resolve(&[root], &model)
            .expect("allowed model path");

        assert_eq!(resolved, fs::canonicalize(model).expect("canonical path"));
    }
}
