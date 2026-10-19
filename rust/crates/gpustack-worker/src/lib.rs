//! Worker configuration, diagnostics, and lifecycle primitives.

/// Configuration loading, precedence, and validation.
pub mod config;
/// Read-only deployment prerequisite diagnostics.
pub mod doctor;
/// Cross-platform graceful shutdown signal handling.
pub mod signal;

pub use config::{ConfigError, ConfigOverrides, LoadedConfig, WorkerConfig, load_config};
pub use doctor::{DoctorReport, run_doctor};
