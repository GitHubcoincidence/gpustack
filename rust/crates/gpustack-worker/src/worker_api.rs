//! Authenticated, read-only Worker HTTP API for resource-api-only mode.

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use axum::Json;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::stream;
use gpustack_client::SecretToken;
use gpustack_client::types::{SERVER_BASELINE_GIT_COMMIT, SERVER_BASELINE_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::config::WorkerConfig;
use crate::resources::unix_seconds_to_rfc3339;

const MAX_MODEL_CONFIG_BYTES: u64 = 16 * 1024 * 1024;
const MODEL_CONFIG_NAMES: [&str; 8] = [
    "config.json",
    "model_index.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "special_tokens_map.json",
    "generation_config.json",
    "adapter_config.json",
    "preprocessor_config.json",
];
const MODEL_WEIGHT_EXTENSIONS: [&str; 4] = ["safetensors", "bin", "pt", "pth"];

#[derive(Debug, Error)]
pub enum WorkerApiBuildError {
    #[error("model root must be an absolute path")]
    RelativeModelRoot,
    #[error("failed to canonicalize model root: {0}")]
    ModelRoot(io::Error),
}

#[derive(Debug, Clone)]
struct AllowedRoot {
    configured: PathBuf,
    canonical: PathBuf,
}

#[derive(Debug, Clone)]
pub struct WorkerApiState {
    token: SecretToken,
    roots: Arc<Vec<AllowedRoot>>,
    log_dir: PathBuf,
}

impl WorkerApiState {
    pub fn new(config: &WorkerConfig, token: SecretToken) -> Result<Self, WorkerApiBuildError> {
        let mut roots = Vec::with_capacity(config.model_roots.len());
        for root in &config.model_roots {
            if !root.is_absolute() {
                return Err(WorkerApiBuildError::RelativeModelRoot);
            }
            let canonical = fs::canonicalize(root).map_err(WorkerApiBuildError::ModelRoot)?;
            roots.push(AllowedRoot {
                configured: root.clone(),
                canonical,
            });
        }
        Ok(Self {
            token,
            roots: Arc::new(roots),
            log_dir: config.log_dir.clone(),
        })
    }
}

pub fn router(state: WorkerApiState) -> Router {
    let state = Arc::new(state);
    let protected = Router::new()
        .route("/token-auth", get(token_auth))
        .route("/files/file-exists", get(file_exists))
        .route("/files/model-config", get(model_config))
        .route("/files/model-weight-size", get(model_weight_size))
        .route("/serveLogOptions/{id}", get(serve_log_options))
        .route("/serveLogs/{id}", get(serve_logs))
        .route("/v2/config", get(get_config).put(put_config))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_worker_token,
        ));
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/version", get(version))
        .merge(protected)
        .fallback(not_found)
        .with_state(state)
}

async fn healthz() -> Json<&'static str> {
    Json("ok")
}

async fn readyz() -> ApiError {
    ApiError::service_unavailable()
}

#[derive(Serialize)]
struct VersionResponse<'a> {
    name: &'a str,
    version: &'a str,
    git_commit: &'a str,
    target: String,
    rust_toolchain: &'a str,
    server_baseline: ServerBaseline<'a>,
}

#[derive(Serialize)]
struct ServerBaseline<'a> {
    version: &'a str,
    git_commit: &'a str,
}

async fn version() -> Json<VersionResponse<'static>> {
    Json(VersionResponse {
        name: "gpustack-rs",
        version: env!("CARGO_PKG_VERSION"),
        git_commit: option_env!("GPUSTACK_GIT_COMMIT").unwrap_or("unknown"),
        target: option_env!("GPUSTACK_BUILD_TARGET").map_or_else(
            || format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            str::to_owned,
        ),
        rust_toolchain: "1.99.1",
        server_baseline: ServerBaseline {
            version: SERVER_BASELINE_VERSION,
            git_commit: SERVER_BASELINE_GIT_COMMIT,
        },
    })
}

async fn require_worker_token(
    State(state): State<Arc<WorkerApiState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if authenticate(request.headers(), &state.token) {
        next.run(request).await
    } else {
        ApiError::unauthorized().into_response()
    }
}

