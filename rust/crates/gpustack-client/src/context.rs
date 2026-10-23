//! Durable operator contexts and permission-checked token loading.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use thiserror::Error;

use crate::{Context, ContextError, SecretToken, SecretTokenError};

const CONTEXT_SCHEMA_VERSION: u32 = 1;
const MAX_CONTEXT_FILE_BYTES: u64 = 1024 * 1024;
const MAX_TOKEN_FILE_BYTES: u64 = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextDocument {
    version: u32,
    active: Option<String>,
    contexts: BTreeMap<String, Context>,
}

impl Default for ContextDocument {
    fn default() -> Self {
        Self {
            version: CONTEXT_SCHEMA_VERSION,
            active: None,
            contexts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextSnapshot {
    pub active: Option<String>,
    pub contexts: Vec<Context>,
}

#[derive(Debug, Clone)]
pub struct ContextStore {
    path: PathBuf,
}

impl ContextStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn discover() -> Result<Self, ContextStoreError> {
        if let Some(path) = env::var_os("GPUSTACK_RS_CONTEXTS_FILE") {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err(ContextStoreError::RelativeStorePath(path));
            }
            return Ok(Self::new(path));
        }

        let base = match env::var_os("XDG_CONFIG_HOME") {
            Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
            _ => env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".config"))
                .ok_or(ContextStoreError::ConfigHomeUnavailable)?,
        };
        Ok(Self::new(base.join("gpustack-rs").join("contexts.json")))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn snapshot(&self) -> Result<ContextSnapshot, ContextStoreError> {
        let document = self.load()?;
        Ok(ContextSnapshot {
            active: document.active,
            contexts: document.contexts.into_values().collect(),
        })
    }

    pub fn add(&self, context: Context, make_active: bool) -> Result<(), ContextStoreError> {
        context.validate()?;
        let mut document = self.load()?;
        if document.contexts.contains_key(&context.name) {
            return Err(ContextStoreError::AlreadyExists(context.name));
        }
        let name = context.name.clone();
        document.contexts.insert(name.clone(), context);
        if make_active || document.active.is_none() {
            document.active = Some(name);
        }
        self.persist(&document)
    }

    pub fn use_context(&self, name: &str) -> Result<Context, ContextStoreError> {
        let mut document = self.load()?;
        let context = document
            .contexts
            .get(name)
            .cloned()
            .ok_or_else(|| ContextStoreError::NotFound(name.to_owned()))?;
        document.active = Some(name.to_owned());
        self.persist(&document)?;
        Ok(context)
    }

    pub fn remove(&self, name: &str) -> Result<Context, ContextStoreError> {
        let mut document = self.load()?;
        let removed = document
            .contexts
            .remove(name)
            .ok_or_else(|| ContextStoreError::NotFound(name.to_owned()))?;
        if document.active.as_deref() == Some(name) {
            document.active = None;
        }
        self.persist(&document)?;
        Ok(removed)
    }

    pub fn resolve(&self, requested: Option<&str>) -> Result<Context, ContextStoreError> {
        let document = self.load()?;
        let name = requested
            .map(str::to_owned)
            .or(document.active)
            .ok_or(ContextStoreError::NoActiveContext)?;
        document
            .contexts
            .get(&name)
            .cloned()
            .ok_or(ContextStoreError::NotFound(name))
    }

    fn load(&self) -> Result<ContextDocument, ContextStoreError> {
        let Some(bytes) = read_private_file(&self.path, MAX_CONTEXT_FILE_BYTES)? else {
            return Ok(ContextDocument::default());
        };
        let document: ContextDocument = serde_json::from_slice(&bytes)
            .map_err(|source| ContextStoreError::Decode { source })?;
        if document.version != CONTEXT_SCHEMA_VERSION {
            return Err(ContextStoreError::UnsupportedVersion(document.version));
        }
        if document
            .active
            .as_ref()
            .is_some_and(|active| !document.contexts.contains_key(active))
        {
            return Err(ContextStoreError::InvalidDocument(
                "active context does not exist",
            ));
        }
        for (name, context) in &document.contexts {
            if name != &context.name {
                return Err(ContextStoreError::InvalidDocument(
                    "context key and embedded name differ",
                ));
            }
            context.validate()?;
        }
        Ok(document)
    }

