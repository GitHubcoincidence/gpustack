//! Worker credential persistence and Server identity establishment.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gpustack_client::http::Transport;
use gpustack_client::types::{WorkerCreate, WorkerPublic, WorkerRegistrationPublic};
use gpustack_client::{ClientError, SecretToken, SecretTokenError, ServerClient};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const IDENTITY_RECORD_VERSION: u32 = 1;
const MAX_PRIVATE_FILE_BYTES: u64 = 16 * 1024;
const WORKER_PRIVATE_DIRECTORY: &str = "managed";
const WORKER_CREDENTIAL_FILE: &str = "worker-token";
const WORKER_IDENTITY_FILE: &str = "worker-identity.json";
static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentitySource {
    Persisted,
    Registered,
    Recovered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerIdentity {
    pub id: i64,
    pub name: String,
    pub worker_uuid: String,
    pub token: SecretToken,
    pub source: IdentitySource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerIdentityRecord {
    pub version: u32,
    pub worker_id: i64,
    pub worker_name: String,
    pub worker_uuid: String,
}

impl WorkerIdentityRecord {
    fn from_registration(worker: &WorkerRegistrationPublic) -> Result<Self, IdentityError> {
        let record = Self {
            version: IDENTITY_RECORD_VERSION,
            worker_id: worker.id,
            worker_name: worker.name.clone(),
            worker_uuid: worker.worker_uuid.clone(),
        };
        record.validate()?;
        Ok(record)
    }

    fn validate(&self) -> Result<(), IdentityError> {
        if self.version != IDENTITY_RECORD_VERSION {
            return Err(IdentityError::InvalidIdentityRecord(
                "unsupported identity record version",
            ));
        }
        if self.worker_id <= 0 {
            return Err(IdentityError::InvalidIdentityRecord(
                "Worker ID must be positive",
            ));
        }
        validate_identity_text(
            &self.worker_name,
            "Worker name is empty or has surrounding whitespace",
        )?;
        validate_identity_text(
            &self.worker_uuid,
            "Worker UUID is empty or has surrounding whitespace",
        )?;
        Ok(())
    }

    fn into_identity(self, token: SecretToken, source: IdentitySource) -> WorkerIdentity {
        WorkerIdentity {
            id: self.worker_id,
            name: self.worker_name,
            worker_uuid: self.worker_uuid,
            token,
            source,
        }
    }
}

fn validate_identity_text(value: &str, reason: &'static str) -> Result<(), IdentityError> {
    if value.is_empty() || value.trim() != value {
        return Err(IdentityError::InvalidIdentityRecord(reason));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct CredentialStore {
    path: PathBuf,
}

impl CredentialStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads a credential while rejecting links, broad permissions, and oversized files.
    pub fn load_optional(&self) -> Result<Option<SecretToken>, IdentityError> {
        let Some(mut bytes) = read_private_file(&self.path, PrivateFileKind::Credential)? else {
            return Ok(None);
        };
        while matches!(bytes.last(), Some(b'\r' | b'\n')) {
            bytes.pop();
        }
        let value = String::from_utf8(bytes).map_err(|_| IdentityError::UnsafeCredential {
            path: self.path.clone(),
            reason: "must contain valid UTF-8",
        })?;
        if value.trim() != value {
            return Err(IdentityError::UnsafeCredential {
                path: self.path.clone(),
                reason: "must not contain surrounding whitespace",
            });
        }
        SecretToken::new(value)
            .map(Some)
            .map_err(IdentityError::InvalidCredential)
    }

    pub fn load(&self) -> Result<SecretToken, IdentityError> {
        self.load_optional()?
            .ok_or_else(|| IdentityError::CredentialMissing(self.path.clone()))
    }

    /// Publishes a new credential atomically and never replaces an existing identity.
    pub fn persist_new(&self, token: &SecretToken) -> Result<(), IdentityError> {
        persist_private_file(&self.path, PrivateFileKind::Credential, |file| {
            token.write_secret(file)
        })
    }
}

#[derive(Debug, Clone)]
pub struct IdentityRecordStore {
    path: PathBuf,
}

impl IdentityRecordStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_optional(&self) -> Result<Option<WorkerIdentityRecord>, IdentityError> {
        let Some(bytes) = read_private_file(&self.path, PrivateFileKind::IdentityRecord)? else {
            return Ok(None);
        };
        let record: WorkerIdentityRecord =
            serde_json::from_slice(&bytes).map_err(IdentityError::IdentityRecordJson)?;
        record.validate()?;
        Ok(Some(record))
    }

    pub fn persist_new(&self, record: &WorkerIdentityRecord) -> Result<(), IdentityError> {
        record.validate()?;
        let bytes = serde_json::to_vec(record).map_err(IdentityError::IdentityRecordJson)?;
        persist_private_file(&self.path, PrivateFileKind::IdentityRecord, |file| {
            file.write_all(&bytes)
        })
    }
}

pub fn worker_credential_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(WORKER_PRIVATE_DIRECTORY)
        .join(WORKER_CREDENTIAL_FILE)
}

pub fn worker_identity_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(WORKER_PRIVATE_DIRECTORY)
        .join(WORKER_IDENTITY_FILE)
}