fn authenticate(headers: &HeaderMap, token: &SecretToken) -> bool {
    // Bearer is authoritative when both accepted headers are present, so an
    // invalid Bearer credential cannot be bypassed with a valid X-API-Key.
    if let Some(value) = headers.get(AUTHORIZATION) {
        return value
            .to_str()
            .ok()
            .and_then(|value| value.strip_prefix("Bearer "))
            .is_some_and(|candidate| token.constant_time_matches(candidate));
    }
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|candidate| token.constant_time_matches(candidate))
}

async fn token_auth() -> Json<&'static str> {
    Json("ok")
}

#[derive(Deserialize)]
struct FileQuery {
    path: String,
}

#[derive(Serialize)]
struct FileExistsResponse {
    exists: bool,
    path: String,
    is_file: bool,
    is_dir: bool,
}

async fn file_exists(
    State(state): State<Arc<WorkerApiState>>,
    query: Result<Query<FileQuery>, QueryRejection>,
) -> Result<Json<FileExistsResponse>, ApiError> {
    let query = valid_query(query)?;
    let resolved = resolve_model_path(&state.roots, Path::new(&query.path), true)?;
    let metadata = match fs::metadata(&resolved) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(map_io_error(error)),
    };
    Ok(Json(FileExistsResponse {
        exists: metadata.is_some(),
        path: query.path.clone(),
        is_file: metadata.as_ref().is_some_and(fs::Metadata::is_file),
        is_dir: metadata.as_ref().is_some_and(fs::Metadata::is_dir),
    }))
}

async fn model_config(
    State(state): State<Arc<WorkerApiState>>,
    query: Result<Query<FileQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let query = valid_query(query)?;
    let path = resolve_model_path(&state.roots, Path::new(&query.path), false)?;
    let name = path.file_name().and_then(|name| name.to_str());
    if !name.is_some_and(|name| MODEL_CONFIG_NAMES.contains(&name)) {
        return Err(ApiError::bad_request());
    }
    read_model_config(&path).map(Json)
}

fn read_model_config(path: &Path) -> Result<Value, ApiError> {
    let metadata = fs::metadata(path).map_err(map_io_error)?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request());
    }
    if metadata.len() > MAX_MODEL_CONFIG_BYTES {
        return Err(ApiError::payload_too_large());
    }
    let content = fs::read(path).map_err(map_io_error)?;
    if content.len() as u64 > MAX_MODEL_CONFIG_BYTES {
        return Err(ApiError::payload_too_large());
    }
    serde_json::from_slice(&content).map_err(|_| ApiError::bad_request())
}

#[derive(Serialize)]
struct ModelWeightSizeResponse {
    size: u64,
}

async fn model_weight_size(
    State(state): State<Arc<WorkerApiState>>,
    query: Result<Query<FileQuery>, QueryRejection>,
) -> Result<Json<ModelWeightSizeResponse>, ApiError> {
    let query = valid_query(query)?;
    let path = resolve_model_path(&state.roots, Path::new(&query.path), false)?;
    let metadata = fs::metadata(&path).map_err(map_io_error)?;
    if !metadata.is_dir() {
        return Err(ApiError::bad_request());
    }
    let size = calculate_model_weight_size(&path)?;
    Ok(Json(ModelWeightSizeResponse { size }))
}

fn calculate_model_weight_size(root: &Path) -> Result<u64, ApiError> {
    let index = root.join("model_index.json");
    if index.is_file() {
        let value: Value = serde_json::from_slice(&fs::read(index).map_err(map_io_error)?)
            .map_err(|_| ApiError::bad_request())?;
        let object = value.as_object().ok_or_else(ApiError::bad_request)?;
        let mut total = 0_u64;
        for component in object.keys().filter(|name| !name.starts_with('_')) {
            if !valid_logical_name(component) {
                return Err(ApiError::bad_request());
            }
            let directory = root.join(component);
            if directory.is_dir() {
                total = total
                    .checked_add(weight_files_in_directory(&directory)?)
                    .ok_or_else(ApiError::internal)?;
            }
        }
        Ok(total)
    } else {
        weight_files_in_directory(root)
    }
}

