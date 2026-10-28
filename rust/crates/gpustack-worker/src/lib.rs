//! Worker configuration, identity, control-plane, diagnostics, and lifecycle primitives.

pub mod config;
pub mod control;
pub mod doctor;
pub mod health;
pub mod identity;
pub mod lab;
pub mod managed;
pub mod metrics;
pub mod resource_run;
pub mod resources;
pub mod run;
pub mod runtime_core;
pub mod signal;
pub mod worker_api;
pub mod workload;

pub use config::{ConfigError, ConfigOverrides, LoadedConfig, WorkerConfig, load_config};
pub use doctor::{DoctorReport, run_doctor};
pub use identity::{
    CredentialStore, IdentityBootstrap, IdentityError, IdentityRecordStore, IdentitySource,
    WorkerIdentity, WorkerIdentityRecord, worker_credential_path, worker_identity_path,
};
pub use resource_run::{
    MANAGED_FILE_ONLY_MODE, MANAGED_RUNTIME_MODE, ManagedFileEvent, ManagedFileSummary,
    ManagedRuntimeEvent, ManagedRuntimeSummary, RESOURCE_API_ONLY_MODE, ResourceApiEvent,
    ResourceApiRunError, ResourceApiSummary, run_managed_file_only, run_managed_runtime,
    run_resource_api_only,
};
pub use run::{
    CONTROL_PLANE_ONLY_MODE, ControlPlaneOnlyEvent, ControlPlaneOnlySummary, WorkerRunError,
    run_control_plane_only,
};
