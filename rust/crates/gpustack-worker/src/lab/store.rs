use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use super::{LAB_SCHEMA_VERSION, LabDeployment, LabError, LabOwner, LabStore, LabStoreSession};

const MAX_STATE_FILE_BYTES: u64 = 1024 * 1024;
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct FsLabStore {
    root: PathBuf,
}

impl FsLabStore {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl LabStore for FsLabStore {
    fn lock(&self) -> Result<Box<dyn LabStoreSession>, LabError> {
        prepare_private_directory(&self.root)?;
        let deployments = self.root.join("deployments");
        let cache = self.root.join("cache");
        prepare_private_directory(&deployments)?;
        prepare_private_directory(&cache)?;

        let lock_path = self.root.join("lab.lock");
        let lock_file = open_lock_file(&lock_path)?;
        lock_exclusive_nonblocking(&lock_file)?;
        Ok(Box::new(FsLabSession {
            root: self.root.clone(),
            deployments,
            cache,
            _lock_file: lock_file,
        }))
    }
}

struct FsLabSession {
    root: PathBuf,
    deployments: PathBuf,
    cache: PathBuf,
    _lock_file: File,
}

impl LabStoreSession for FsLabSession {
    fn managed_lock_active(&self) -> Result<bool, LabError> {
        let data_root = self.root.parent().ok_or_else(|| {
            LabError::Unsafe("Lab state root must have a data directory parent".to_string())
        })?;
        lock_is_active(&data_root.join("managed/managed.lock"))
    }

    fn load_owner(&self) -> Result<Option<LabOwner>, LabError> {
        read_json_optional(&self.root.join("owner.json"))
    }

    fn save_owner(&mut self, owner: &LabOwner) -> Result<(), LabError> {
        if owner.schema_version != LAB_SCHEMA_VERSION {
            return Err(LabError::Invalid(
                "unsupported Lab owner schema".to_string(),
            ));
        }
        let path = self.root.join("owner.json");
        if path_exists(&path)? {
            return Err(LabError::Unsafe(
                "owner.json already exists and cannot be replaced".to_string(),
            ));
        }
        atomic_write_json(&path, owner, false)
    }

    fn list_deployments(&self) -> Result<Vec<LabDeployment>, LabError> {
        let mut deployments = Vec::new();
        for entry in read_directory(&self.deployments)? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let deployment: LabDeployment = read_json(&path)?;
            validate_deployment_file(&path, &deployment)?;
            deployments.push(deployment);
        }
        deployments.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(deployments)
    }

    fn save_deployment(&mut self, deployment: &LabDeployment) -> Result<(), LabError> {
        validate_identifier(&deployment.lab_id)?;
        if deployment.schema_version != LAB_SCHEMA_VERSION {
            return Err(LabError::Invalid(
                "unsupported Lab deployment schema".to_string(),
            ));
        }
        atomic_write_json(
            &self.deployments.join(format!("{}.json", deployment.lab_id)),
            deployment,
            true,
        )
    }

    fn remove_deployment(&mut self, lab_id: &str) -> Result<(), LabError> {
        validate_identifier(lab_id)?;
        remove_file_if_present(&self.deployments.join(format!("{lab_id}.json")))?;
        sync_directory(&self.deployments)
    }

    fn cache_path(&self, lab_id: &str) -> Result<PathBuf, LabError> {
        validate_identifier(lab_id)?;
        Ok(self.cache.join(lab_id))
    }

    fn prepare_cache(&mut self, lab_id: &str) -> Result<PathBuf, LabError> {
        let path = self.cache_path(lab_id)?;
        prepare_private_directory(&path)?;
        Ok(path)
    }

    fn list_cache_ids(&self) -> Result<Vec<String>, LabError> {
        let mut ids = Vec::new();
        for entry in read_directory(&self.cache)? {
            let file_type = entry.file_type().map_err(|error| {
                LabError::filesystem("inspect cache entry", entry.path(), error)
            })?;
            if !file_type.is_dir() || file_type.is_symlink() {
                return Err(LabError::Unsafe(format!(
                    "Lab cache entry is not a real directory: {}",
                    entry.path().display()
                )));
            }
            let id = entry
                .file_name()
                .into_string()
                .map_err(|_| LabError::Unsafe("Lab cache ID is not valid UTF-8".to_string()))?;
            validate_identifier(&id)?;
            ids.push(id);
        }
        ids.sort();
        Ok(ids)
    }

    fn remove_cache(&mut self, lab_id: &str) -> Result<(), LabError> {
        let path = self.cache_path(lab_id)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                fs::remove_dir_all(&path)
                    .map_err(|error| LabError::filesystem("remove Lab cache", &path, error))?;
                sync_directory(&self.cache)?;
            }
            Ok(_) => {
                return Err(LabError::Unsafe(format!(
                    "refusing to remove non-directory Lab cache {}",
                    path.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(LabError::filesystem("inspect Lab cache", path, error));
            }
        }
        Ok(())
    }
}

fn open_lock_file(path: &Path) -> Result<File, LabError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|error| LabError::filesystem("open Lab lock", path, error))?;
    validate_private_file(path, &file)?;
    Ok(file)
}