fn weight_files_in_directory(directory: &Path) -> Result<u64, ApiError> {
    let mut total = 0_u64;
    for entry in fs::read_dir(directory).map_err(map_io_error)? {
        let entry = entry.map_err(map_io_error)?;
        let file_type = entry.file_type().map_err(map_io_error)?;
        if !file_type.is_file() {
            continue;
        }
        let accepted = entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| MODEL_WEIGHT_EXTENSIONS.contains(&extension));
        if accepted {
            total = total
                .checked_add(entry.metadata().map_err(map_io_error)?.len())
                .ok_or_else(ApiError::internal)?;
        }
    }
    Ok(total)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ServeLogRestart {
    previous: bool,
    started_at: Option<String>,
    containers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ServeLogOptionsResponse {
    restarts: Vec<ServeLogRestart>,
}

async fn serve_log_options(
    State(state): State<Arc<WorkerApiState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ServeLogOptionsResponse>, ApiError> {
    let id = valid_positive_id(&id)?;
    let restarts = discover_log_restarts(&state.log_dir.join("serve"), id)?;
    Ok(Json(ServeLogOptionsResponse { restarts }))
}

fn discover_log_restarts(directory: &Path, id: u64) -> Result<Vec<ServeLogRestart>, ApiError> {
    let mut by_restart: BTreeMap<u64, (PathBuf, BTreeSet<String>)> = BTreeMap::new();
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(map_io_error(error)),
    };
    for entry in entries {
        let entry = entry.map_err(map_io_error)?;
        if !entry.file_type().map_err(map_io_error)?.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if let Some(restart) = parse_main_log_name(&name, id) {
            by_restart
                .entry(restart)
                .or_insert_with(|| (entry.path(), BTreeSet::new()));
        } else if let Some((restart, container)) = parse_container_log_name(&name, id) {
            by_restart
                .entry(restart)
                .or_insert_with(|| {
                    (
                        directory.join(format!("{id}.{restart}.log")),
                        BTreeSet::new(),
                    )
                })
                .1
                .insert(container);
        }
    }
    let mut restarts = Vec::with_capacity(by_restart.len());
    for (position, (_restart, (path, containers))) in by_restart.into_iter().rev().enumerate() {
        if !path.is_file() {
            continue;
        }
        let started_at = path
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| unix_seconds_to_rfc3339(duration.as_secs()));
        restarts.push(ServeLogRestart {
            previous: position != 0,
            started_at,
            containers: containers.into_iter().collect(),
        });
    }
    Ok(restarts)
}

fn parse_main_log_name(name: &str, id: u64) -> Option<u64> {
    let middle = name.strip_prefix(&format!("{id}."))?.strip_suffix(".log")?;
    (!middle.contains('.'))
        .then(|| middle.parse().ok())
        .flatten()
}

