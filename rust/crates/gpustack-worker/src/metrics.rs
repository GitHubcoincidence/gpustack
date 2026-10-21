//! Prometheus exposition for the typed Worker resource snapshot.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, AtomicU64};

use gpustack_client::types::{GpuDeviceInfo, MountPointInfo, WorkerResourceStatus};
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use thiserror::Error;
use tokio::sync::watch;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::control::ControlPlaneHealth;
use crate::resources::CollectedResources;

type Labels = Vec<(String, String)>;
type FloatGauge = Gauge<f64, AtomicU64>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricIdentity {
    pub instance: String,
    pub worker_id: i64,
    pub worker_name: String,
}

#[derive(Debug, Error)]
pub enum MetricsError {
    #[error("failed to encode Prometheus metrics: {0}")]
    Encode(#[from] std::fmt::Error),
}

#[derive(Clone)]
pub struct MetricsState {
    pub identity: MetricIdentity,
    pub resources: watch::Receiver<CollectedResources>,
    pub control: watch::Receiver<ControlPlaneHealth>,
}

pub fn router(state: MetricsState) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}

async fn metrics_handler(State(state): State<MetricsState>) -> Response {
    let resources = state.resources.borrow().clone();
    let control = state.control.borrow().clone();
    match render_metrics(&state.identity, &resources.status, &control) {
        Ok(body) => ([(CONTENT_TYPE, "text/plain; version=0.0.4")], body).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "metrics encoding failed\n",
        )
            .into_response(),
    }
}

/// Builds one internally consistent scrape from immutable resource/control snapshots.
pub fn render_metrics(
    identity: &MetricIdentity,
    resources: &WorkerResourceStatus,
    control: &ControlPlaneHealth,
) -> Result<String, MetricsError> {
    let mut registry = Registry::default();
    let mut gauges = GaugeFamilies::default();
    register_resource_metrics(&mut gauges, identity, resources);
    register_control_metrics(&mut gauges, &mut registry, control);
    gauges.register(&mut registry);
    let mut output = String::new();
    encode(&mut output, &registry)?;
    Ok(output)
}

fn register_resource_metrics(
    registry: &mut GaugeFamilies,
    identity: &MetricIdentity,
    resources: &WorkerResourceStatus,
) {
    let common = common_labels(identity);
    if let Some(os) = &resources.os {
        set_gauge(
            registry,
            "gpustack:worker_node_os_info",
            "Worker operating system identity.",
            labels_with(
                &common,
                [("name", os.name.as_str()), ("version", os.version.as_str())],
            ),
            1.0,
        );
    }
    if let Some(kernel) = &resources.kernel {
        set_gauge(
            registry,
            "gpustack:worker_node_kernel_info",
            "Worker kernel identity.",
            labels_with(
                &common,
                [
                    ("name", kernel.name.as_str()),
                    ("release", kernel.release.as_str()),
                    ("version", kernel.version.as_str()),
                    ("architecture", kernel.architecture.as_str()),
                ],
            ),
            1.0,
        );
    }
    if let Some(uptime) = &resources.uptime {
        set_gauge(
            registry,
            "gpustack:worker_node_uptime_seconds",
            "Worker uptime in seconds.",
            common.clone(),
            uptime.uptime,
        );
    }
    if let Some(cpu) = &resources.cpu {
        set_optional_gauge(
            registry,
            "gpustack:worker_node_cpu_cores",
            "Worker logical CPU core count.",
            common.clone(),
            cpu.total.map(|value| value as f64),
        );
        set_optional_gauge(
            registry,
            "gpustack:worker_node_cpu_utilization_rate",
            "Worker CPU utilization percentage.",
            common.clone(),
            cpu.utilization_rate,
        );
    }
    if let Some(memory) = &resources.memory {
        set_optional_gauge(
            registry,
            "gpustack:worker_node_memory_total_bytes",
            "Worker memory capacity in bytes.",
            common.clone(),
            memory.total.map(|value| value as f64),
        );
        set_optional_gauge(
            registry,
            "gpustack:worker_node_memory_used_bytes",
            "Worker used memory in bytes.",
            common.clone(),
            memory.used.map(|value| value as f64),
        );
        set_optional_gauge(
            registry,
            "gpustack:worker_node_memory_utilization_rate",
            "Worker memory utilization percentage.",
            common.clone(),
            memory.utilization_rate,
        );
    }
    for gpu in resources.gpu_devices.as_deref().unwrap_or_default() {
        register_gpu_metrics(registry, &common, gpu);
    }
    for filesystem in resources.filesystem.as_deref().unwrap_or_default() {
        register_filesystem_metrics(registry, &common, filesystem);
    }
}