pub struct IdentityBootstrap<'a, T> {
    client: &'a ServerClient<T>,
    worker_credentials: CredentialStore,
    worker_identity: IdentityRecordStore,
}

impl<'a, T: Transport> IdentityBootstrap<'a, T> {
    pub fn new(
        client: &'a ServerClient<T>,
        worker_credentials: CredentialStore,
        worker_identity: IdentityRecordStore,
    ) -> Self {
        Self {
            client,
            worker_credentials,
            worker_identity,
        }
    }

    /// Establishes the Server-assigned identity without silently creating a second Worker.
    pub async fn establish(
        &self,
        registration_token: Option<&SecretToken>,
        request: &WorkerCreate,
    ) -> Result<WorkerIdentity, IdentityError> {
        let record = self.worker_identity.load_optional()?;
        let token = self.worker_credentials.load_optional()?;

        // A Worker token cannot recover the UUID because the `me` response omits it.
        // Treating the startup request UUID as equivalent could target a different Worker.
        if record.is_none() && token.is_some() {
            return Err(IdentityError::IdentityRecordMissing(
                self.worker_identity.path.clone(),
            ));
        }

        self.client.version().await?.verify_baseline()?;
        match (record, token) {
            (Some(record), Some(token)) => self.restore(record, token).await,
            (Some(record), None) => {
                self.recover_registration(record, registration_token, request)
                    .await
            }
            (None, None) => self.register_new(registration_token, request).await,
            (None, Some(_)) => unreachable!("token-only state is rejected above"),
        }
    }

    async fn restore(
        &self,
        record: WorkerIdentityRecord,
        token: SecretToken,
    ) -> Result<WorkerIdentity, IdentityError> {
        let worker = self.client.me(&token).await?;
        validate_current_worker(&worker, &record)?;
        Ok(record.into_identity(token, IdentitySource::Persisted))
    }

    async fn register_new(
        &self,
        registration_token: Option<&SecretToken>,
        request: &WorkerCreate,
    ) -> Result<WorkerIdentity, IdentityError> {
        let registration_token =
            registration_token.ok_or(IdentityError::RegistrationTokenRequired)?;
        let worker = self.client.register(registration_token, request).await?;
        let record = WorkerIdentityRecord::from_registration(&worker)?;

        // The durable Server identity is published before the token. A crash between
        // these writes is recoverable by re-registering with the saved Server UUID.
        self.worker_identity.persist_new(&record)?;
        self.worker_credentials.persist_new(&worker.token)?;
        Ok(record.into_identity(worker.token, IdentitySource::Registered))
    }

