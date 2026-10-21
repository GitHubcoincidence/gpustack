//! Typed host and NVIDIA GPU resource collection.

use std::fs;
use std::path::Path;
use std::thread;

use gpustack_client::types::{
    CpuInfo, GpuCoreInfo, GpuDeviceInfo, KernelInfo, MemoryInfo, MountPointInfo,
    OperatingSystemInfo, SwapInfo, UptimeInfo, WorkerResourceStatus,
};
use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use serde::Serialize;
use sysinfo::{Disks, System};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectedResources {
    pub machine_id: Option<String>,
    pub status: WorkerResourceStatus,
}

pub trait SystemProbe: Send {
    fn collect_system(&mut self) -> Result<SystemResources, ResourceError>;
}

pub trait GpuProbe: Send + Sync {
    fn collect_gpus(&self) -> Result<Vec<GpuDeviceInfo>, ResourceError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemResources {
    pub machine_id: Option<String>,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub swap: SwapInfo,
    pub filesystem: Vec<MountPointInfo>,
    pub os: OperatingSystemInfo,
    pub kernel: KernelInfo,
    pub uptime: UptimeInfo,
}

pub struct ResourceCollector<S, G> {
    system: S,
    gpu: G,
}

impl<S, G> ResourceCollector<S, G>
where
    S: SystemProbe,
    G: GpuProbe,
{
    pub fn new(system: S, gpu: G) -> Self {
        Self { system, gpu }
    }

    pub fn collect(&mut self) -> Result<CollectedResources, ResourceError> {
        let system = self.system.collect_system()?;
        let gpu_devices = self.gpu.collect_gpus()?;
        if gpu_devices.is_empty() {
            return Err(ResourceError::NoGpu);
        }
        Ok(CollectedResources {
            machine_id: system.machine_id,
            status: WorkerResourceStatus {
                cpu: Some(system.cpu),
                filesystem: Some(system.filesystem),
                gpu_devices: Some(gpu_devices),
                kernel: Some(system.kernel),
                memory: Some(system.memory),
                os: Some(system.os),
                rpc_servers: None,
                swap: Some(system.swap),
                uptime: Some(system.uptime),
            },
        })
    }
}

pub struct SysinfoProbe {
    system: System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FilesystemSpace {
    total: u64,
    used: u64,
    free: u64,
    available: u64,
}

impl SysinfoProbe {
    pub fn new() -> Self {
        let mut system = System::new_all();
        // CPU usage is a delta. The second refresh must happen after sysinfo's
        // documented minimum interval or the first published sample is stale.
        thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_usage();
        Self { system }
    }
}

impl Default for SysinfoProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemProbe for SysinfoProbe {
    fn collect_system(&mut self) -> Result<SystemResources, ResourceError> {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        let total_memory = self.system.total_memory();
        let used_memory = self.system.used_memory();
        let total_swap = self.system.total_swap();
        let used_swap = self.system.used_swap();
        let disks = Disks::new_with_refreshed_list();
        let filesystem = disks
            .list()
            .iter()
            .map(|disk| {
                let space = filesystem_space(
                    disk.mount_point(),
                    disk.total_space(),
                    disk.available_space(),
                );
                MountPointInfo {
                    name: disk.name().to_string_lossy().into_owned(),
                    mount_point: disk.mount_point().to_string_lossy().into_owned(),
                    mount_from: disk.name().to_string_lossy().into_owned(),
                    total: space.total,
                    used: space.used,
                    free: space.free,
                    available: space.available,
                }
            })
            .collect();
        let uptime = System::uptime();
        let boot_time = System::boot_time();
        Ok(SystemResources {
            machine_id: read_trimmed(Path::new("/etc/machine-id")),
            cpu: CpuInfo {
                total: Some(self.system.cpus().len() as u64),
                utilization_rate: Some(f64::from(self.system.global_cpu_usage())),
            },
            memory: MemoryInfo {
                allocated: None,
                is_unified_memory: false,
                total: Some(total_memory),
                used: Some(used_memory),
                utilization_rate: percentage(used_memory, total_memory),
            },
            swap: SwapInfo {
                total: Some(total_swap),
                used: Some(used_swap),
                utilization_rate: percentage(used_swap, total_swap),
            },
            filesystem,
            os: OperatingSystemInfo {
                name: System::name().unwrap_or_else(|| "Unknown".to_owned()),
                version: System::long_os_version().unwrap_or_else(|| "Unknown".to_owned()),
            },
            kernel: KernelInfo {
                architecture: System::cpu_arch(),
                name: "Linux".to_owned(),
                release: System::kernel_version().unwrap_or_else(|| "Unknown".to_owned()),
                version: read_trimmed(Path::new("/proc/version"))
                    .unwrap_or_else(|| "Unknown".to_owned()),
            },
            uptime: UptimeInfo {
                boot_time: unix_seconds_to_rfc3339(boot_time),
                uptime: uptime as f64,
            },
        })
    }
}

#[cfg(unix)]
fn filesystem_space(mount_point: &Path, total: u64, available: u64) -> FilesystemSpace {
    rustix::fs::statvfs(mount_point)
        .ok()
        .and_then(|stats| {
            let block_size = if stats.f_frsize == 0 {
                stats.f_bsize
            } else {
                stats.f_frsize
            };
            (block_size != 0).then(|| {
                filesystem_space_from_blocks(
                    stats.f_blocks,
                    stats.f_bfree,
                    stats.f_bavail,
                    block_size,
                )
            })
        })
        .unwrap_or_else(|| filesystem_space_fallback(total, available))
}

#[cfg(not(unix))]
fn filesystem_space(_mount_point: &Path, total: u64, available: u64) -> FilesystemSpace {
    filesystem_space_fallback(total, available)
}

fn filesystem_space_from_blocks(
    blocks: u64,
    blocks_free: u64,
    blocks_available: u64,
    block_size: u64,
) -> FilesystemSpace {
    // Fastfetch distinguishes all free blocks from blocks available to an
    // unprivileged process, and derives used space from the former.
    let total = blocks.saturating_mul(block_size);
    let free = blocks_free.saturating_mul(block_size);
    let available = blocks_available.saturating_mul(block_size);
    FilesystemSpace {
        total,
        used: total.saturating_sub(free),
        free,
        available,
    }
}

fn filesystem_space_fallback(total: u64, available: u64) -> FilesystemSpace {
    FilesystemSpace {
        total,
        used: total.saturating_sub(available),
        free: available,
        available,
    }
}

pub struct NvmlGpuProbe {
    nvml: Nvml,
    driver_version: String,
    runtime_version: String,
}

impl NvmlGpuProbe {
    pub fn new() -> Result<Self, ResourceError> {
        // A single NVML handle is retained because repeatedly loading and
        // shutting down the driver library races concurrent API/metrics reads.
        let nvml = Nvml::init().map_err(|error| ResourceError::Nvml(error.to_string()))?;
        let driver_version = nvml
            .sys_driver_version()
            .map_err(|error| ResourceError::Nvml(error.to_string()))?;
        let runtime_version = format_cuda_version(
            nvml.sys_cuda_driver_version()
                .map_err(|error| ResourceError::Nvml(error.to_string()))?,
        );
        Ok(Self {
            nvml,
            driver_version,
            runtime_version,
        })
    }
}

impl GpuProbe for NvmlGpuProbe {
    fn collect_gpus(&self) -> Result<Vec<GpuDeviceInfo>, ResourceError> {
        let count = self
            .nvml
            .device_count()
            .map_err(|error| ResourceError::Nvml(error.to_string()))?;
        let mut devices = Vec::with_capacity(count as usize);
        for index in 0..count {
            let device =
                self.nvml
                    .device_by_index(index)
                    .map_err(|error| ResourceError::GpuField {
                        index,
                        field: "device",
                        message: error.to_string(),
                    })?;
            let name = required_gpu_field(index, "name", device.name())?;
            let uuid = required_gpu_field(index, "uuid", device.uuid())?;
            let memory = required_gpu_field(index, "memory", device.memory_info())?;
            let capability = required_gpu_field(
                index,
                "compute_capability",
                device.cuda_compute_capability(),
            )?;
            let cores = required_gpu_field(index, "cores", device.num_cores())?;
            let utilization = device.utilization_rates().ok();
            let temperature = device.temperature(TemperatureSensor::Gpu).ok();
            devices.push(GpuDeviceInfo {
                vendor: Some("NVIDIA".to_owned()),
                device_type: Some("cuda".to_owned()),
                index: Some(index),
                device_index: index,
                device_chip_index: 0,
                arch_family: infer_nvidia_arch(&name).map(str::to_owned),
                name,
                uuid: Some(uuid),
                driver_version: Some(self.driver_version.clone()),
                runtime_version: Some(self.runtime_version.clone()),
                compute_capability: Some(format!("{}.{}", capability.major, capability.minor)),
                core: Some(GpuCoreInfo {
                    total: Some(u64::from(cores)),
                    utilization_rate: utilization.as_ref().map(|value| f64::from(value.gpu)),
                }),
                memory: Some(MemoryInfo {
                    allocated: None,
                    is_unified_memory: false,
                    total: Some(memory.total),
                    used: Some(memory.used),
                    utilization_rate: percentage(memory.used, memory.total),
                }),
                temperature: temperature.map(f64::from),
                network: None,
            });
        }
        Ok(devices)
    }
}

/// Collects the current NVIDIA inventory for the explicit diagnostic CLI command.
pub fn collect_gpu_inventory() -> Result<Vec<GpuDeviceInfo>, ResourceError> {
    NvmlGpuProbe::new()?.collect_gpus()
}

fn required_gpu_field<T>(
    index: u32,
    field: &'static str,
    result: Result<T, nvml_wrapper::error::NvmlError>,
) -> Result<T, ResourceError> {
    result.map_err(|error| ResourceError::GpuField {
        index,
        field,
        message: error.to_string(),
    })
}

fn percentage(used: u64, total: u64) -> Option<f64> {
    (total != 0).then(|| used as f64 * 100.0 / total as f64)
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn format_cuda_version(value: i32) -> String {
    let major = value / 1000;
    let minor = value % 1000 / 10;
    format!("{major}.{minor}")
}

fn infer_nvidia_arch(name: &str) -> Option<&'static str> {
    if name.contains("A100") || name.contains("A30") || name.contains("A10") {
        Some("Ampere")
    } else {
        None
    }
}

pub(crate) fn unix_seconds_to_rfc3339(seconds: u64) -> String {
    let days = seconds / 86_400;
    let seconds_of_day = seconds % 86_400;
    // Civil date conversion avoids another time dependency in the collector.
    // The algorithm operates on days since the Unix epoch and is valid for the
    // positive timestamps returned by Linux boot_time.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let hour = seconds_of_day / 3_600;
    let minute = seconds_of_day % 3_600 / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("failed to initialize or query NVML: {0}")]
    Nvml(String),
    #[error("GPU {index} is missing required field {field}: {message}")]
    GpuField {
        index: u32,
        field: &'static str,
        message: String,
    },
    #[error("NVML returned no GPU devices")]
    NoGpu,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSystem;

