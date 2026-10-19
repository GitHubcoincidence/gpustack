//! Read-only host runtime diagnostics shared by Worker commands.

use std::fs::Metadata;
use std::path::Path;

use serde::Serialize;

/// Severity and outcome of one diagnostic check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// The prerequisite is available and has the expected shape.
    Pass,
    /// The prerequisite is optional or can be created later.
    Warn,
    /// The prerequisite is required but unavailable or invalid.
    Fail,
}

/// One named, human-readable runtime diagnostic result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticCheck {
    /// Stable check name used by table and JSON output.
    pub name: String,
    /// Machine-readable result severity.
    pub status: CheckStatus,
    /// Human-readable explanation that does not expose secret values.
    pub message: String,
}

impl DiagnosticCheck {
    /// Constructs a successful diagnostic check.
    #[must_use]
    pub fn pass(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Pass,
            message: message.into(),
        }
    }

    /// Constructs a warning for a deferred or optional prerequisite.
    #[must_use]
    pub fn warn(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warn,
            message: message.into(),
        }
    }

    /// Constructs a failed diagnostic check.
    #[must_use]
    pub fn fail(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Fail,
            message: message.into(),
        }
    }
}

/// Expected filesystem object type for a path diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// Any filesystem object type is acceptable.
    Any,
    /// The path must be a directory.
    Directory,
    /// The path must be a regular file.
    File,
    /// The path must be a Unix domain socket.
    UnixSocket,
}

/// Inspects a path without opening or mutating the referenced object.
///
/// Missing optional paths produce a warning so commands can distinguish
/// deferred setup from an unusable required prerequisite.
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