fn parse_container_log_name(name: &str, id: u64) -> Option<(u64, String)> {
    let middle = name
        .strip_prefix(&format!("{id}.container."))?
        .strip_suffix(".log")?;
    if let Ok(restart) = middle.parse() {
        return Some((restart, "default".to_owned()));
    }
    let (container, restart) = middle.rsplit_once('.')?;
    let restart = restart.parse().ok()?;
    valid_logical_name(container).then(|| (restart, container.to_owned()))
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ServeLogsQuery {
    model_instance_name: String,
    model_file_id: Option<u64>,
    container_name: Option<String>,
    tail: i64,
    follow: bool,
    previous: bool,
}

impl Default for ServeLogsQuery {
    fn default() -> Self {
        Self {
            model_instance_name: String::new(),
            model_file_id: None,
            container_name: None,
            tail: -1,
            follow: false,
            previous: false,
        }
    }
}

async fn serve_logs(
    State(state): State<Arc<WorkerApiState>>,
    AxumPath(id): AxumPath<String>,
    query: Result<Query<ServeLogsQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let id = valid_positive_id(&id)?;
    let Query(query) = valid_query(query)?;
    validate_serve_logs_query(&query)?;
    let directory = state.log_dir.join("serve");
    let mut restarts: Vec<u64> = discover_main_restarts(&directory, id)?;
    restarts.sort_unstable_by(|left, right| right.cmp(left));
    let restart = select_restart(&restarts, query.previous)?;
    let main_path = directory.join(format!("{id}.{restart}.log"));
    let mut paths = Vec::new();
    if let Some(model_file_id) = query.model_file_id {
        let download = directory.join(format!("model_file_{model_file_id}.download.log"));
        if download.is_file() {
            paths.push(download);
        }
    }
    paths.push(main_path.clone());
    let initial = read_log_payload(&paths, query.tail)?;
    let body = if query.follow {
        follow_log_body(initial, main_path)
    } else {
        Body::from(initial)
    };
    Ok(([(CONTENT_TYPE, "application/octet-stream")], body).into_response())
}

fn validate_serve_logs_query(query: &ServeLogsQuery) -> Result<(), ApiError> {
    if query
        .container_name
        .as_deref()
        .is_some_and(|name| !name.is_empty())
    {
        return Err(ApiError::not_supported());
    }
    if query.tail < -1
        || !query.model_instance_name.is_empty() && !valid_logical_name(&query.model_instance_name)
    {
        return Err(ApiError::invalid());
    }
    Ok(())
}

fn select_restart(restarts_descending: &[u64], previous: bool) -> Result<u64, ApiError> {
    if previous && restarts_descending.len() >= 2 {
        Ok(restarts_descending[1])
    } else {
        restarts_descending
            .first()
            .copied()
            .ok_or_else(ApiError::not_found)
    }
}

fn discover_main_restarts(directory: &Path, id: u64) -> Result<Vec<u64>, ApiError> {
    let entries = fs::read_dir(directory).map_err(map_io_error)?;
    let mut restarts = Vec::new();
    for entry in entries {
        let entry = entry.map_err(map_io_error)?;
        if entry.file_type().map_err(map_io_error)?.is_file()
            && let Some(name) = entry.file_name().to_str()
            && let Some(restart) = parse_main_log_name(name, id)
        {
            restarts.push(restart);
        }
    }
    Ok(restarts)
}

fn read_log_payload(paths: &[PathBuf], tail: i64) -> Result<Vec<u8>, ApiError> {
    if tail > 0 {
        let content =
            fs::read(paths.last().ok_or_else(ApiError::not_found)?).map_err(map_io_error)?;
        return Ok(tail_lines(&content, tail as usize));
    }
    let mut content = Vec::new();
    for path in paths {
        content.extend(fs::read(path).map_err(map_io_error)?);
    }
    Ok(content)
}

fn tail_lines(content: &[u8], lines: usize) -> Vec<u8> {
    if lines == 0 {
        return Vec::new();
    }
    let mut newlines = 0;
    let mut start = 0;
    for (index, byte) in content.iter().enumerate().rev() {
        if *byte == b'\n' && index + 1 < content.len() {
            newlines += 1;
            if newlines == lines {
                start = index + 1;
                break;
            }
        }
    }
    content[start..].to_vec()
}

fn follow_log_body(initial: Vec<u8>, path: PathBuf) -> Body {
    let offset = fs::metadata(&path).map_or(0, |metadata| metadata.len());
    let stream = stream::unfold(
        (Some(initial), path, offset),
        |(mut initial, path, mut offset)| async move {
            if let Some(initial) = initial.take() {
                return Some((
                    Ok::<Bytes, Infallible>(Bytes::from(initial)),
                    (None, path, offset),
                ));
            }
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                let Ok(content) = fs::read(&path) else {
                    continue;
                };
                if content.len() as u64 > offset {
                    let chunk = Bytes::copy_from_slice(&content[offset as usize..]);
                    offset = content.len() as u64;
                    return Some((Ok(chunk), (None, path, offset)));
                }
                if (content.len() as u64) < offset {
                    // Log rotation/truncation restarts streaming at the beginning
                    // of the current file without reading any sibling path.
                    offset = 0;
                }
            }
        },
    );
    Body::from_stream(stream)
}

#[derive(Debug, Default, Serialize)]
pub struct ApiConfigView {
    debug: bool,
    system_default_container_registry: Option<String>,
    server_external_url: Option<String>,
    grafana_url: Option<String>,
    disable_builtin_observability: bool,
    grafana_worker_dashboard_uid: Option<String>,
    grafana_model_dashboard_uid: Option<String>,
}

