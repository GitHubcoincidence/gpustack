//! Safe resolution of local-path model artifacts for Managed workloads.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModelFile {
    pub paths: Vec<PathBuf>,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct LocalModelResolver {
    roots: Vec<PathBuf>,
}

impl LocalModelResolver {
    pub fn new<I>(roots: I) -> Result<Self, ResolveError>
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let mut canonical_roots = Vec::new();
        for root in roots {
            if !root.is_absolute() {
                return Err(ResolveError::RootNotAbsolute);
            }
            let canonical = fs::canonicalize(root).map_err(|error| ResolveError::Io {
                operation: "canonicalize model root",
                kind: error.kind(),
            })?;
            if !canonical.is_dir() {
                return Err(ResolveError::RootNotDirectory);
            }
            canonical_roots.push(canonical);
        }
        if canonical_roots.is_empty() {
            return Err(ResolveError::EmptyRoots);
        }
        canonical_roots.sort();
        canonical_roots.dedup();
        Ok(Self {
            roots: canonical_roots,
        })
    }

    pub fn resolve(&self, input: &Path) -> Result<ResolvedModelFile, ResolveError> {
        if !input.is_absolute() {
            return Err(ResolveError::InputNotAbsolute);
        }
        let canonical = fs::canonicalize(input).map_err(|error| ResolveError::Io {
            operation: "canonicalize model path",
            kind: error.kind(),
        })?;
        self.require_allowed(&canonical)?;

        let metadata = fs::metadata(&canonical).map_err(|error| ResolveError::Io {
            operation: "read model metadata",
            kind: error.kind(),
        })?;
        if metadata.is_dir() {
            let size = self.path_size(&canonical, &mut HashSet::new())?;
            return Ok(ResolvedModelFile {
                paths: vec![canonical],
                size,
            });
        }
        if !metadata.is_file() {
            return Err(ResolveError::UnsupportedFileType);
        }

        let paths = self.resolve_shards(&canonical)?;
        let size = paths.iter().try_fold(0_u64, |total, path| {
            let length = fs::metadata(path)
                .map_err(|error| ResolveError::Io {
                    operation: "read model file metadata",
                    kind: error.kind(),
                })?
                .len();
            total.checked_add(length).ok_or(ResolveError::SizeOverflow)
        })?;
        Ok(ResolvedModelFile { paths, size })
    }

    fn resolve_shards(&self, input: &Path) -> Result<Vec<PathBuf>, ResolveError> {
        let Some(target) = input.file_name().and_then(|name| name.to_str()) else {
            return Err(ResolveError::NonUtf8Path);
        };
        let Some(target_shard) = ShardName::parse(target) else {
            return Ok(vec![input.to_path_buf()]);
        };
        let parent = input.parent().ok_or(ResolveError::InvalidShardSet)?;
        let mut matches = BTreeMap::new();

        for entry in fs::read_dir(parent).map_err(|error| ResolveError::Io {
            operation: "read shard directory",
            kind: error.kind(),
        })? {
            let entry = entry.map_err(|error| ResolveError::Io {
                operation: "read shard entry",
                kind: error.kind(),
            })?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(candidate) = ShardName::parse(&name) else {
                continue;
            };
            if candidate.prefix != target_shard.prefix
                || candidate.extension != target_shard.extension
            {
                continue;
            }
            if candidate.total != target_shard.total {
                return Err(ResolveError::InconsistentShardTotal);
            }
            let canonical = fs::canonicalize(entry.path()).map_err(|error| ResolveError::Io {
                operation: "canonicalize shard",
                kind: error.kind(),
            })?;
            self.require_allowed(&canonical)?;
            if !canonical.is_file() || matches.insert(candidate.index, canonical).is_some() {
                return Err(ResolveError::InvalidShardSet);
            }
        }

        if matches.len() != usize::from(target_shard.total)
            || !(1..=target_shard.total).all(|index| matches.contains_key(&index))
        {
            return Err(ResolveError::IncompleteShardSet);
        }
        Ok(matches.into_values().collect())
    }

    fn path_size(&self, path: &Path, visited: &mut HashSet<PathBuf>) -> Result<u64, ResolveError> {
        let canonical = fs::canonicalize(path).map_err(|error| ResolveError::Io {
            operation: "canonicalize model tree entry",
            kind: error.kind(),
        })?;
        self.require_allowed(&canonical)?;
        if !visited.insert(canonical.clone()) {
            return Ok(0);
        }

        let metadata = fs::metadata(&canonical).map_err(|error| ResolveError::Io {
            operation: "read model tree metadata",
            kind: error.kind(),
        })?;
        if metadata.is_file() {
            return Ok(metadata.len());
        }
        if !metadata.is_dir() {
            return Err(ResolveError::UnsupportedFileType);
        }

        fs::read_dir(&canonical)
            .map_err(|error| ResolveError::Io {
                operation: "read model directory",
                kind: error.kind(),
            })?
            .try_fold(0_u64, |total, entry| {
                let entry = entry.map_err(|error| ResolveError::Io {
                    operation: "read model directory entry",
                    kind: error.kind(),
                })?;
                let length = self.path_size(&entry.path(), visited)?;
                total.checked_add(length).ok_or(ResolveError::SizeOverflow)
            })
    }

    fn require_allowed(&self, path: &Path) -> Result<(), ResolveError> {
        if self.roots.iter().any(|root| path.starts_with(root)) {
            Ok(())
        } else {
            Err(ResolveError::OutsideAllowedRoots)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShardName {
    prefix: String,
    extension: String,
    index: u16,
    total: u16,
}

impl ShardName {
    fn parse(name: &str) -> Option<Self> {
        let (stem, extension) = name.rsplit_once('.')?;
        let (numbered, total) = stem.rsplit_once("-of-")?;
        let (prefix, index) = numbered.rsplit_once('-')?;
        if prefix.is_empty() || extension.is_empty() {
            return None;
        }
        let index = parse_five_digits(index)?;
        let total = parse_five_digits(total)?;
        if index == 0 || total == 0 || index > total {
            return None;
        }
        Some(Self {
            prefix: prefix.to_owned(),
            extension: extension.to_owned(),
            index,
            total,
        })
    }
}

fn parse_five_digits(value: &str) -> Option<u16> {
    if value.len() != 5 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResolveError {
    #[error("at least one model root is required")]
    EmptyRoots,
    #[error("model roots must be absolute")]
    RootNotAbsolute,
    #[error("model root must be a directory")]
    RootNotDirectory,
    #[error("local model path must be absolute")]
    InputNotAbsolute,
    #[error("local model path escapes the configured model roots")]
    OutsideAllowedRoots,
    #[error("local model path contains unsupported non-UTF-8 components")]
    NonUtf8Path,
    #[error("local model path has an unsupported file type")]
    UnsupportedFileType,
    #[error("sharded model files declare inconsistent totals")]
    InconsistentShardTotal,
    #[error("sharded model files are incomplete")]
    IncompleteShardSet,
    #[error("sharded model files are invalid")]
    InvalidShardSet,
    #[error("model artifact size exceeds the supported range")]
    SizeOverflow,
    #[error("failed to {operation}: {kind:?}")]
    Io {
        operation: &'static str,
        kind: ErrorKind,
    },
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn resolves_directory_and_counts_nested_files() {
        let root = tempfile::tempdir().expect("model root");
        let model = root.path().join("model");
        fs::create_dir(&model).expect("model directory");
        fs::create_dir(model.join("nested")).expect("nested directory");
        fs::write(model.join("config.json"), b"1234").expect("config");
        fs::write(model.join("nested/weights.bin"), b"123456").expect("weights");
        let resolver = LocalModelResolver::new([root.path().to_path_buf()]).expect("resolver");

        let resolved = resolver.resolve(&model).expect("resolved model");

        assert_eq!(
            resolved.paths,
            [fs::canonicalize(model).expect("canonical")]
        );
        assert_eq!(resolved.size, 10);
    }

    #[test]
    fn expands_a_complete_shard_set_in_stable_order() {
        let root = tempfile::tempdir().expect("model root");
        let first = root.path().join("model-00001-of-00003.safetensors");
        let second = root.path().join("model-00002-of-00003.safetensors");
        let third = root.path().join("model-00003-of-00003.safetensors");
        fs::write(&third, b"333").expect("third shard");
        fs::write(&first, b"1").expect("first shard");
        fs::write(&second, b"22").expect("second shard");
        let resolver = LocalModelResolver::new([root.path().to_path_buf()]).expect("resolver");

        let resolved = resolver.resolve(&second).expect("resolved shards");

        assert_eq!(resolved.size, 6);
        assert_eq!(
            resolved.paths,
            [first, second, third].map(|path| fs::canonicalize(path).expect("canonical shard"))
        );
    }

    #[test]
    fn rejects_incomplete_and_inconsistent_shards() {
        let root = tempfile::tempdir().expect("model root");
        let first = root.path().join("model-00001-of-00003.safetensors");
        fs::write(&first, b"1").expect("first shard");
        fs::write(root.path().join("model-00002-of-00002.safetensors"), b"2")
            .expect("inconsistent shard");
        let resolver = LocalModelResolver::new([root.path().to_path_buf()]).expect("resolver");

        assert_eq!(
            resolver.resolve(&first).expect_err("inconsistent total"),
            ResolveError::InconsistentShardTotal
        );

        fs::remove_file(root.path().join("model-00002-of-00002.safetensors"))
            .expect("remove inconsistent shard");
        assert_eq!(
            resolver.resolve(&first).expect_err("missing shards"),
            ResolveError::IncompleteShardSet
        );
    }

    #[test]
    fn rejects_relative_and_outside_paths() {
        let root = tempfile::tempdir().expect("model root");
        let outside = tempfile::NamedTempFile::new().expect("outside model");
        let resolver = LocalModelResolver::new([root.path().to_path_buf()]).expect("resolver");

        assert_eq!(
            resolver
                .resolve(Path::new("relative-model"))
                .expect_err("relative path"),
            ResolveError::InputNotAbsolute
        );
        assert_eq!(
            resolver.resolve(outside.path()).expect_err("outside path"),
            ResolveError::OutsideAllowedRoots
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_a_model_root() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("model root");
        let outside = tempfile::NamedTempFile::new().expect("outside model");
        let link = root.path().join("escaped-model");
        symlink(outside.path(), &link).expect("model symlink");
        let resolver = LocalModelResolver::new([root.path().to_path_buf()]).expect("resolver");

        assert_eq!(
            resolver.resolve(&link).expect_err("escaped symlink"),
            ResolveError::OutsideAllowedRoots
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_nested_symlinks_that_escape_a_model_directory() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("model root");
        let model = root.path().join("model");
        fs::create_dir(&model).expect("model directory");
        let outside = tempfile::NamedTempFile::new().expect("outside model");
        symlink(outside.path(), model.join("escaped-weights")).expect("nested symlink");
        let resolver = LocalModelResolver::new([root.path().to_path_buf()]).expect("resolver");

        assert_eq!(
            resolver
                .resolve(&model)
                .expect_err("escaped nested symlink"),
            ResolveError::OutsideAllowedRoots
        );
    }
}