    fn persist(&self, document: &ContextDocument) -> Result<(), ContextStoreError> {
        let parent = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| ContextStoreError::InvalidStorePath(self.path.clone()))?;
        prepare_private_directory(parent)?;
        reject_non_regular_target(&self.path)?;

        let mut bytes = serde_json::to_vec_pretty(document)
            .map_err(|source| ContextStoreError::Encode { source })?;
        bytes.push(b'\n');
        let temporary = temporary_path(&self.path);
        let mut guard = TemporaryFileGuard(Some(temporary.clone()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&temporary)
            .map_err(|source| io_error("create temporary context file", &temporary, source))?;
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|source| io_error("restrict temporary context file", &temporary, source))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|source| io_error("write temporary context file", &temporary, source))?;
        fs::rename(&temporary, &self.path)
            .map_err(|source| io_error("publish context file", &self.path, source))?;
        guard.0 = None;
        Ok(())
    }
}

/// Loads an operator credential without allowing links, broad modes, or oversized files.
pub fn load_context_token(context: &Context) -> Result<SecretToken, ContextStoreError> {
    context.validate()?;
    let mut bytes = read_private_file(&context.token_file, MAX_TOKEN_FILE_BYTES)?
        .ok_or_else(|| ContextStoreError::TokenMissing(context.token_file.clone()))?;
    while matches!(bytes.last(), Some(b'\r' | b'\n')) {
        bytes.pop();
    }
    let value = String::from_utf8(bytes).map_err(|_| ContextStoreError::UnsafePrivateFile {
        path: context.token_file.clone(),
        reason: "token must contain valid UTF-8",
    })?;
    if value.trim() != value {
        return Err(ContextStoreError::UnsafePrivateFile {
            path: context.token_file.clone(),
            reason: "token must not contain surrounding whitespace",
        });
    }
    SecretToken::new(value).map_err(ContextStoreError::InvalidToken)
}

fn read_private_file(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, ContextStoreError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let file = match options.open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error("open private file", path, source)),
    };
    let metadata = file
        .metadata()
        .map_err(|source| io_error("inspect private file", path, source))?;
    if !metadata.is_file() {
        return Err(ContextStoreError::UnsafePrivateFile {
            path: path.to_path_buf(),
            reason: "must be a regular file",
        });
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(ContextStoreError::UnsafePrivateFile {
            path: path.to_path_buf(),
            reason: "must not be accessible by group or other users",
        });
    }
    if metadata.len() > limit {
        return Err(ContextStoreError::UnsafePrivateFile {
            path: path.to_path_buf(),
            reason: "exceeds the private file size limit",
        });
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| io_error("read private file", path, source))?;
    if bytes.len() as u64 > limit {
        return Err(ContextStoreError::UnsafePrivateFile {
            path: path.to_path_buf(),
            reason: "exceeds the private file size limit",
        });
    }
    Ok(Some(bytes))
}

fn prepare_private_directory(path: &Path) -> Result<(), ContextStoreError> {
    fs::create_dir_all(path)
        .map_err(|source| io_error("create context directory", path, source))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| io_error("inspect context directory", path, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ContextStoreError::UnsafePrivateFile {
            path: path.to_path_buf(),
            reason: "context parent must be a real directory",
        });
    }
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|source| io_error("restrict context directory", path, source))?;
    }
    Ok(())
}

fn reject_non_regular_target(path: &Path) -> Result<(), ContextStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(ContextStoreError::UnsafePrivateFile {
            path: path.to_path_buf(),
            reason: "context target must be a regular file",
        }),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error("inspect context target", path, source)),
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("contexts.json");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    path.with_file_name(format!(".{file_name}.{}.{nonce}.tmp", std::process::id()))
}

struct TemporaryFileGuard(Option<PathBuf>);

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(path);
        }
    }
}

fn io_error(action: &'static str, path: &Path, source: io::Error) -> ContextStoreError {
    ContextStoreError::Io {
        action,
        path: path.to_path_buf(),
        source,
    }
}