pub fn public_config_view() -> ApiConfigView {
    ApiConfigView::default()
}

async fn get_config() -> Json<ApiConfigView> {
    Json(ApiConfigView::default())
}

async fn put_config() -> ApiError {
    ApiError::method_not_allowed()
}

async fn not_found() -> ApiError {
    ApiError::not_found()
}

fn valid_query<T>(query: Result<Query<T>, QueryRejection>) -> Result<Query<T>, ApiError> {
    query.map_err(|_| ApiError::invalid())
}

fn valid_positive_id(value: &str) -> Result<u64, ApiError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(ApiError::bad_request)
}

fn valid_logical_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && value != "."
        && value != ".."
}

fn resolve_model_path(
    roots: &[AllowedRoot],
    requested: &Path,
    allow_missing: bool,
) -> Result<PathBuf, ApiError> {
    if !requested.is_absolute()
        || requested
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(ApiError::bad_request());
    }
    let root = roots
        .iter()
        .find(|root| requested.starts_with(&root.configured))
        .ok_or_else(ApiError::forbidden)?;
    if path_exists(requested)? {
        let canonical = fs::canonicalize(requested).map_err(map_io_error)?;
        return canonical
            .starts_with(&root.canonical)
            .then_some(canonical)
            .ok_or_else(ApiError::forbidden);
    }
    if !allow_missing {
        return Err(ApiError::not_found());
    }
    let mut ancestor = requested;
    let mut missing = Vec::new();
    while !path_exists(ancestor)? {
        let name = ancestor.file_name().ok_or_else(ApiError::bad_request)?;
        missing.push(name.to_owned());
        ancestor = ancestor.parent().ok_or_else(ApiError::bad_request)?;
    }
    let canonical_ancestor = fs::canonicalize(ancestor).map_err(map_io_error)?;
    if !canonical_ancestor.starts_with(&root.canonical) {
        return Err(ApiError::forbidden());
    }
    let mut resolved = canonical_ancestor;
    for component in missing.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn path_exists(path: &Path) -> Result<bool, ApiError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(map_io_error(error)),
    }
}