fn register_gpu_metrics(registry: &mut GaugeFamilies, common: &Labels, gpu: &GpuDeviceInfo) {
    let index = gpu.device_index.to_string();
    let chip_index = gpu.device_chip_index.to_string();
    let labels = labels_with(
        common,
        [
            ("gpu_index", index.as_str()),
            ("gpu_chip_index", chip_index.as_str()),
            ("gpu_name", gpu.name.as_str()),
        ],
    );
    set_gauge(
        registry,
        "worker_node_gpu_info",
        "Worker GPU identity.",
        labels.clone(),
        1.0,
    );
    if let Some(core) = &gpu.core {
        set_optional_gauge(
            registry,
            "gpustack:worker_node_gpu_cores",
            "Worker GPU core count.",
            labels.clone(),
            core.total.map(|value| value as f64),
        );
        set_optional_gauge(
            registry,
            "gpustack:worker_node_gpu_utilization_rate",
            "Worker GPU utilization percentage.",
            labels.clone(),
            core.utilization_rate,
        );
    }
    set_optional_gauge(
        registry,
        "gpustack:worker_node_gpu_temperature_celsius",
        "Worker GPU temperature in Celsius.",
        labels.clone(),
        gpu.temperature,
    );
    if let Some(memory) = &gpu.memory {
        set_optional_gauge(
            registry,
            "gpustack:worker_node_gram_total_bytes",
            "Worker GPU memory capacity in bytes.",
            labels.clone(),
            memory.total.map(|value| value as f64),
        );
        // Python exports the allocated metric family even though the Server,
        // rather than the Worker, owns the allocation value.
        registry.ensure(
            "gpustack:worker_node_gram_allocated_bytes",
            "Worker allocated GPU memory in bytes.",
        );
        set_optional_gauge(
            registry,
            "gpustack:worker_node_gram_allocated_bytes",
            "Worker allocated GPU memory in bytes.",
            labels.clone(),
            memory.allocated.map(|value| value as f64),
        );
        set_optional_gauge(
            registry,
            "gpustack:worker_node_gram_used_bytes",
            "Worker used GPU memory in bytes.",
            labels.clone(),
            memory.used.map(|value| value as f64),
        );
        set_optional_gauge(
            registry,
            "gpustack:worker_node_gram_utilization_rate",
            "Worker GPU memory utilization percentage.",
            labels,
            memory.utilization_rate,
        );
    }
}

fn register_filesystem_metrics(
    registry: &mut GaugeFamilies,
    common: &Labels,
    filesystem: &MountPointInfo,
) {
    let labels = labels_with(common, [("mountpoint", filesystem.mount_point.as_str())]);
    set_gauge(
        registry,
        "gpustack:worker_node_filesystem_total_bytes",
        "Worker filesystem capacity in bytes.",
        labels.clone(),
        filesystem.total as f64,
    );
    set_gauge(
        registry,
        "gpustack:worker_node_filesystem_used_bytes",
        "Worker used filesystem space in bytes.",
        labels.clone(),
        filesystem.used as f64,
    );
    let utilization = if filesystem.total == 0 {
        0.0
    } else {
        filesystem.used as f64 * 100.0 / filesystem.total as f64
    };
    set_gauge(
        registry,
        "gpustack:worker_node_filesystem_utilization_rate",
        "Worker filesystem utilization percentage.",
        labels,
        utilization,
    );
}