#[derive(Debug, Error)]
pub enum ContextStoreError {
    #[error("context store path must be absolute: {0}")]
    RelativeStorePath(PathBuf),
    #[error("HOME or an absolute XDG_CONFIG_HOME is required for the context store")]
    ConfigHomeUnavailable,
    #[error("invalid context store path: {0}")]
    InvalidStorePath(PathBuf),
    #[error("context '{0}' already exists")]
    AlreadyExists(String),
    #[error("context '{0}' was not found")]
    NotFound(String),
    #[error("no active context is configured; use --context or 'context use'")]
    NoActiveContext,
    #[error("unsupported context store version {0}")]
    UnsupportedVersion(u32),
    #[error("invalid context store: {0}")]
    InvalidDocument(&'static str),
    #[error("unsafe private file {path}: {reason}")]
    UnsafePrivateFile { path: PathBuf, reason: &'static str },
    #[error("operator token file does not exist: {0}")]
    TokenMissing(PathBuf),
    #[error("invalid context: {0}")]
    InvalidContext(#[from] ContextError),
    #[error("invalid operator token: {0}")]
    InvalidToken(SecretTokenError),
    #[error("failed to decode context store: {source}")]
    Decode { source: serde_json::Error },
    #[error("failed to encode context store: {source}")]
    Encode { source: serde_json::Error },
    #[error("failed to {action} at {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use url::Url;

    fn context(root: &Path, name: &str) -> Context {
        Context {
            name: name.to_owned(),
            server_url: Url::parse("http://127.0.0.1/base").expect("URL"),
            token_file: root.join(format!("{name}.token")),
        }
    }

    #[test]
    fn context_lifecycle_is_atomic_and_never_stores_token_values() {
        let temporary = tempdir().expect("temporary directory");
        let store = ContextStore::new(temporary.path().join("config/contexts.json"));
        let first = context(temporary.path(), "first");
        let second = context(temporary.path(), "second");

        store.add(first.clone(), false).expect("add first");
        assert_eq!(store.resolve(None).expect("active context"), first);
        store.add(second.clone(), false).expect("add second");
        assert_eq!(store.use_context("second").expect("use second"), second);
        assert_eq!(store.snapshot().expect("snapshot").contexts.len(), 2);
        assert_eq!(store.remove("second").expect("remove second"), second);
        assert!(matches!(
            store.resolve(None),
            Err(ContextStoreError::NoActiveContext)
        ));

        let raw = fs::read_to_string(store.path()).expect("read contexts");
        assert!(raw.contains("token_file"));
        assert!(!raw.contains("token_value"));
    }

    #[test]
    fn token_loader_trims_line_endings_and_rejects_surrounding_spaces() {
        let temporary = tempdir().expect("temporary directory");
        let context = context(temporary.path(), "operator");
        fs::write(&context.token_file, b"operator-secret\r\n").expect("write token");
        #[cfg(unix)]
        fs::set_permissions(&context.token_file, fs::Permissions::from_mode(0o600))
            .expect("restrict token");

        let token = load_context_token(&context).expect("load token");
        assert!(!format!("{token:?}").contains("operator-secret"));

        fs::write(&context.token_file, b" operator-secret\n").expect("rewrite token");
        let error = load_context_token(&context).expect_err("spaces are rejected");
        assert!(matches!(error, ContextStoreError::UnsafePrivateFile { .. }));
        assert!(!error.to_string().contains("operator-secret"));
    }

    #[cfg(unix)]
    #[test]
    fn token_loader_rejects_broad_permissions_and_symlinks() {
        use std::os::unix::fs::symlink;

        let temporary = tempdir().expect("temporary directory");
        let context = context(temporary.path(), "operator");
        fs::write(&context.token_file, b"secret").expect("write token");
        fs::set_permissions(&context.token_file, fs::Permissions::from_mode(0o644))
            .expect("set broad mode");
        assert!(matches!(
            load_context_token(&context),
            Err(ContextStoreError::UnsafePrivateFile { .. })
        ));

        fs::remove_file(&context.token_file).expect("remove token");
        let real = temporary.path().join("real.token");
        fs::write(&real, b"secret").expect("write real token");
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).expect("restrict real token");
        symlink(&real, &context.token_file).expect("create symlink");
        assert!(load_context_token(&context).is_err());
    }
}