#[cfg(unix)]
fn lock_is_active(path: &Path) -> Result<bool, LabError> {
    use rustix::fs::{FlockOperation, flock};

    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW);
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(LabError::filesystem("open Managed lock", path, error)),
    };
    validate_private_file(path, &file)?;
    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => {
            flock(&file, FlockOperation::Unlock)
                .map_err(|error| LabError::filesystem("unlock Managed state", path, error))?;
            Ok(false)
        }
        Err(error) if error == rustix::io::Errno::WOULDBLOCK => Ok(true),
        Err(error) => Err(LabError::filesystem("inspect Managed lock", path, error)),
    }
}

#[cfg(not(unix))]
fn lock_is_active(_path: &Path) -> Result<bool, LabError> {
    Ok(false)
}

#[cfg(unix)]
fn lock_exclusive_nonblocking(file: &File) -> Result<(), LabError> {
    use rustix::fs::{FlockOperation, flock};

    flock(file, FlockOperation::NonBlockingLockExclusive).map_err(|error| {
        if error == rustix::io::Errno::WOULDBLOCK {
            LabError::Busy
        } else {
            LabError::filesystem("lock Lab state", "lab.lock", error)
        }
    })
}

#[cfg(not(unix))]
fn lock_exclusive_nonblocking(_file: &File) -> Result<(), LabError> {
    Ok(())
}

fn read_json_optional<T>(path: &Path) -> Result<Option<T>, LabError>
where
    T: serde::de::DeserializeOwned,
{
    match fs::symlink_metadata(path) {
        Ok(_) => read_json(path).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(LabError::filesystem("inspect Lab state", path, error)),
    }
}

fn read_json<T>(path: &Path) -> Result<T, LabError>
where
    T: serde::de::DeserializeOwned,
{
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let file = options
        .open(path)
        .map_err(|error| LabError::filesystem("open Lab state", path, error))?;
    validate_private_file(path, &file)?;
    let length = file
        .metadata()
        .map_err(|error| LabError::filesystem("inspect Lab state", path, error))?
        .len();
    if length > MAX_STATE_FILE_BYTES {
        return Err(LabError::Unsafe(format!(
            "Lab state file exceeds 1 MiB: {}",
            path.display()
        )));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(MAX_STATE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| LabError::filesystem("read Lab state", path, error))?;
    if bytes.len() as u64 > MAX_STATE_FILE_BYTES {
        return Err(LabError::Unsafe(format!(
            "Lab state file exceeds 1 MiB: {}",
            path.display()
        )));
    }
    serde_json::from_slice(&bytes).map_err(LabError::from)
}

fn atomic_write_json<T>(path: &Path, value: &T, replace: bool) -> Result<(), LabError>
where
    T: serde::Serialize,
{
    let parent = path
        .parent()
        .ok_or_else(|| LabError::Unsafe("Lab state target has no parent".to_string()))?;
    prepare_private_directory(parent)?;
    if path_exists(path)? {
        if !replace {
            return Err(LabError::Unsafe(format!(
                "Lab state already exists: {}",
                path.display()
            )));
        }
        let existing = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|error| LabError::filesystem("open Lab state", path, error))?;
        validate_private_file(path, &existing)?;
    }

    let temporary = temporary_path(path);
    let mut guard = TemporaryFileGuard(Some(temporary.clone()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .map_err(|error| LabError::filesystem("create temporary Lab state", &temporary, error))?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| LabError::filesystem("restrict temporary Lab state", &temporary, error))?;
    let mut encoded = serde_json::to_vec_pretty(value)?;
    encoded.push(b'\n');
    file.write_all(&encoded)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|error| LabError::filesystem("persist temporary Lab state", &temporary, error))?;
    drop(file);
    fs::rename(&temporary, path)
        .map_err(|error| LabError::filesystem("publish Lab state", path, error))?;
    guard.0 = None;
    sync_directory(parent)
}