    async fn recover_registration(
        &self,
        record: WorkerIdentityRecord,
        registration_token: Option<&SecretToken>,
        request: &WorkerCreate,
    ) -> Result<WorkerIdentity, IdentityError> {
        let registration_token =
            registration_token.ok_or(IdentityError::RegistrationTokenRequired)?;
        let mut recovery_request = request.clone();
        recovery_request.name.clone_from(&record.worker_name);
        recovery_request.worker_uuid.clone_from(&record.worker_uuid);

        let worker = self
            .client
            .register(registration_token, &recovery_request)
            .await?;
        validate_registration(&worker, &record)?;
        self.worker_credentials.persist_new(&worker.token)?;
        Ok(record.into_identity(worker.token, IdentitySource::Recovered))
    }
}

fn validate_current_worker(
    worker: &WorkerPublic,
    record: &WorkerIdentityRecord,
) -> Result<(), IdentityError> {
    if worker.me != Some(true) {
        return Err(IdentityError::IdentityMismatch(
            "Server did not mark the credential as the current Worker",
        ));
    }
    if worker.id != record.worker_id {
        return Err(IdentityError::IdentityMismatch(
            "Server Worker ID differs from the persisted identity",
        ));
    }
    if worker.name != record.worker_name {
        return Err(IdentityError::IdentityMismatch(
            "Server Worker name differs from the persisted identity",
        ));
    }
    Ok(())
}