    impl SystemProbe for FakeSystem {
        fn collect_system(&mut self) -> Result<SystemResources, ResourceError> {
            Ok(SystemResources {
                machine_id: Some("contract-machine-id".to_owned()),
                cpu: CpuInfo {
                    total: Some(96),
                    utilization_rate: Some(0.2),
                },
                memory: MemoryInfo {
                    allocated: None,
                    is_unified_memory: false,
                    total: Some(809_601_908_736),
                    used: Some(22_264_052_408),
                    utilization_rate: Some(2.75),
                },
                swap: SwapInfo {
                    total: Some(8_589_934_592),
                    used: Some(0),
                    utilization_rate: Some(0.0),
                },
                filesystem: vec![MountPointInfo {
                    name: "contract-home-volume".to_owned(),
                    mount_point: "/home".to_owned(),
                    mount_from: "/dev/mapper/contract-home".to_owned(),
                    total: 15_393_162_788_864,
                    used: 8_573_157_376_000,
                    free: 6_819_997_067_264,
                    available: 6_819_997_067_264,
                }],
                os: OperatingSystemInfo {
                    name: "Ubuntu".to_owned(),
                    version: "24.04.4 LTS".to_owned(),
                },
                kernel: KernelInfo {
                    architecture: "x86_64".to_owned(),
                    name: "Linux".to_owned(),
                    release: "6.8.0-147-generic".to_owned(),
                    version: "contract".to_owned(),
                },
                uptime: UptimeInfo {
                    boot_time: "2026-10-20T04:20:00Z".to_owned(),
                    uptime: 86_400.0,
                },
            })
        }
    }

