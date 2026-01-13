use crate::sources::{DiskStats, IOReport, MemoryStats, Smc, SysctlInfo};
use crate::types::*;
use std::time::{Duration, Instant};

pub struct MetricsCollector {
    ioreport: Option<IOReport>,
    smc: Option<Smc>,
    memory_stats: MemoryStats,
    disk_stats: DiskStats,
    sysctl_info: SysctlInfo,
    last_sample: Instant,
    interval: Duration,
}

impl MetricsCollector {
    pub fn new(interval: Duration) -> Result<Self, String> {
        let sysctl_info = SysctlInfo::new()?;

        // Initialize IOReport with relevant channel groups
        let ioreport = IOReport::new(&[
            ("Energy Model", None),
            ("CPU Stats", Some("CPU Complex Performance States")),
            ("CPU Stats", Some("CPU Core Performance States")),
            ("GPU Stats", Some("GPU Performance States")),
        ])
        .ok();

        let smc = Smc::new().ok();

        let memory_stats = MemoryStats::new(sysctl_info.page_size, sysctl_info.physical_memory);
        let disk_stats = DiskStats::new();

        Ok(Self {
            ioreport,
            smc,
            memory_stats,
            disk_stats,
            sysctl_info,
            last_sample: Instant::now(),
            interval,
        })
    }

    pub fn collect(&mut self) -> AllMetrics {
        let now = Instant::now();
        let actual_interval = now.duration_since(self.last_sample);
        let interval_secs = actual_interval.as_secs_f64();
        self.last_sample = now;

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // Collect from each source
        let memory = self.memory_stats.get_metrics();
        let disks = self.disk_stats.get_metrics(interval_secs);

        // Parse IOReport samples
        let (cpu_clusters, gpu, ane, system) = self.collect_ioreport_metrics();

        AllMetrics {
            timestamp_ms,
            interval_ms: actual_interval.as_millis() as u64,
            memory,
            cpu_clusters,
            gpu,
            ane,
            disks,
            system,
        }
    }

    fn collect_ioreport_metrics(
        &mut self,
    ) -> (Vec<CpuClusterMetrics>, GpuMetrics, AneMetrics, SystemMetrics) {
        let mut cpu_clusters = Vec::new();
        let mut gpu = GpuMetrics::default();
        let mut ane = AneMetrics::default();
        let mut system = SystemMetrics::default();

        // Get samples from IOReport
        if let Some(ref mut ioreport) = self.ioreport {
            let samples = ioreport.get_sample();

            // Aggregate by cluster/component
            let mut ecpu_residency = 0i64;
            let mut ecpu_total = 0i64;
            let mut pcpu_residency = 0i64;
            let mut pcpu_total = 0i64;

            for sample in &samples {
                match sample.group.as_str() {
                    "CPU Stats" => {
                        if sample.channel.contains("ECPU") || sample.channel.contains("E-Cluster")
                        {
                            if sample.subgroup.contains("Performance States") {
                                ecpu_total += sample.value.max(0);
                                if !sample.channel.contains("IDLE") {
                                    ecpu_residency += sample.value.max(0);
                                }
                            }
                        } else if sample.channel.contains("PCPU")
                            || sample.channel.contains("P-Cluster")
                        {
                            if sample.subgroup.contains("Performance States") {
                                pcpu_total += sample.value.max(0);
                                if !sample.channel.contains("IDLE") {
                                    pcpu_residency += sample.value.max(0);
                                }
                            }
                        }
                    }
                    "Energy Model" => {
                        let power_mw = sample.value as f64 / 1000.0; // Typically in uW or nW
                        let power_w = power_mw / 1000.0;

                        if sample.channel.contains("CPU") {
                            system.cpu_power_watts += power_w;
                        } else if sample.channel.contains("GPU") {
                            system.gpu_power_watts += power_w;
                            gpu.power_watts = power_w;
                        } else if sample.channel.contains("ANE") {
                            system.ane_power_watts += power_w;
                            ane.power_watts = power_w;
                        } else if sample.channel.contains("DRAM") {
                            system.dram_power_watts += power_w;
                        }
                    }
                    "GPU Stats" => {
                        // GPU frequency/utilization
                        if sample.subgroup.contains("Performance States") {
                            // GPU utilization from residency
                        }
                    }
                    _ => {}
                }
            }

            // Calculate CPU cluster metrics
            if ecpu_total > 0 {
                let ecpu_active = (ecpu_residency as f64 / ecpu_total as f64 * 100.0).min(100.0);
                cpu_clusters.push(CpuClusterMetrics {
                    name: "E-Cluster".to_string(),
                    freq_mhz: 0, // Would need DVFS data
                    freq_max_mhz: 0,
                    active_pct: ecpu_active,
                    idle_pct: 100.0 - ecpu_active,
                    power_watts: 0.0, // Part of system.cpu_power_watts
                });
            }

            if pcpu_total > 0 {
                let pcpu_active = (pcpu_residency as f64 / pcpu_total as f64 * 100.0).min(100.0);
                cpu_clusters.push(CpuClusterMetrics {
                    name: "P-Cluster".to_string(),
                    freq_mhz: 0,
                    freq_max_mhz: 0,
                    active_pct: pcpu_active,
                    idle_pct: 100.0 - pcpu_active,
                    power_watts: 0.0,
                });
            }

            system.total_power_watts = system.cpu_power_watts
                + system.gpu_power_watts
                + system.ane_power_watts
                + system.dram_power_watts;
        }

        // Fallback: create default clusters based on sysctl info
        if cpu_clusters.is_empty() {
            if self.sysctl_info.cpu_cores_eff > 0 {
                cpu_clusters.push(CpuClusterMetrics {
                    name: "E-Cluster".to_string(),
                    freq_mhz: 0,
                    freq_max_mhz: 0,
                    active_pct: 0.0,
                    idle_pct: 100.0,
                    power_watts: 0.0,
                });
            }
            if self.sysctl_info.cpu_cores_perf > 0 {
                cpu_clusters.push(CpuClusterMetrics {
                    name: "P-Cluster".to_string(),
                    freq_mhz: 0,
                    freq_max_mhz: 0,
                    active_pct: 0.0,
                    idle_pct: 100.0,
                    power_watts: 0.0,
                });
            }
        }

        // Try to get thermal pressure
        system.thermal_pressure = self.get_thermal_pressure();

        (cpu_clusters, gpu, ane, system)
    }

    fn get_thermal_pressure(&self) -> ThermalPressure {
        // Try to read from sysctl
        unsafe {
            let name = std::ffi::CString::new("kern.thermalpressure").unwrap();
            let mut level: i32 = 0;
            let mut size = std::mem::size_of::<i32>();

            let result = libc::sysctlbyname(
                name.as_ptr(),
                &mut level as *mut i32 as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            );

            if result == 0 {
                match level {
                    0 => ThermalPressure::Nominal,
                    1 => ThermalPressure::Moderate,
                    2 => ThermalPressure::Heavy,
                    3 => ThermalPressure::Critical,
                    4 => ThermalPressure::Sleeping,
                    _ => ThermalPressure::Nominal,
                }
            } else {
                ThermalPressure::Nominal
            }
        }
    }

    pub fn cpu_brand(&self) -> &str {
        &self.sysctl_info.cpu_brand
    }
}
