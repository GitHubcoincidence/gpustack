//! Worker configuration, identity, control-plane, diagnostics, and lifecycle primitives.

mod api_documentation;
pub mod config;
pub mod control;
pub mod doctor;
pub mod health;
pub mod identity;
pub mod lab;
pub mod managed;
pub mod metrics;
pub mod readiness;
pub mod resources;
pub mod run;
pub mod runtime_core;
pub mod signal;
mod supervisor;
pub mod worker_api;
pub mod workload;

pub use config::{ConfigError, ConfigOverrides, LoadedConfig, WorkerConfig, load_config};
pub use doctor::{DoctorReport, run_doctor};
pub use identity::{
    CredentialStore, IdentityBootstrap, IdentityError, IdentityEstablishment, IdentityRecordStore,
    IdentitySource, WorkerIdentity, WorkerIdentityRecord, worker_credential_path,
    worker_identity_path,
};
pub use run::{WorkerEvent, WorkerRunError, WorkerSummary, run_worker};
