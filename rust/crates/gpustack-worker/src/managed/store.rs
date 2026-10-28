//! Private recovery records and current-session logs for Managed workloads.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::workload::{WorkloadMode, WorkloadSpecV2};

pub const MANAGED_MANIFEST_SCHEMA_VERSION: u8 = 1;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedRuntimeManifest {
    pub schema_version: u8,
    pub instance_id: i64,
    pub worker_uuid: String,
    pub spec: WorkloadSpecV2,
    pub container_id: String,
    pub restart_count: u32,
    pub started_at_unix: u64,
    pub last_observed_state: String,
}

impl ManagedRuntimeManifest {
    pub fn validate(&self) -> Result<(), ManagedStoreError> {
        if self.schema_version != MANAGED_MANIFEST_SCHEMA_VERSION
            || self.instance_id <= 0
            || self.spec.mode != WorkloadMode::Managed
            || self.spec.deployment_id != self.instance_id.to_string()
            || self.spec.labels.get(crate::workload::LABEL_WORKER_UUID) != Some(&self.worker_uuid)
            || self.container_id.trim().is_empty()
            || self.restart_count != 0
        {
            return Err(ManagedStoreError::Unsafe(
                "Managed manifest identity is inconsistent".to_string(),
            ));
        }
        self.spec
            .verify_integrity()
            .map_err(|error| ManagedStoreError::Unsafe(error.to_string()))
    }
}

pub trait ManagedRuntimeStore: Send + Sync + 'static {
    fn cache_root(&self) -> PathBuf;
    fn list_manifests(&self) -> Result<Vec<ManagedRuntimeManifest>, ManagedStoreError>;
    fn load_manifest(
        &self,
        instance_id: i64,
    ) -> Result<Option<ManagedRuntimeManifest>, ManagedStoreError>;
    fn save_manifest(&self, manifest: &ManagedRuntimeManifest) -> Result<(), ManagedStoreError>;
    fn remove_manifest(&self, instance_id: i64) -> Result<(), ManagedStoreError>;
    fn prepare_cache(&self, instance_id: i64) -> Result<PathBuf, ManagedStoreError>;
    fn remove_cache(&self, instance_id: i64) -> Result<(), ManagedStoreError>;
    fn prepare_current_log(
        &self,
        instance_id: i64,
        restart_count: u32,
    ) -> Result<PathBuf, ManagedStoreError>;
    fn remove_current_log(
        &self,
        instance_id: i64,
        restart_count: u32,
    ) -> Result<(), ManagedStoreError>;
}

pub struct FsManagedRuntimeStore {
    root: PathBuf,
    manifests: PathBuf,
    cache: PathBuf,
    logs: PathBuf,
    _lab_lock: File,
    _managed_lock: File,
}

