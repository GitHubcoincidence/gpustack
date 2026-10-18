use std::fs::Metadata;
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
}

impl DiagnosticCheck {
    #[must_use]
    pub fn pass(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Pass,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn warn(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warn,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn fail(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Fail,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    Any,
    Directory,
    File,
    UnixSocket,
}

#[must_use]
pub fn inspect_path(
    name: &str,
    path: &Path,
    expected: PathKind,
    required: bool,
) -> DiagnosticCheck {
    match path.metadata() {
        Ok(metadata) if matches_kind(&metadata, expected) => {
            DiagnosticCheck::pass(name, format!("{} is available", path.display()))
        }
        Ok(_) => DiagnosticCheck::fail(
            name,
            format!("{} has an unexpected file type", path.display()),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !required => {
            DiagnosticCheck::warn(name, format!("{} does not exist yet", path.display()))
        }
        Err(error) => DiagnosticCheck::fail(
            name,
            format!("cannot inspect {}: {}", path.display(), error.kind()),
        ),
    }
}

fn matches_kind(metadata: &Metadata, expected: PathKind) -> bool {
    match expected {
        PathKind::Any => true,
        PathKind::Directory => metadata.is_dir(),
        PathKind::File => metadata.is_file(),
        PathKind::UnixSocket => is_unix_socket(metadata),
    }
}

#[cfg(unix)]
fn is_unix_socket(metadata: &Metadata) -> bool {
    use std::os::unix::fs::FileTypeExt;

    metadata.file_type().is_socket()
}

#[cfg(not(unix))]
fn is_unix_socket(_metadata: &Metadata) -> bool {
    false
}