    struct FakeGpu(Vec<GpuDeviceInfo>);

    impl GpuProbe for FakeGpu {
        fn collect_gpus(&self) -> Result<Vec<GpuDeviceInfo>, ResourceError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn fake_collectors_match_the_frozen_status_fixture() {
        let fixture: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/server-v2.2.3/worker-status-resource.request.json"
        ))
        .expect("resource status fixture");
        let gpus: Vec<GpuDeviceInfo> =
            serde_json::from_value(fixture["status"]["gpu_devices"].clone())
                .expect("typed GPU fixture");
        let expected: WorkerResourceStatus =
            serde_json::from_value(fixture["status"].clone()).expect("typed resource fixture");
        let mut collector = ResourceCollector::new(FakeSystem, FakeGpu(gpus));

        let collected = collector.collect().expect("collect fake resources");

        assert_eq!(collected.machine_id.as_deref(), Some("contract-machine-id"));
        assert_eq!(collected.status, expected);
    }

    #[test]
    fn missing_gpu_fails_closed() {
        let mut collector = ResourceCollector::new(FakeSystem, FakeGpu(Vec::new()));

        assert!(matches!(collector.collect(), Err(ResourceError::NoGpu)));
    }

    #[test]
    fn timestamp_and_cuda_formatting_are_stable() {
        assert_eq!(unix_seconds_to_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_cuda_version(13_000), "13.0");
        assert_eq!(infer_nvidia_arch("NVIDIA A100 80GB PCIe"), Some("Ampere"));
    }

    #[test]
    fn filesystem_space_distinguishes_free_from_available() {
        let space = filesystem_space_from_blocks(100, 40, 25, 4096);

        assert_eq!(space.total, 409_600);
        assert_eq!(space.used, 245_760);
        assert_eq!(space.free, 163_840);
        assert_eq!(space.available, 102_400);
    }

    #[test]
    fn filesystem_space_fallback_preserves_sysinfo_semantics() {
        let space = filesystem_space_fallback(100, 25);

        assert_eq!(space.total, 100);
        assert_eq!(space.used, 75);
        assert_eq!(space.free, 25);
        assert_eq!(space.available, 25);
    }
}