impl FsManagedRuntimeStore {
    pub fn open(data_dir: &Path, log_dir: &Path) -> Result<Self, ManagedStoreError> {
        let lab_root = data_dir.join("lab");
        prepare_private_directory(&lab_root)?;
        let lab_lock_path = lab_root.join("lab.lock");
        let lab_lock = open_private_file(&lab_lock_path, false)?;
        lock_exclusive_nonblocking(&lab_lock)?;

        let root = data_dir.join("managed");
        let manifests = root.join("manifests");
        let cache = root.join("cache");
        let logs = log_dir.join("serve");
        for path in [&root, &manifests, &cache, &logs] {
            prepare_private_directory(path)?;
        }
        let lock_path = root.join("managed.lock");
        let managed_lock = open_private_file(&lock_path, false)?;
        lock_exclusive_nonblocking(&managed_lock)?;
        Ok(Self {
            root,
            manifests,
            cache,
            logs,
            _lab_lock: lab_lock,
            _managed_lock: managed_lock,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn manifest_path(&self, instance_id: i64) -> Result<PathBuf, ManagedStoreError> {
        validate_instance_id(instance_id)?;
        Ok(self.manifests.join(format!("{instance_id}.json")))
    }

    fn cache_path(&self, instance_id: i64) -> Result<PathBuf, ManagedStoreError> {
        validate_instance_id(instance_id)?;
        Ok(self.cache.join(instance_id.to_string()))
    }

    fn log_path(&self, instance_id: i64, restart_count: u32) -> Result<PathBuf, ManagedStoreError> {
        validate_instance_id(instance_id)?;
        if restart_count != 0 {
            return Err(ManagedStoreError::Unsafe(
                "M6-B supports only restart count 0".to_string(),
            ));
        }
        Ok(self.logs.join(format!("{instance_id}.{restart_count}.log")))
    }
}

impl ManagedRuntimeStore for FsManagedRuntimeStore {
    fn cache_root(&self) -> PathBuf {
        self.cache.clone()
    }

    fn list_manifests(&self) -> Result<Vec<ManagedRuntimeManifest>, ManagedStoreError> {
        let mut manifests = Vec::new();
        for entry in fs::read_dir(&self.manifests)
            .map_err(|error| filesystem("read Managed manifests", &self.manifests, error))?
        {
            let entry = entry.map_err(|error| {
                filesystem("read Managed manifest entry", &self.manifests, error)
            })?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let manifest: ManagedRuntimeManifest = read_json(&path)?;
            manifest.validate()?;
            if path.file_stem().and_then(|value| value.to_str())
                != Some(manifest.instance_id.to_string().as_str())
            {
                return Err(ManagedStoreError::Unsafe(format!(
                    "Managed manifest ID does not match filename: {}",
                    path.display()
                )));
            }
            manifests.push(manifest);
        }
        manifests.sort_by_key(|manifest| manifest.instance_id);
        Ok(manifests)
    }

    fn load_manifest(
        &self,
        instance_id: i64,
    ) -> Result<Option<ManagedRuntimeManifest>, ManagedStoreError> {
        let path = self.manifest_path(instance_id)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let manifest: ManagedRuntimeManifest = read_json(&path)?;
                manifest.validate()?;
                Ok(Some(manifest))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(filesystem("inspect Managed manifest", path, error)),
        }
    }

    fn save_manifest(&self, manifest: &ManagedRuntimeManifest) -> Result<(), ManagedStoreError> {
        manifest.validate()?;
        atomic_write_json(&self.manifest_path(manifest.instance_id)?, manifest)
    }

    fn remove_manifest(&self, instance_id: i64) -> Result<(), ManagedStoreError> {
        remove_file_if_present(&self.manifest_path(instance_id)?)?;
        sync_directory(&self.manifests)
    }

    fn prepare_cache(&self, instance_id: i64) -> Result<PathBuf, ManagedStoreError> {
        let path = self.cache_path(instance_id)?;
        prepare_private_directory(&path)?;
        Ok(path)
    }

    fn remove_cache(&self, instance_id: i64) -> Result<(), ManagedStoreError> {
        let path = self.cache_path(instance_id)?;
        remove_private_directory_if_present(&path)?;
        sync_directory(&self.cache)
    }

    fn prepare_current_log(
        &self,
        instance_id: i64,
        restart_count: u32,
    ) -> Result<PathBuf, ManagedStoreError> {
        let path = self.log_path(instance_id, restart_count)?;
        drop(open_private_file(&path, true)?);
        Ok(path)
    }

    fn remove_current_log(
        &self,
        instance_id: i64,
        restart_count: u32,
    ) -> Result<(), ManagedStoreError> {
        remove_file_if_present(&self.log_path(instance_id, restart_count)?)?;
        sync_directory(&self.logs)
    }
}

#[derive(Debug, Error)]
pub enum ManagedStoreError {
    #[error("Managed state store is busy; another Managed Worker holds the lock")]
    Busy,
    #[error("Managed state is unsafe: {0}")]
    Unsafe(String),
    #[error("Managed filesystem {action} failed for {path}: {message}")]
    Filesystem {
        action: &'static str,
        path: PathBuf,
        message: String,
    },
    #[error("cannot serialize Managed manifest: {0}")]
    Serialize(#[from] serde_json::Error),
}

fn validate_instance_id(instance_id: i64) -> Result<(), ManagedStoreError> {
    if instance_id <= 0 {
        return Err(ManagedStoreError::Unsafe(
            "ModelInstance ID must be positive".to_string(),
        ));
    }
    Ok(())
}

fn filesystem(
    action: &'static str,
    path: impl Into<PathBuf>,
    error: io::Error,
) -> ManagedStoreError {
    ManagedStoreError::Filesystem {
        action,
        path: path.into(),
        message: error.to_string(),
    }
}

fn prepare_private_directory(path: &Path) -> Result<(), ManagedStoreError> {
    fs::create_dir_all(path).map_err(|error| filesystem("create directory", path, error))?;
    let metadata =
        fs::symlink_metadata(path).map_err(|error| filesystem("inspect directory", path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ManagedStoreError::Unsafe(format!(
            "Managed directory must be a real directory: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| filesystem("restrict directory", path, error))?;
    Ok(())
}

fn open_private_file(path: &Path, truncate: bool) -> Result<File, ManagedStoreError> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .truncate(truncate);
    #[cfg(unix)]
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    let file = options
        .open(path)
        .map_err(|error| filesystem("open private file", path, error))?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| filesystem("restrict private file", path, error))?;
    validate_private_file(path, &file)?;
    Ok(file)
}

fn validate_private_file(path: &Path, file: &File) -> Result<(), ManagedStoreError> {
    let metadata = file
        .metadata()
        .map_err(|error| filesystem("inspect private file", path, error))?;
    if !metadata.is_file() {
        return Err(ManagedStoreError::Unsafe(format!(
            "Managed state is not a regular file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(ManagedStoreError::Unsafe(format!(
            "Managed state must use mode 0600: {}",
            path.display()
        )));
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ManagedStoreError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let file = options
        .open(path)
        .map_err(|error| filesystem("open manifest", path, error))?;
    validate_private_file(path, &file)?;
    let length = file
        .metadata()
        .map_err(|error| filesystem("inspect manifest", path, error))?
        .len();
    if length > MAX_MANIFEST_BYTES {
        return Err(ManagedStoreError::Unsafe(format!(
            "Managed manifest exceeds 1 MiB: {}",
            path.display()
        )));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| filesystem("read manifest", path, error))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ManagedStoreError::Unsafe(format!(
            "Managed manifest exceeds 1 MiB: {}",
            path.display()
        )));
    }
    serde_json::from_slice(&bytes).map_err(Into::into)
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ManagedStoreError> {
    let parent = path.parent().ok_or_else(|| {
        ManagedStoreError::Unsafe("Managed manifest has no parent directory".to_string())
    })?;
    prepare_private_directory(parent)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(ManagedStoreError::Unsafe(format!(
                "Managed manifest must not be a symlink: {}",
                path.display()
            )));
        }
        Ok(_) => {
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            options.custom_flags(libc::O_NOFOLLOW);
            let existing = options
                .open(path)
                .map_err(|error| filesystem("open existing manifest", path, error))?;
            validate_private_file(path, &existing)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(filesystem("inspect existing manifest", path, error)),
    }
    let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("manifest"),
        std::process::id(),
        sequence
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .map_err(|error| filesystem("create temporary manifest", &temporary, error))?;
    let result = (|| {
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| filesystem("restrict temporary manifest", &temporary, error))?;
        let mut encoded = serde_json::to_vec_pretty(value)?;
        encoded.push(b'\n');
        file.write_all(&encoded)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|error| filesystem("persist temporary manifest", &temporary, error))?;
        drop(file);
        fs::rename(&temporary, path)
            .map_err(|error| filesystem("publish manifest", path, error))?;
        sync_directory(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn remove_file_if_present(path: &Path) -> Result<(), ManagedStoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(filesystem("remove file", path, error)),
    }
}

fn remove_private_directory_if_present(path: &Path) -> Result<(), ManagedStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(path).map_err(|error| filesystem("remove cache", path, error))
        }
        Ok(_) => Err(ManagedStoreError::Unsafe(format!(
            "refusing to remove non-directory Managed cache: {}",
            path.display()
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(filesystem("inspect cache", path, error)),
    }
}

fn sync_directory(path: &Path) -> Result<(), ManagedStoreError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| filesystem("sync directory", path, error))
}

#[cfg(unix)]
fn lock_exclusive_nonblocking(file: &File) -> Result<(), ManagedStoreError> {
    use rustix::fs::{FlockOperation, flock};

    flock(file, FlockOperation::NonBlockingLockExclusive).map_err(|error| {
        if error == rustix::io::Errno::WOULDBLOCK {
            ManagedStoreError::Busy
        } else {
            filesystem("lock Managed state", "managed.lock", error.into())
        }
    })
}

#[cfg(not(unix))]
fn lock_exclusive_nonblocking(_file: &File) -> Result<(), ManagedStoreError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::workload::{
        VllmOptions, WorkloadBuildRequest, WorkloadGpu, WorkloadMode, WorkloadSpecV2,
    };

    fn managed_spec_fixture(root: &Path) -> WorkloadSpecV2 {
        let model = root.join("model");
        let cache = root.join("cache").join("301");
        fs::create_dir_all(&model).expect("model directory");
        fs::create_dir_all(&cache).expect("cache directory");
        WorkloadSpecV2::build(WorkloadBuildRequest {
            mode: WorkloadMode::Managed,
            owner_id: "worker-uuid".to_string(),
            deployment_id: "301".to_string(),
            managed_worker_uuid: Some("worker-uuid".to_string()),
            model_path: model,
            cache_path: cache,
            served_model_name: "qwen".to_string(),
            requested_image: format!("runner@sha256:{}", "a".repeat(64)),
            resolved_image_digest: format!("runner@sha256:{}", "a".repeat(64)),
            gpus: vec![WorkloadGpu {
                uuid: "GPU-test".to_string(),
                host_index: 0,
            }],
            service_host: "127.0.0.1".to_string(),
            service_port: 40_000,
            tensor_parallel_size: 1,
            container_user: "1000:1000".to_string(),
            vllm: VllmOptions::default(),
            environment: BTreeMap::new(),
        })
        .expect("Managed spec")
    }

    #[test]
    fn persists_private_manifest_cache_and_current_log() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = FsManagedRuntimeStore::open(directory.path(), directory.path())
            .expect("open Managed store");
        let spec = managed_spec_fixture(directory.path());
        let manifest = ManagedRuntimeManifest {
            schema_version: MANAGED_MANIFEST_SCHEMA_VERSION,
            instance_id: 301,
            worker_uuid: "worker-uuid".to_string(),
            spec,
            container_id: "container-301".to_string(),
            restart_count: 0,
            started_at_unix: 10,
            last_observed_state: "starting".to_string(),
        };

        store.prepare_cache(301).expect("cache");
        let log = store.prepare_current_log(301, 0).expect("log");
        store.save_manifest(&manifest).expect("save manifest");

        assert_eq!(store.load_manifest(301).expect("load"), Some(manifest));
        assert!(log.ends_with("301.0.log"));
        #[cfg(unix)]
        {
            assert_eq!(
                fs::metadata(&log)
                    .expect("log metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn rejects_a_second_managed_store_lock() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let _first = FsManagedRuntimeStore::open(directory.path(), directory.path())
            .expect("first Managed store");
        let second = FsManagedRuntimeStore::open(directory.path(), directory.path());
        #[cfg(unix)]
        assert!(matches!(second, Err(ManagedStoreError::Busy)));
    }

    #[cfg(unix)]
    #[test]
    fn managed_store_excludes_lab_sessions_for_its_full_lifetime() {
        use crate::lab::{FsLabStore, LabError, LabStore};

        let directory = tempfile::tempdir().expect("temporary directory");
        let _managed = FsManagedRuntimeStore::open(directory.path(), directory.path())
            .expect("Managed store");
        let lab = FsLabStore::new(directory.path().join("lab"));

        assert!(matches!(lab.lock(), Err(LabError::Busy)));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_replace_a_symlinked_manifest() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = FsManagedRuntimeStore::open(directory.path(), directory.path())
            .expect("open Managed store");
        let outside = directory.path().join("outside.json");
        fs::write(&outside, b"outside").expect("outside file");
        std::os::unix::fs::symlink(&outside, store.manifests.join("301.json"))
            .expect("manifest symlink");
        let manifest = ManagedRuntimeManifest {
            schema_version: MANAGED_MANIFEST_SCHEMA_VERSION,
            instance_id: 301,
            worker_uuid: "worker-uuid".to_string(),
            spec: managed_spec_fixture(directory.path()),
            container_id: "container-301".to_string(),
            restart_count: 0,
            started_at_unix: 10,
            last_observed_state: "starting".to_string(),
        };

        let error = store
            .save_manifest(&manifest)
            .expect_err("symlink replacement must fail");

        assert!(matches!(error, ManagedStoreError::Unsafe(_)));
        assert_eq!(fs::read(&outside).expect("outside file"), b"outside");
    }
}