fn register_control_metrics(
    registry: &mut GaugeFamilies,
    prometheus_registry: &mut Registry,
    control: &ControlPlaneHealth,
) {
    set_gauge(
        registry,
        "gpustack:rust_worker_mode_info",
        "Rust Worker operating mode.",
        vec![("mode".to_owned(), "resource_api_only".to_owned())],
        1.0,
    );
    for (name, value) in [
        ("gpustack:rust_worker_ready", false),
        (
            "gpustack:rust_worker_server_connected",
            control.heartbeat_ok || control.status_ok || control.full_sync_ok,
        ),
        (
            "gpustack:rust_worker_heartbeat_success",
            control.heartbeat_ok,
        ),
        ("gpustack:rust_worker_status_success", control.status_ok),
    ] {
        let gauge = Gauge::<i64, AtomicI64>::default();
        gauge.set(if value { 1 } else { 0 });
        prometheus_registry.register(name, "Rust Worker control-loop state.", gauge);
    }
    for resource in ["models", "model_instances", "model_files"] {
        set_gauge(
            registry,
            "gpustack:rust_worker_watch_connected",
            "Rust Worker watch connection state.",
            vec![("resource".to_owned(), resource.to_owned())],
            if control.watch_connected { 1.0 } else { 0.0 },
        );
    }
    let reconnects = Family::<Labels, Counter>::default();
    for resource in ["models", "model_instances", "model_files"] {
        reconnects
            .get_or_create(&vec![("resource".to_owned(), resource.to_owned())])
            .inc_by(control.reconnect_attempts);
    }
    prometheus_registry.register(
        "gpustack:rust_worker_watch_reconnects",
        "Rust Worker watch reconnect attempts.",
        reconnects,
    );
}

fn common_labels(identity: &MetricIdentity) -> Labels {
    vec![
        ("instance".to_owned(), identity.instance.clone()),
        ("worker_id".to_owned(), identity.worker_id.to_string()),
        ("worker_name".to_owned(), identity.worker_name.clone()),
    ]
}

fn labels_with<'a, const N: usize>(common: &Labels, extra: [(&'a str, &'a str); N]) -> Labels {
    let mut labels = common.clone();
    labels.extend(
        extra
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned())),
    );
    labels
}

fn set_optional_gauge(
    registry: &mut GaugeFamilies,
    name: &'static str,
    help: &'static str,
    labels: Labels,
    value: Option<f64>,
) {
    if let Some(value) = value {
        set_gauge(registry, name, help, labels, value);
    }
}

fn set_gauge(
    registry: &mut GaugeFamilies,
    name: &'static str,
    help: &'static str,
    labels: Labels,
    value: f64,
) {
    registry.set(name, help, labels, value);
}

#[derive(Default)]
struct GaugeFamilies {
    families: BTreeMap<&'static str, (&'static str, Family<Labels, FloatGauge>)>,
}

impl GaugeFamilies {
    fn ensure(&mut self, name: &'static str, help: &'static str) {
        self.families
            .entry(name)
            .or_insert_with(|| (help, Family::default()));
    }

    fn set(&mut self, name: &'static str, help: &'static str, labels: Labels, value: f64) {
        let (_, family) = self
            .families
            .entry(name)
            .or_insert_with(|| (help, Family::default()));
        family.get_or_create(&labels).set(value);
    }

    fn register(self, registry: &mut Registry) {
        for (name, (help, family)) in self.families {
            registry.register(name, help, family);
        }
    }
}

#[cfg(test)]
mod tests {
    use gpustack_client::types::{
        CpuInfo, GpuCoreInfo, GpuDeviceInfo, KernelInfo, MemoryInfo, MountPointInfo,
        OperatingSystemInfo, UptimeInfo,
    };

