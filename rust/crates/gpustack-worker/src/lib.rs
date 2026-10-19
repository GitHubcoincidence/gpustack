//! Worker configuration, identity, control-plane, diagnostics, and lifecycle primitives.

pub mod config;
pub mod control;
pub mod doctor;
pub mod identity;
pub mod signal;

pub use config::{ConfigError, ConfigOverrides, LoadedConfig, WorkerConfig, load_config};
pub use doctor::{DoctorReport, run_doctor};
pub use identity::{
    CredentialStore, IdentityBootstrap, IdentityError, IdentitySource, WorkerIdentity,
    worker_credential_path,
};