fn validate_registration(
    worker: &WorkerRegistrationPublic,
    record: &WorkerIdentityRecord,
) -> Result<(), IdentityError> {
    if worker.id != record.worker_id {
        return Err(IdentityError::IdentityMismatch(
            "re-registered Worker ID differs from the persisted identity",
        ));
    }
    if worker.name != record.worker_name {
        return Err(IdentityError::IdentityMismatch(
            "re-registered Worker name differs from the persisted identity",
        ));
    }
    if worker.worker_uuid != record.worker_uuid {
        return Err(IdentityError::IdentityMismatch(
            "re-registered Worker UUID differs from the persisted identity",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PrivateFileKind {
    Credential,
    IdentityRecord,
}

fn read_private_file(path: &Path, kind: PrivateFileKind) -> Result<Option<Vec<u8>>, IdentityError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);

    let file = match options.open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(IdentityError::Io {
                action: "open private identity file",
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let metadata = file.metadata().map_err(|source| IdentityError::Io {
        action: "inspect private identity file",
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(unsafe_private_file(kind, path, "must be a regular file"));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(unsafe_private_file(
            kind,
            path,
            "must not be accessible by group or other users",
        ));
    }
    if metadata.len() > MAX_PRIVATE_FILE_BYTES {
        return Err(unsafe_private_file(
            kind,
            path,
            "exceeds the 16 KiB private file size limit",
        ));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_PRIVATE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| IdentityError::Io {
            action: "read private identity file",
            path: path.to_path_buf(),
            source,
        })?;
    if bytes.len() as u64 > MAX_PRIVATE_FILE_BYTES {
        return Err(unsafe_private_file(
            kind,
            path,
            "exceeds the 16 KiB private file size limit",
        ));
    }
    Ok(Some(bytes))
}

fn persist_private_file(
    path: &Path,
    kind: PrivateFileKind,
    write_contents: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), IdentityError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| unsafe_private_file(kind, path, "must have a parent directory"))?;
    prepare_private_directory(parent, kind)?;
    match fs::symlink_metadata(path) {
        Ok(_) => return Err(private_file_already_exists(kind, path)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(IdentityError::Io {
                action: "inspect private identity target",
                path: path.to_path_buf(),
                source,
            });
        }
    }

    let temporary_path = temporary_path(path);
    let mut temporary_guard = TemporaryFileGuard(Some(temporary_path.clone()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary_path)
        .map_err(|source| IdentityError::Io {
            action: "create temporary private identity file",
            path: temporary_path.clone(),
            source,
        })?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|source| IdentityError::Io {
            action: "restrict temporary private identity file",
            path: temporary_path.clone(),
            source,
        })?;
    write_contents(&mut file)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|source| IdentityError::Io {
            action: "persist temporary private identity file",
            path: temporary_path.clone(),
            source,
        })?;
    drop(file);

    // Hard-link publication provides an atomic no-replace operation on the target filesystem.
    fs::hard_link(&temporary_path, path).map_err(|source| {
        if source.kind() == io::ErrorKind::AlreadyExists {
            private_file_already_exists(kind, path)
        } else {
            IdentityError::Io {
                action: "publish private identity file",
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    fs::remove_file(&temporary_path).map_err(|source| IdentityError::Io {
        action: "remove temporary private identity file",
        path: temporary_path.clone(),
        source,
    })?;
    temporary_guard.0 = None;
    sync_directory(parent)?;
    Ok(())
}

fn prepare_private_directory(path: &Path, kind: PrivateFileKind) -> Result<(), IdentityError> {
    fs::create_dir_all(path).map_err(|source| IdentityError::Io {
        action: "create private identity directory",
        path: path.to_path_buf(),
        source,
    })?;
    let metadata = fs::symlink_metadata(path).map_err(|source| IdentityError::Io {
        action: "inspect private identity directory",
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(unsafe_private_file(
            kind,
            path,
            "parent must be a real directory",
        ));
    }
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|source| {
        IdentityError::Io {
            action: "restrict private identity directory",
            path: path.to_path_buf(),
            source,
        }
    })?;
    Ok(())
}

fn unsafe_private_file(kind: PrivateFileKind, path: &Path, reason: &'static str) -> IdentityError {
    match kind {
        PrivateFileKind::Credential => IdentityError::UnsafeCredential {
            path: path.to_path_buf(),
            reason,
        },
        PrivateFileKind::IdentityRecord => IdentityError::UnsafeIdentityRecord {
            path: path.to_path_buf(),
            reason,
        },
    }
}

fn private_file_already_exists(kind: PrivateFileKind, path: &Path) -> IdentityError {
    match kind {
        PrivateFileKind::Credential => IdentityError::CredentialAlreadyExists(path.to_path_buf()),
        PrivateFileKind::IdentityRecord => {
            IdentityError::IdentityRecordAlreadyExists(path.to_path_buf())
        }
    }
}

fn temporary_path(target: &Path) -> PathBuf {
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "private".into());
    target
        .parent()
        .expect("private file target has a parent")
        .join(format!(
            ".{file_name}.tmp-{}-{sequence}",
            std::process::id()
        ))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), IdentityError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| IdentityError::Io {
            action: "sync private identity directory",
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), IdentityError> {
    Ok(())
}

struct TemporaryFileGuard(Option<PathBuf>);

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("credential does not exist: {0}")]
    CredentialMissing(PathBuf),
    #[error("credential already exists: {0}")]
    CredentialAlreadyExists(PathBuf),
    #[error("Worker identity record does not exist: {0}")]
    IdentityRecordMissing(PathBuf),
    #[error("Worker identity record already exists: {0}")]
    IdentityRecordAlreadyExists(PathBuf),
    #[error("unsafe credential at {path}: {reason}")]
    UnsafeCredential { path: PathBuf, reason: &'static str },
    #[error("unsafe Worker identity record at {path}: {reason}")]
    UnsafeIdentityRecord { path: PathBuf, reason: &'static str },
    #[error("invalid credential: {0}")]
    InvalidCredential(SecretTokenError),
    #[error("invalid Worker identity record JSON: {0}")]
    IdentityRecordJson(serde_json::Error),
    #[error("invalid Worker identity record: {0}")]
    InvalidIdentityRecord(&'static str),
    #[error("{action} failed for {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("registration token is required when no usable persisted Worker identity exists")]
    RegistrationTokenRequired,
    #[error("Worker identity mismatch: {0}")]
    IdentityMismatch(&'static str),
    #[error(transparent)]
    Client(#[from] ClientError),
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use gpustack_client::http::{HttpRequest, HttpResponse, StreamResponse, TransportError};
    use serde_json::Value;
    use url::Url;

    use super::*;

    #[derive(Clone)]
    struct QueueTransport {
        responses: Arc<Mutex<VecDeque<HttpResponse>>>,
        requests: Arc<Mutex<Vec<HttpRequest>>>,
    }

    impl QueueTransport {
        fn new(responses: impl IntoIterator<Item = HttpResponse>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses.into_iter().collect())),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn requests(&self) -> Vec<HttpRequest> {
            self.requests.lock().expect("request queue lock").clone()
        }
    }

    impl Transport for QueueTransport {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            self.requests
                .lock()
                .expect("request queue lock")
                .push(request);
            self.responses
                .lock()
                .expect("response queue lock")
                .pop_front()
                .ok_or_else(|| TransportError::Request("missing mock response".to_owned()))
        }

        async fn open_stream(
            &self,
            _request: HttpRequest,
        ) -> Result<StreamResponse, TransportError> {
            Err(TransportError::Request(
                "streaming is not used by identity tests".to_owned(),
            ))
        }
    }

    fn response(body: &[u8]) -> HttpResponse {
        response_with_status(200, body)
    }

    fn response_with_status(status: u16, body: &[u8]) -> HttpResponse {
        HttpResponse {
            status,
            content_type: Some("application/json".to_owned()),
            body: body.to_vec(),
        }
    }

    fn registration_request() -> WorkerCreate {
        serde_json::from_slice(include_bytes!(
            "../../../fixtures/server-v2.2.3/worker-register.request.json"
        ))
        .expect("registration fixture")
    }

    fn registration_response() -> WorkerRegistrationPublic {
        serde_json::from_slice(include_bytes!(
            "../../../fixtures/server-v2.2.3/worker-register.response.json"
        ))
        .expect("registration response fixture")
    }

    fn identity_record() -> WorkerIdentityRecord {
        WorkerIdentityRecord::from_registration(&registration_response())
            .expect("valid identity fixture")
    }

    fn stores(directory: &Path) -> (CredentialStore, IdentityRecordStore) {
        (
            CredentialStore::new(worker_credential_path(directory)),
            IdentityRecordStore::new(worker_identity_path(directory)),
        )
    }

    #[test]
    fn credential_round_trip_is_private_and_atomic() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (store, _) = stores(directory.path());
        let token = SecretToken::new("issued-secret").expect("valid token");

        store.persist_new(&token).expect("persist credential");

        assert_eq!(store.load().expect("load credential"), token);
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(store.path())
                .expect("credential metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let entries = fs::read_dir(store.path().parent().expect("credential parent"))
            .expect("read credential directory")
            .count();
        assert_eq!(entries, 1);
    }

    #[test]
    fn credential_store_rejects_broad_permissions_and_overwrite() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (store, _) = stores(directory.path());
        let first = SecretToken::new("first-secret").expect("valid token");
        store.persist_new(&first).expect("persist first credential");

        let overwrite = store.persist_new(&SecretToken::new("second-secret").expect("valid token"));
        assert!(matches!(
            overwrite,
            Err(IdentityError::CredentialAlreadyExists(_))
        ));

        #[cfg(unix)]
        {
            fs::set_permissions(store.path(), fs::Permissions::from_mode(0o644))
                .expect("broaden permissions");
            assert!(matches!(
                store.load(),
                Err(IdentityError::UnsafeCredential { .. })
            ));
        }
    }

    #[test]
    fn identity_record_round_trip_is_private_and_validated() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (_, store) = stores(directory.path());
        let record = identity_record();

        store.persist_new(&record).expect("persist identity");

        assert_eq!(store.load_optional().expect("load identity"), Some(record));
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(store.path())
                .expect("identity metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn identity_record_rejects_unknown_version() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (_, store) = stores(directory.path());
        let parent = store.path().parent().expect("identity parent");
        prepare_private_directory(parent, PrivateFileKind::IdentityRecord)
            .expect("prepare identity directory");
        fs::write(
            store.path(),
            br#"{"version":99,"worker_id":101,"worker_name":"worker","worker_uuid":"uuid"}"#,
        )
        .expect("write invalid identity");
        #[cfg(unix)]
        fs::set_permissions(store.path(), fs::Permissions::from_mode(0o600))
            .expect("restrict identity");

        assert!(matches!(
            store.load_optional(),
            Err(IdentityError::InvalidIdentityRecord(_))
        ));
    }

    #[tokio::test]
    async fn bootstrap_restores_server_assigned_identity() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (credentials, records) = stores(directory.path());
        let record = identity_record();
        records.persist_new(&record).expect("persist identity");
        credentials
            .persist_new(&SecretToken::new("persisted-secret").expect("valid token"))
            .expect("persist credential");
        let transport = QueueTransport::new([
            response(include_bytes!(
                "../../../fixtures/server-v2.2.3/live-version.response.json"
            )),
            response(include_bytes!(
                "../../../fixtures/server-v2.2.3/workers-me.response.json"
            )),
        ]);
        let client = ServerClient::new(Url::parse("http://server.test").expect("URL"), transport)
            .expect("client");

        let identity = IdentityBootstrap::new(&client, credentials, records)
            .establish(None, &registration_request())
            .await
            .expect("restore identity");

        assert_eq!(identity.id, 101);
        assert_eq!(identity.worker_uuid, record.worker_uuid);
        assert_eq!(identity.source, IdentitySource::Persisted);
        assert_eq!(
            identity.token,
            SecretToken::new("persisted-secret").expect("valid token")
        );
    }

    #[tokio::test]
    async fn bootstrap_persists_server_assigned_identity_before_the_token() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (credentials, records) = stores(directory.path());
        let transport = QueueTransport::new([
            response(include_bytes!(
                "../../../fixtures/server-v2.2.3/live-version.response.json"
            )),
            response(include_bytes!(
                "../../../fixtures/server-v2.2.3/worker-register.response.json"
            )),
        ]);
        let client = ServerClient::new(Url::parse("http://server.test").expect("URL"), transport)
            .expect("client");
        let registration_token = SecretToken::new("registration-secret").expect("valid token");
        let request = registration_request();

        let identity = IdentityBootstrap::new(&client, credentials.clone(), records.clone())
            .establish(Some(&registration_token), &request)
            .await
            .expect("register identity");

        assert_ne!(identity.worker_uuid, request.worker_uuid);
        assert_eq!(identity.worker_uuid, registration_response().worker_uuid);
        assert_eq!(identity.source, IdentitySource::Registered);
        assert_eq!(
            records.load_optional().expect("load identity"),
            Some(identity_record())
        );
        assert_eq!(
            credentials.load().expect("load issued token"),
            identity.token
        );
    }

    #[tokio::test]
    async fn bootstrap_recovers_metadata_only_state_with_the_server_uuid() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (credentials, records) = stores(directory.path());
        let record = identity_record();
        records.persist_new(&record).expect("persist identity");
        let transport = QueueTransport::new([
            response(include_bytes!(
                "../../../fixtures/server-v2.2.3/live-version.response.json"
            )),
            response(include_bytes!(
                "../../../fixtures/server-v2.2.3/worker-register.response.json"
            )),
        ]);
        let client = ServerClient::new(
            Url::parse("http://server.test").expect("URL"),
            transport.clone(),
        )
        .expect("client");
        let registration_token = SecretToken::new("registration-secret").expect("valid token");

        let identity = IdentityBootstrap::new(&client, credentials.clone(), records)
            .establish(Some(&registration_token), &registration_request())
            .await
            .expect("recover registration");

        assert_eq!(identity.source, IdentitySource::Recovered);
        assert_eq!(identity.worker_uuid, record.worker_uuid);
        assert_eq!(credentials.load().expect("recovered token"), identity.token);
        let requests = transport.requests();
        let recovered: WorkerCreate = serde_json::from_slice(
            requests[1]
                .body
                .as_deref()
                .expect("registration request body"),
        )
        .expect("recovery request");
        assert_eq!(recovered.name, record.worker_name);
        assert_eq!(recovered.worker_uuid, record.worker_uuid);
    }

    #[tokio::test]
    async fn bootstrap_rejects_a_token_without_identity_metadata_before_network_io() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (credentials, records) = stores(directory.path());
        credentials
            .persist_new(&SecretToken::new("persisted-secret").expect("valid token"))
            .expect("persist credential");
        let transport = QueueTransport::new([]);
        let client = ServerClient::new(
            Url::parse("http://server.test").expect("URL"),
            transport.clone(),
        )
        .expect("client");

        let error = IdentityBootstrap::new(&client, credentials, records)
            .establish(None, &registration_request())
            .await
            .expect_err("token-only identity must fail closed");

        assert!(matches!(error, IdentityError::IdentityRecordMissing(_)));
        assert!(transport.requests().is_empty());
    }

    #[tokio::test]
    async fn bootstrap_rejects_me_identity_mismatch() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (credentials, records) = stores(directory.path());
        records
            .persist_new(&identity_record())
            .expect("persist identity");
        credentials
            .persist_new(&SecretToken::new("persisted-secret").expect("valid token"))
            .expect("persist credential");
        let mut me: Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/server-v2.2.3/workers-me.response.json"
        ))
        .expect("me fixture");
        me["items"][0]["id"] = 999.into();
        let me = serde_json::to_vec(&me).expect("mismatched me response");
        let transport = QueueTransport::new([
            response(include_bytes!(
                "../../../fixtures/server-v2.2.3/live-version.response.json"
            )),
            response(&me),
        ]);
        let client = ServerClient::new(Url::parse("http://server.test").expect("URL"), transport)
            .expect("client");

        let error = IdentityBootstrap::new(&client, credentials, records)
            .establish(None, &registration_request())
            .await
            .expect_err("mismatched identity must fail closed");

        assert!(matches!(error, IdentityError::IdentityMismatch(_)));
    }

    #[tokio::test]
    async fn bootstrap_never_re_registers_after_a_persisted_token_is_rejected() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (credentials, records) = stores(directory.path());
        records
            .persist_new(&identity_record())
            .expect("persist identity");
        let persisted = SecretToken::new("persisted-secret").expect("valid token");
        credentials
            .persist_new(&persisted)
            .expect("persist credential");
        let transport = QueueTransport::new([
            response(include_bytes!(
                "../../../fixtures/server-v2.2.3/live-version.response.json"
            )),
            response_with_status(
                401,
                include_bytes!("../../../fixtures/server-v2.2.3/unauthenticated.response.json"),
            ),
        ]);
        let client = ServerClient::new(
            Url::parse("http://server.test").expect("URL"),
            transport.clone(),
        )
        .expect("client");
        let registration_token = SecretToken::new("registration-secret").expect("valid token");

        let error = IdentityBootstrap::new(&client, credentials.clone(), records)
            .establish(Some(&registration_token), &registration_request())
            .await
            .expect_err("persisted identity must fail closed");

        assert!(matches!(
            error,
            IdentityError::Client(ClientError::Api { status: 401, .. })
        ));
        assert_eq!(transport.requests().len(), 2);
        assert_eq!(credentials.load().expect("preserve credential"), persisted);
    }
}