    use super::*;

    fn resources() -> WorkerResourceStatus {
        WorkerResourceStatus {
            cpu: Some(CpuInfo {
                total: Some(96),
                utilization_rate: Some(12.5),
            }),
            filesystem: Some(vec![MountPointInfo {
                name: "home".to_owned(),
                mount_point: "/home".to_owned(),
                mount_from: "/dev/home".to_owned(),
                total: 100,
                used: 25,
                free: 75,
                available: 75,
            }]),
            gpu_devices: Some(vec![GpuDeviceInfo {
                vendor: Some("NVIDIA".to_owned()),
                device_type: Some("cuda".to_owned()),
                index: Some(0),
                device_index: 0,
                device_chip_index: 0,
                arch_family: Some("Ampere".to_owned()),
                name: "NVIDIA A100 80GB PCIe".to_owned(),
                uuid: Some("GPU-contract".to_owned()),
                driver_version: Some("580.178.04".to_owned()),
                runtime_version: Some("13.0".to_owned()),
                compute_capability: Some("8.0".to_owned()),
                core: Some(GpuCoreInfo {
                    total: Some(6912),
                    utilization_rate: Some(2.0),
                }),
                memory: Some(MemoryInfo {
                    allocated: None,
                    is_unified_memory: false,
                    total: Some(85_899_345_920),
                    used: Some(1_048_576),
                    utilization_rate: Some(0.001_220_703_125),
                }),
                temperature: Some(31.0),
                network: None,
            }]),
            kernel: Some(KernelInfo {
                architecture: "x86_64".to_owned(),
                name: "Linux".to_owned(),
                release: "contract".to_owned(),
                version: "contract".to_owned(),
            }),
            memory: Some(MemoryInfo {
                allocated: None,
                is_unified_memory: false,
                total: Some(1000),
                used: Some(200),
                utilization_rate: Some(20.0),
            }),
            os: Some(OperatingSystemInfo {
                name: "Ubuntu".to_owned(),
                version: "24.04".to_owned(),
            }),
            rpc_servers: None,
            swap: None,
            uptime: Some(UptimeInfo {
                boot_time: "2026-10-20T00:00:00Z".to_owned(),
                uptime: 10.0,
            }),
        }
    }

    #[test]
    fn scrape_contains_frozen_resource_and_control_metric_names() {
        let identity = MetricIdentity {
            instance: "10.213.80.90".to_owned(),
            worker_id: 42,
            worker_name: "rust-worker".to_owned(),
        };
        let control = ControlPlaneHealth {
            heartbeat_ok: true,
            status_ok: true,
            full_sync_ok: true,
            watch_connected: true,
            reconnect_attempts: 3,
        };

        let text = render_metrics(&identity, &resources(), &control).expect("render metrics");

        for name in [
            "gpustack:worker_node_os_info",
            "gpustack:worker_node_kernel_info",
            "gpustack:worker_node_uptime_seconds",
            "gpustack:worker_node_cpu_cores",
            "gpustack:worker_node_memory_total_bytes",
            "worker_node_gpu_info",
            "gpustack:worker_node_gpu_cores",
            "gpustack:worker_node_gram_total_bytes",
            "gpustack:worker_node_filesystem_total_bytes",
            "gpustack:rust_worker_mode_info",
            "gpustack:rust_worker_ready",
            "gpustack:rust_worker_watch_connected",
            "gpustack:rust_worker_watch_reconnects_total",
        ] {
            assert!(text.contains(name), "missing {name}\n{text}");
        }
        assert!(text.contains("instance=\"10.213.80.90\""));
        assert!(text.contains("worker_id=\"42\""));
        assert!(text.contains("gpu_name=\"NVIDIA A100 80GB PCIe\""));
        assert!(!text.contains("token"));
        assert!(text.contains("gpustack:rust_worker_ready 0"));
    }
}
