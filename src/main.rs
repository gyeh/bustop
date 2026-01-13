mod display;
mod metrics;
mod sources;
mod types;

use clap::Parser;
use metrics::MetricsCollector;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "bustop")]
#[command(author = "bustop")]
#[command(version = "0.1.0")]
#[command(about = "Bus and interconnect utilization monitor for macOS", long_about = None)]
struct Args {
    /// Sample interval in milliseconds
    #[arg(short = 'i', long = "interval", default_value_t = 1000)]
    interval: u64,

    /// Number of samples to collect (0 = infinite)
    #[arg(short = 'n', long = "count", default_value_t = 0)]
    count: u64,

    /// Output in JSON format (one object per line)
    #[arg(short = 'j', long = "json")]
    json: bool,

    /// Don't clear screen between updates (append mode)
    #[arg(short = 'a', long = "append")]
    append: bool,
}

fn main() {
    let args = Args::parse();

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc_handler(r);

    // Initialize metrics collector
    let interval = Duration::from_millis(args.interval);
    let mut collector = match MetricsCollector::new(interval) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to initialize metrics collector: {}", e);
            std::process::exit(1);
        }
    };

    // Initial header for non-JSON mode
    if !args.json && !args.append {
        display::print_header(collector.cpu_brand(), args.interval);
    }

    let mut sample_count: u64 = 0;
    let mut first = true;

    while running.load(Ordering::SeqCst) {
        // Collect metrics
        let metrics = collector.collect();

        // Output
        if args.json {
            if !first {
                display::print_json(&metrics);
            }
        } else if args.append {
            if !first {
                print_append_mode(&metrics);
            }
        } else {
            display::print_metrics(&metrics, first);
        }

        first = false;
        sample_count += 1;

        // Check if we've collected enough samples
        if args.count > 0 && sample_count >= args.count {
            break;
        }

        // Flush stdout
        io::stdout().flush().ok();

        // Sleep until next sample
        std::thread::sleep(interval);
    }
}

fn ctrlc_handler(running: Arc<AtomicBool>) {
    let _ = ctrlc::set_handler(move || {
        running.store(false, Ordering::SeqCst);
    });
}

fn print_append_mode(metrics: &types::AllMetrics) {
    let mem = &metrics.memory;
    let sys = &metrics.system;

    // Compact single-line output for append mode
    print!(
        "mem: {:.1}GB used, {:.1}GB free | ",
        mem.used_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        mem.free_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    for cluster in &metrics.cpu_clusters {
        print!("{}: {:.1}% | ", cluster.name, cluster.active_pct);
    }

    print!("gpu: {:.1}% | ", metrics.gpu.active_pct);

    if sys.total_power_watts > 0.0 {
        print!("power: {:.1}W | ", sys.total_power_watts);
    }

    for disk in &metrics.disks {
        print!(
            "{}: {:.1}/{:.1} MB/s",
            disk.name,
            disk.read_bytes_per_sec as f64 / (1024.0 * 1024.0),
            disk.write_bytes_per_sec as f64 / (1024.0 * 1024.0)
        );
    }

    println!();
}
