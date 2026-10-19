//! Worker credential persistence and Server identity establishment.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gpustack_client::http::Transport;
use gpustack_client::types::WorkerCreate;
use gpustack_client::{ClientError, SecretToken, SecretTokenError, ServerClient};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const MAX_CREDENTIAL_BYTES: u64 = 16 * 1024;
const WORKER_CREDENTIAL_DIRECTORY: &str = "managed";
const WORKER_CREDENTIAL_FILE: &str = "worker-token";
static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentitySource {
    Persisted,
    Registered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerIdentity {
    pub id: i64,
    pub name: String,
    pub worker_uuid: String,
    pub token: SecretToken,
    pub source: IdentitySource,
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
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NOFOLLOW);

        let mut file = match options.open(&self.path) {
            Ok(file) => file,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(IdentityError::Io {
                    action: "open credential",
                    path: self.path.clone(),
                    source,
                });
            }
        };
        let metadata = file.metadata().map_err(|source| IdentityError::Io {
            action: "inspect credential",
            path: self.path.clone(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(IdentityError::UnsafeCredential {
                path: self.path.clone(),
                reason: "must be a regular file",
            });
        }
        #[cfg(unix)]
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(IdentityError::UnsafeCredential {
                path: self.path.clone(),
                reason: "must not be accessible by group or other users",
            });
        }
        if metadata.len() > MAX_CREDENTIAL_BYTES {
            return Err(IdentityError::UnsafeCredential {
                path: self.path.clone(),
                reason: "exceeds the 16 KiB credential size limit",
            });
        }

        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut bytes)
            .map_err(|source| IdentityError::Io {
                action: "read credential",
                path: self.path.clone(),
                source,
            })?;
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
        let parent = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| IdentityError::UnsafeCredential {
                path: self.path.clone(),
                reason: "must have a parent directory",
            })?;
        prepare_private_directory(parent)?;
        match fs::symlink_metadata(&self.path) {
            Ok(_) => return Err(IdentityError::CredentialAlreadyExists(self.path.clone())),
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(IdentityError::Io {
                    action: "inspect credential target",
                    path: self.path.clone(),
                    source,
                });
            }
        }

        let temporary_path = temporary_path(parent);
        let mut temporary_guard = TemporaryFileGuard(Some(temporary_path.clone()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&temporary_path)
            .map_err(|source| IdentityError::Io {
                action: "create temporary credential",
                path: temporary_path.clone(),
                source,
            })?;
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|source| IdentityError::Io {
                action: "restrict temporary credential",
                path: temporary_path.clone(),
                source,
            })?;
        token
            .write_secret(&mut file)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|source| IdentityError::Io {
                action: "persist temporary credential",
                path: temporary_path.clone(),
                source,
            })?;
        drop(file);

        fs::hard_link(&temporary_path, &self.path).map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                IdentityError::CredentialAlreadyExists(self.path.clone())
            } else {
                IdentityError::Io {
                    action: "publish credential",
                    path: self.path.clone(),
                    source,
                }
            }
        })?;
        fs::remove_file(&temporary_path).map_err(|source| IdentityError::Io {
            action: "remove temporary credential",
            path: temporary_path.clone(),
            source,
        })?;
        temporary_guard.0 = None;
        sync_directory(parent)?;
        Ok(())
    }
}

pub fn worker_credential_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(WORKER_CREDENTIAL_DIRECTORY)
        .join(WORKER_CREDENTIAL_FILE)
}

pub struct IdentityBootstrap<'a, T> {
    client: &'a ServerClient<T>,
    worker_credentials: CredentialStore,
}

impl<'a, T: Transport> IdentityBootstrap<'a, T> {
    pub fn new(client: &'a ServerClient<T>, worker_credentials: CredentialStore) -> Self {
        Self {
            client,
            worker_credentials,
        }
    }

    /// Restores an issued Worker identity, or registers exactly once when none exists.
    pub async fn establish(
        &self,
        registration_token: Option<&SecretToken>,
        request: &WorkerCreate,
    ) -> Result<WorkerIdentity, IdentityError> {
        self.client.version().await?.verify_baseline()?;
        if let Some(token) = self.worker_credentials.load_optional()? {
            let worker = self.client.me(&token).await?;
            validate_current_worker(&worker.name, worker.me == Some(true), request)?;
            return Ok(WorkerIdentity {
                id: worker.id,
                name: worker.name,
                worker_uuid: request.worker_uuid.clone(),
                token,
                source: IdentitySource::Persisted,
            });
        }

        let registration_token =
            registration_token.ok_or(IdentityError::RegistrationTokenRequired)?;
        let worker = self.client.register(registration_token, request).await?;
        validate_current_worker(&worker.name, true, request)?;
        if worker.worker_uuid != request.worker_uuid {
            return Err(IdentityError::IdentityMismatch(
                "Server Worker UUID differs from the local identity",
            ));
        }
        self.worker_credentials.persist_new(&worker.token)?;
        Ok(WorkerIdentity {
            id: worker.id,
            name: worker.name,
            worker_uuid: worker.worker_uuid,
            token: worker.token,
            source: IdentitySource::Registered,
        })
    }
}

