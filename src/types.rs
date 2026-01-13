use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub active_bytes: u64,
    pub wired_bytes: u64,
    pub compressed_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub page_ins: u64,
    pub page_outs: u64,
    pub page_faults: u64,
    pub pressure: MemoryPressure,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MemoryPressure {
    #[default]
    Normal,
    Warn,
    Critical,
}

impl std::fmt::Display for MemoryPressure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryPressure::Normal => write!(f, "normal"),
            MemoryPressure::Warn => write!(f, "warn"),
            MemoryPressure::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CpuClusterMetrics {
    pub name: String,
    pub freq_mhz: u32,
    pub freq_max_mhz: u32,
    pub active_pct: f64,
    pub idle_pct: f64,
    pub power_watts: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GpuMetrics {
    pub freq_mhz: u32,
    pub freq_max_mhz: u32,
    pub active_pct: f64,
    pub power_watts: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AneMetrics {
    pub power_watts: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiskMetrics {
    pub name: String,
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
    pub read_ops_per_sec: u64,
    pub write_ops_per_sec: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SystemMetrics {
    pub total_power_watts: f64,
    pub cpu_power_watts: f64,
    pub gpu_power_watts: f64,
    pub ane_power_watts: f64,
    pub dram_power_watts: f64,
    pub thermal_pressure: ThermalPressure,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ThermalPressure {
    #[default]
    Nominal,
    Moderate,
    Heavy,
    Critical,
    Sleeping,
}

impl std::fmt::Display for ThermalPressure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThermalPressure::Nominal => write!(f, "nominal"),
            ThermalPressure::Moderate => write!(f, "moderate"),
            ThermalPressure::Heavy => write!(f, "heavy"),
            ThermalPressure::Critical => write!(f, "critical"),
            ThermalPressure::Sleeping => write!(f, "sleeping"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AllMetrics {
    pub timestamp_ms: u64,
    pub interval_ms: u64,
    pub memory: MemoryMetrics,
    pub cpu_clusters: Vec<CpuClusterMetrics>,
    pub gpu: GpuMetrics,
    pub ane: AneMetrics,
    pub disks: Vec<DiskMetrics>,
    pub system: SystemMetrics,
}