fn map_io_error(error: io::Error) -> ApiError {
    match error.kind() {
        io::ErrorKind::NotFound => ApiError::not_found(),
        io::ErrorKind::PermissionDenied => ApiError::forbidden(),
        _ => ApiError::internal(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ErrorResponse {
    code: u16,
    reason: &'static str,
    message: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApiError {
    status: StatusCode,
    body: ErrorResponse,
}

impl ApiError {
    fn new(status: StatusCode, reason: &'static str, message: &'static str) -> Self {
        Self {
            status,
            body: ErrorResponse {
                code: status.as_u16(),
                reason,
                message,
            },
        }
    }

    fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "Unauthorized",
            "Invalid authentication credentials",
        )
    }

    fn bad_request() -> Self {
        Self::new(StatusCode::BAD_REQUEST, "BadRequest", "Invalid request")
    }

    fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "Forbidden", "Access denied")
    }

    fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "NotFound", "Resource not found")
    }

    fn payload_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PayloadTooLarge",
            "Model config file is too large",
        )
    }

    fn method_not_allowed() -> Self {
        Self::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "MethodNotAllowed",
            "Method not allowed",
        )
    }

    fn invalid() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Invalid",
            "Request validation failed",
        )
    }

    fn not_supported() -> Self {
        Self::new(
            StatusCode::NOT_IMPLEMENTED,
            "NotSupported",
            "Container logs are unavailable in resource-api-only mode",
        )
    }

    fn service_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "ServiceUnavailable",
            "Worker runtime is not ready in resource-api-only mode",
        )
    }

    fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServerError",
            "Internal server error",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::unix::fs::symlink;

    use futures_util::StreamExt;
    use tempfile::TempDir;

    use super::*;

    fn allowed_root(directory: &TempDir) -> AllowedRoot {
        AllowedRoot {
            configured: directory.path().to_owned(),
            canonical: fs::canonicalize(directory.path()).expect("canonical root"),
        }
    }

    #[test]
    fn bearer_has_priority_and_both_headers_use_constant_time_token_matching() {
        let token = SecretToken::new("contract-worker-token").expect("test token");
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            "contract-worker-token".parse().expect("header"),
        );
        assert!(authenticate(&headers, &token));

        headers.insert(AUTHORIZATION, "Bearer wrong-token".parse().expect("header"));
        assert!(!authenticate(&headers, &token));

        headers.insert(
            AUTHORIZATION,
            "Bearer contract-worker-token".parse().expect("header"),
        );
        assert!(authenticate(&headers, &token));
    }

    #[test]
    fn path_resolution_rejects_relative_parent_and_outside_paths() {
        let root = TempDir::new().expect("root");
        let roots = vec![allowed_root(&root)];

        assert_eq!(
            resolve_model_path(&roots, Path::new("relative"), true)
                .expect_err("relative path")
                .status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            resolve_model_path(&roots, &root.path().join("a/../b"), true)
                .expect_err("parent path")
                .status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            resolve_model_path(&roots, Path::new("/outside"), true)
                .expect_err("outside path")
                .status,
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn path_resolution_blocks_final_and_ancestor_symlink_escape() {
        let root = TempDir::new().expect("root");
        let outside = TempDir::new().expect("outside");
        fs::write(outside.path().join("config.json"), b"{}").expect("outside file");
        symlink(
            outside.path().join("config.json"),
            root.path().join("final.json"),
        )
        .expect("final symlink");
        symlink(outside.path(), root.path().join("ancestor")).expect("ancestor symlink");
        let roots = vec![allowed_root(&root)];

        for path in [
            root.path().join("final.json"),
            root.path().join("ancestor/config.json"),
            root.path().join("ancestor/missing.json"),
        ] {
            assert_eq!(
                resolve_model_path(&roots, &path, true)
                    .expect_err("symlink escape")
                    .status,
                StatusCode::FORBIDDEN
            );
        }
    }

    #[test]
    fn missing_path_inside_root_is_allowed_only_for_exists_checks() {
        let root = TempDir::new().expect("root");
        let roots = vec![allowed_root(&root)];
        let missing = root.path().join("model/missing.json");

        assert!(resolve_model_path(&roots, &missing, true).is_ok());
        assert_eq!(
            resolve_model_path(&roots, &missing, false)
                .expect_err("missing resource")
                .status,
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn model_weight_size_distinguishes_llm_and_diffusion_layouts() {
        let llm = TempDir::new().expect("llm");
        fs::write(llm.path().join("a.safetensors"), vec![0; 3]).expect("weight");
        fs::write(llm.path().join("b.BIN"), vec![0; 9]).expect("uppercase ignored");
        fs::create_dir(llm.path().join("nested")).expect("nested");
        fs::write(llm.path().join("nested/c.bin"), vec![0; 11]).expect("nested weight");
        assert_eq!(
            calculate_model_weight_size(llm.path()).expect("LLM size"),
            3
        );

        let diffusion = TempDir::new().expect("diffusion");
        fs::write(
            diffusion.path().join("model_index.json"),
            br#"{"_class_name":"Pipeline","unet":["x"],"vae":["y"]}"#,
        )
        .expect("index");
        for (component, size) in [("unet", 5), ("vae", 7), ("ignored", 13)] {
            fs::create_dir(diffusion.path().join(component)).expect("component");
            fs::write(
                diffusion.path().join(component).join("model.safetensors"),
                vec![0; size],
            )
            .expect("component weight");
        }
        assert_eq!(
            calculate_model_weight_size(diffusion.path()).expect("diffusion size"),
            12
        );
    }

    #[test]
    fn model_config_accepts_the_limit_and_rejects_one_extra_byte() {
        let directory = TempDir::new().expect("model directory");
        let accepted = directory.path().join("config.json");
        let mut exact = Vec::with_capacity(MAX_MODEL_CONFIG_BYTES as usize);
        exact.push(b'"');
        exact.resize(MAX_MODEL_CONFIG_BYTES as usize - 1, b'a');
        exact.push(b'"');
        fs::write(&accepted, exact).expect("exact boundary config");
        assert!(read_model_config(&accepted).is_ok());

        let oversized = directory.path().join("tokenizer.json");
        let mut too_large = vec![b'a'; MAX_MODEL_CONFIG_BYTES as usize + 1];
        too_large[0] = b'"';
        *too_large.last_mut().expect("last byte") = b'"';
        fs::write(&oversized, too_large).expect("oversized config");
        assert_eq!(
            read_model_config(&oversized)
                .expect_err("size limit")
                .status,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert!(!MODEL_CONFIG_NAMES.contains(&"weights.json"));
    }

    #[test]
    fn log_discovery_orders_restarts_and_rejects_container_reads() {
        let logs = TempDir::new().expect("logs");
        for name in [
            "7.1.log",
            "7.container.1.log",
            "7.2.log",
            "7.container.ray-head.2.log",
            "8.9.log",
        ] {
            fs::write(logs.path().join(name), format!("{name}\n")).expect("log");
        }
        let restarts = discover_log_restarts(logs.path(), 7).expect("discover logs");
        assert_eq!(restarts.len(), 2);
        assert!(!restarts[0].previous);
        assert_eq!(restarts[0].containers, vec!["ray-head"]);
        assert!(restarts[1].previous);
        assert_eq!(restarts[1].containers, vec!["default"]);

        let query = ServeLogsQuery {
            container_name: Some("default".to_owned()),
            ..ServeLogsQuery::default()
        };
        assert_eq!(
            validate_serve_logs_query(&query)
                .expect_err("container logs remain disabled")
                .status,
            StatusCode::NOT_IMPLEMENTED
        );
        assert_eq!(select_restart(&[2, 1], false).expect("current"), 2);
        assert_eq!(select_restart(&[2, 1], true).expect("previous"), 1);
        assert_eq!(select_restart(&[2], true).expect("current fallback"), 2);
    }

    #[test]
    fn tail_returns_exact_last_lines() {
        assert_eq!(tail_lines(b"one\ntwo\nthree\n", 2), b"two\nthree\n");
        assert_eq!(tail_lines(b"one\ntwo\n", 10), b"one\ntwo\n");
    }

    #[tokio::test]
    async fn follow_stream_emits_initial_then_appended_bytes() {
        let directory = TempDir::new().expect("logs");
        let path = directory.path().join("7.1.log");
        fs::write(&path, b"initial\n").expect("initial log");
        let body = follow_log_body(b"initial\n".to_vec(), path.clone());
        let mut stream = body.into_data_stream();

        let initial = stream
            .next()
            .await
            .expect("initial chunk")
            .expect("initial body");
        assert_eq!(initial, Bytes::from_static(b"initial\n"));
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open log");
        file.write_all(b"next\n").expect("append log");
        let appended = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
            .await
            .expect("follow timeout")
            .expect("follow chunk")
            .expect("follow body");
        assert_eq!(appended, Bytes::from_static(b"next\n"));
    }

    #[test]
    fn stable_error_fixtures_match_contract() {
        let fixture: Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/worker-v2.2.3/error-responses.json"
        ))
        .expect("error fixture");
        for (name, error) in [
            ("unauthorized", ApiError::unauthorized()),
            ("bad_request", ApiError::bad_request()),
            ("forbidden", ApiError::forbidden()),
            ("not_found", ApiError::not_found()),
            ("payload_too_large", ApiError::payload_too_large()),
            ("method_not_allowed", ApiError::method_not_allowed()),
            ("invalid", ApiError::invalid()),
            ("not_supported", ApiError::not_supported()),
            ("service_unavailable", ApiError::service_unavailable()),
        ] {
            let actual = serde_json::to_value(error.body).expect("error JSON");
            assert_eq!(actual, fixture[name], "error fixture {name}");
        }
    }

    #[test]
    fn config_view_is_fixed_and_contains_no_secret_fields() {
        let value = serde_json::to_value(ApiConfigView::default()).expect("config JSON");
        let expected: Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/worker-v2.2.3/config.response.json"
        ))
        .expect("config fixture");
        assert_eq!(value, expected);
        let text = value.to_string();
        assert!(!text.contains("token"));
        assert!(!text.contains("docker"));
    }
}