fn validate_current_worker(
    name: &str,
    is_current_worker: bool,
    request: &WorkerCreate,
) -> Result<(), IdentityError> {
    if !is_current_worker {
        return Err(IdentityError::IdentityMismatch(
            "Server did not mark the credential as the current Worker",
        ));
    }
    if name != request.name {
        return Err(IdentityError::IdentityMismatch(
            "Server Worker name differs from the local identity",
        ));
    }
    Ok(())
}

fn prepare_private_directory(path: &Path) -> Result<(), IdentityError> {
    fs::create_dir_all(path).map_err(|source| IdentityError::Io {
        action: "create credential directory",
        path: path.to_path_buf(),
        source,
    })?;
    let metadata = fs::symlink_metadata(path).map_err(|source| IdentityError::Io {
        action: "inspect credential directory",
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(IdentityError::UnsafeCredential {
            path: path.to_path_buf(),
            reason: "credential parent must be a real directory",
        });
    }
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|source| {
        IdentityError::Io {
            action: "restrict credential directory",
            path: path.to_path_buf(),
            source,
        }
    })?;
    Ok(())
}

fn temporary_path(parent: &Path) -> PathBuf {
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        ".{WORKER_CREDENTIAL_FILE}.tmp-{}-{sequence}",
        std::process::id()
    ))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), IdentityError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| IdentityError::Io {
            action: "sync credential directory",
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
    #[error("unsafe credential at {path}: {reason}")]
    UnsafeCredential { path: PathBuf, reason: &'static str },
    #[error("invalid credential: {0}")]
    InvalidCredential(SecretTokenError),
    #[error("{action} failed for {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("registration token is required when no persisted Worker identity exists")]
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

        fn request_count(&self) -> usize {
            self.requests.lock().expect("request queue lock").len()
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

    fn response(body: &'static [u8]) -> HttpResponse {
        response_with_status(200, body)
    }

    fn response_with_status(status: u16, body: &'static [u8]) -> HttpResponse {
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

    #[test]
    fn credential_round_trip_is_private_and_atomic() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = CredentialStore::new(worker_credential_path(directory.path()));
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
        let store = CredentialStore::new(worker_credential_path(directory.path()));
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

    #[tokio::test]
    async fn bootstrap_restores_persisted_worker_identity() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = CredentialStore::new(worker_credential_path(directory.path()));
        store
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

        let identity = IdentityBootstrap::new(&client, store)
            .establish(None, &registration_request())
            .await
            .expect("restore identity");

        assert_eq!(identity.id, 101);
        assert_eq!(identity.source, IdentitySource::Persisted);
        assert_eq!(
            identity.token,
            SecretToken::new("persisted-secret").expect("valid token")
        );
    }

    #[tokio::test]
    async fn bootstrap_registers_once_and_persists_issued_token() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = CredentialStore::new(worker_credential_path(directory.path()));
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

        let identity = IdentityBootstrap::new(&client, store.clone())
            .establish(Some(&registration_token), &registration_request())
            .await
            .expect("register identity");

        assert_eq!(identity.source, IdentitySource::Registered);
        assert_eq!(store.load().expect("load issued token"), identity.token);
    }

    #[tokio::test]
    async fn bootstrap_never_re_registers_after_a_persisted_token_is_rejected() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = CredentialStore::new(worker_credential_path(directory.path()));
        let persisted = SecretToken::new("persisted-secret").expect("valid token");
        store.persist_new(&persisted).expect("persist credential");
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

        let error = IdentityBootstrap::new(&client, store.clone())
            .establish(Some(&registration_token), &registration_request())
            .await
            .expect_err("persisted identity must fail closed");

        assert!(matches!(
            error,
            IdentityError::Client(ClientError::Api { status: 401, .. })
        ));
        assert_eq!(transport.request_count(), 2);
        assert_eq!(store.load().expect("preserve credential"), persisted);
    }
}