fn validate_private_file(path: &Path, file: &File) -> Result<(), LabError> {
    let metadata = file
        .metadata()
        .map_err(|error| LabError::filesystem("inspect Lab state", path, error))?;
    if !metadata.is_file() {
        return Err(LabError::Unsafe(format!(
            "Lab state is not a regular file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(LabError::Unsafe(format!(
            "Lab state must use mode 0600: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_deployment_file(path: &Path, deployment: &LabDeployment) -> Result<(), LabError> {
    validate_identifier(&deployment.lab_id)?;
    let expected = format!("{}.json", deployment.lab_id);
    if path.file_name().and_then(|name| name.to_str()) != Some(&expected) {
        return Err(LabError::Unsafe(format!(
            "deployment ID does not match its filename: {}",
            path.display()
        )));
    }
    if deployment.spec.mode != crate::workload::WorkloadMode::Lab
        || deployment.spec.deployment_id != deployment.lab_id
    {
        return Err(LabError::Unsafe(format!(
            "deployment manifest identity is inconsistent: {}",
            path.display()
        )));
    }
    deployment
        .spec
        .verify_integrity()
        .map_err(|error| LabError::Unsafe(error.to_string()))?;
    Ok(())
}

fn prepare_private_directory(path: &Path) -> Result<(), LabError> {
    fs::create_dir_all(path)
        .map_err(|error| LabError::filesystem("create Lab directory", path, error))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LabError::filesystem("inspect Lab directory", path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LabError::Unsafe(format!(
            "Lab directory must not be a symlink: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| LabError::filesystem("restrict Lab directory", path, error))?;
    Ok(())
}

fn read_directory(path: &Path) -> Result<Vec<fs::DirEntry>, LabError> {
    fs::read_dir(path)
        .map_err(|error| LabError::filesystem("read Lab directory", path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| LabError::filesystem("read Lab directory entry", path, error))
}

fn path_exists(path: &Path) -> Result<bool, LabError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(LabError::filesystem("inspect Lab path", path, error)),
    }
}

fn remove_file_if_present(path: &Path) -> Result<(), LabError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(LabError::filesystem("remove Lab state", path, error)),
    }
}

fn validate_identifier(value: &str) -> Result<(), LabError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        || matches!(value, "." | "..")
    {
        return Err(LabError::Invalid(
            "Lab ID must contain 1-64 safe ASCII characters".to_string(),
        ));
    }
    Ok(())
}

fn temporary_path(target: &Path) -> PathBuf {
    let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "state".into());
    target
        .parent()
        .expect("Lab state target has a parent")
        .join(format!(".{name}.tmp-{}-{sequence}", std::process::id()))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), LabError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| LabError::filesystem("sync Lab directory", path, error))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), LabError> {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::lab::LAB_PORT_START;
    use crate::workload::{
        VllmOptions, WorkloadBuildRequest, WorkloadGpu, WorkloadMode, WorkloadSpecV1,
    };

    use super::*;

    fn deployment(root: &Path) -> LabDeployment {
        let spec = WorkloadSpecV1::build(WorkloadBuildRequest {
            mode: WorkloadMode::Lab,
            owner_id: "owner-a".to_string(),
            deployment_id: "lab-a".to_string(),
            managed_worker_uuid: None,
            model_path: root.join("model"),
            cache_path: root.join("cache/lab-a"),
            served_model_name: "demo".to_string(),
            requested_image:
                "runner@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            resolved_image_digest:
                "runner@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            gpus: vec![WorkloadGpu {
                uuid: "GPU-example".to_string(),
                host_index: 0,
            }],
            service_host: "127.0.0.1".to_string(),
            service_port: LAB_PORT_START,
            tensor_parallel_size: 1,
            vllm: VllmOptions::default(),
            environment: BTreeMap::new(),
        })
        .expect("workload spec");
        LabDeployment {
            schema_version: LAB_SCHEMA_VERSION,
            lab_id: "lab-a".to_string(),
            name: "demo".to_string(),
            state: super::super::LabState::Created,
            spec,
            container_id: None,
            last_error: None,
            created_at_unix: 1,
            updated_at_unix: 1,
        }
    }

    #[test]
    fn state_round_trip_is_private_and_replace_is_atomic() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = FsLabStore::new(directory.path().join("lab"));
        let mut session = store.lock().expect("lock Lab state");
        session
            .save_owner(&LabOwner {
                schema_version: LAB_SCHEMA_VERSION,
                owner_id: "owner-a".to_string(),
            })
            .expect("save owner");
        let mut expected = deployment(directory.path());
        session.save_deployment(&expected).expect("save deployment");
        expected.state = super::super::LabState::Starting;
        session
            .save_deployment(&expected)
            .expect("replace deployment");

        assert_eq!(
            session.load_owner().expect("owner").unwrap().owner_id,
            "owner-a"
        );
        assert_eq!(
            session.list_deployments().expect("deployments"),
            vec![expected]
        );
        #[cfg(unix)]
        {
            assert_eq!(
                fs::metadata(store.root())
                    .expect("root metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(store.root().join("owner.json"))
                    .expect("owner metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn second_writer_cannot_enter_the_lab_critical_section() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = FsLabStore::new(directory.path().join("lab"));
        let first = store.lock().expect("first lock");

        let second = store.lock();

        #[cfg(unix)]
        assert!(matches!(second, Err(LabError::Busy)));
        drop(first);
    }

    #[cfg(unix)]
    #[test]
    fn active_managed_lock_is_detected_without_taking_it_over() {
        use std::os::unix::fs::OpenOptionsExt;

        use rustix::fs::{FlockOperation, flock};

        let directory = tempfile::tempdir().expect("temporary directory");
        let data_root = directory.path().join("state");
        let managed_root = data_root.join("managed");
        fs::create_dir_all(&managed_root).expect("managed directory");
        let lock_path = managed_root.join("managed.lock");
        let lock_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&lock_path)
            .expect("managed lock file");
        flock(&lock_file, FlockOperation::LockExclusive).expect("hold Managed lock");
        let store = FsLabStore::new(data_root.join("lab"));
        let session = store.lock().expect("Lab lock");

        assert!(session.managed_lock_active().expect("Managed lock state"));
    }
}
