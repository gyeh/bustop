use crate::types::AllMetrics;

const BYTES_PER_MB: f64 = 1024.0 * 1024.0;
const BYTES_PER_GB: f64 = 1024.0 * 1024.0 * 1024.0;

pub fn print_header(cpu_brand: &str, interval_ms: u64) {
    println!(
        "bustop - Bus/Interconnect Monitor                    Interval: {}ms",
        interval_ms
    );
    println!("CPU: {}", cpu_brand);
    println!();
}

pub fn print_metrics(metrics: &AllMetrics, first: bool) {
    if first {
        // Need at least one interval to compute rates
        println!("Collecting initial sample...");
        return;
    }

    // Clear screen for refresh (optional: can use ANSI codes)
    print!("\x1B[2J\x1B[H"); // Clear screen and move cursor to top

    println!(
        "bustop - Bus/Interconnect Monitor                    Interval: {}ms",
        metrics.interval_ms
    );
    println!();

    // Memory section
    print_memory_section(metrics);
    println!();

    // CPU Fabric section
    print_cpu_section(metrics);
    println!();

    // GPU section
    print_gpu_section(metrics);
    println!();

    // Storage section
    print_storage_section(metrics);
    println!();

    // System section
    print_system_section(metrics);
}

fn print_memory_section(metrics: &AllMetrics) {
    let mem = &metrics.memory;

    println!("MEMORY");
    println!(
        "{:>12} {:>12} {:>12} {:>10} {:>12} {:>14}",
        "used_GB", "free_GB", "wired_GB", "pressure", "swap_GB", "faults/s"
    );

    let used_gb = mem.used_bytes as f64 / BYTES_PER_GB;
    let free_gb = mem.free_bytes as f64 / BYTES_PER_GB;
    let wired_gb = mem.wired_bytes as f64 / BYTES_PER_GB;
    let swap_gb = mem.swap_used_bytes as f64 / BYTES_PER_GB;
    let interval_secs = metrics.interval_ms as f64 / 1000.0;
    let faults_per_sec = mem.page_faults as f64 / interval_secs;

    println!(
        "{:>12.2} {:>12.2} {:>12.2} {:>10} {:>12.2} {:>14.0}",
        used_gb, free_gb, wired_gb, mem.pressure, swap_gb, faults_per_sec
    );
}

fn print_cpu_section(metrics: &AllMetrics) {
    if metrics.cpu_clusters.is_empty() {
        println!("CPU FABRIC");
        println!("  (no data available)");
        return;
    }

    println!("CPU FABRIC");
    println!(
        "{:<12} {:>10} {:>10} {:>10} {:>10}",
        "cluster", "freq_MHz", "active%", "idle%", "power_W"
    );

    for cluster in &metrics.cpu_clusters {
        println!(
            "{:<12} {:>10} {:>10.1} {:>10.1} {:>10.2}",
            cluster.name,
            if cluster.freq_mhz > 0 {
                cluster.freq_mhz.to_string()
            } else {
                "-".to_string()
            },
            cluster.active_pct,
            cluster.idle_pct,
            cluster.power_watts
        );
    }
}

fn print_gpu_section(metrics: &AllMetrics) {
    let gpu = &metrics.gpu;

    println!("GPU FABRIC");
    println!(
        "{:<12} {:>10} {:>10} {:>10}",
        "device", "freq_MHz", "active%", "power_W"
    );

    println!(
        "{:<12} {:>10} {:>10.1} {:>10.2}",
        "gpu0",
        if gpu.freq_mhz > 0 {
            gpu.freq_mhz.to_string()
        } else {
            "-".to_string()
        },
        gpu.active_pct,
        gpu.power_watts
    );

    if metrics.ane.power_watts > 0.0 {
        println!(
            "{:<12} {:>10} {:>10} {:>10.2}",
            "ane", "-", "-", metrics.ane.power_watts
        );
    }
}

fn print_storage_section(metrics: &AllMetrics) {
    if metrics.disks.is_empty() {
        println!("STORAGE");
        println!("  (no data available)");
        return;
    }

    println!("STORAGE");
    println!(
        "{:<12} {:>12} {:>12} {:>10} {:>10}",
        "device", "read_MB/s", "write_MB/s", "r_ops/s", "w_ops/s"
    );

    for disk in &metrics.disks {
        let read_mb = disk.read_bytes_per_sec as f64 / BYTES_PER_MB;
        let write_mb = disk.write_bytes_per_sec as f64 / BYTES_PER_MB;

        println!(
            "{:<12} {:>12.2} {:>12.2} {:>10} {:>10}",
            disk.name, read_mb, write_mb, disk.read_ops_per_sec, disk.write_ops_per_sec
        );
    }
}

fn print_system_section(metrics: &AllMetrics) {
    let sys = &metrics.system;

    println!("SYSTEM");
    println!(
        "{:>12} {:>12} {:>12} {:>12} {:>16}",
        "total_W", "cpu_W", "gpu_W", "dram_W", "thermal"
    );

    println!(
        "{:>12.2} {:>12.2} {:>12.2} {:>12.2} {:>16}",
        sys.total_power_watts,
        sys.cpu_power_watts,
        sys.gpu_power_watts,
        sys.dram_power_watts,
        sys.thermal_pressure
    );
}

pub fn print_json(metrics: &AllMetrics) {
    match serde_json::to_string(metrics) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error serializing to JSON: {}", e),
    }
}
