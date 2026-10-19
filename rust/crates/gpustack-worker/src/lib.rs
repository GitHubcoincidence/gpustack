//! Worker configuration, diagnostics, and lifecycle primitives.

pub mod config;
pub mod doctor;
pub mod signal;

pub use config::{ConfigError, ConfigOverrides, LoadedConfig, WorkerConfig, load_config};
pub use doctor::{DoctorReport, run_doctor};
