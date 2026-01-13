# bustop

A command-line tool for monitoring bus and interconnect utilization on macOS, with an iostat-like presentation.

bustop displays real-time metrics for memory, CPU fabric, GPU, storage, and system power consumption on Apple Silicon Macs.

## Features

- **Memory Bus**: Used/free/wired memory, memory pressure, swap usage, page faults per second
- **CPU Fabric**: Per-cluster (E-Cluster/P-Cluster) activity and idle percentages
- **GPU Fabric**: GPU utilization metrics
- **Storage**: Read/write throughput (MB/s) and IOPS for each disk
- **System**: Total power consumption and thermal pressure status
- **Multiple output formats**: Human-readable tables, JSON for scripting, or compact append mode
- **No sudo required**: Uses Apple's private IOReport APIs for sudoless operation

## Installation

### From Source

Requires Rust 1.70 or later.

```bash
git clone https://github.com/yourusername/bustop.git
cd bustop
cargo build --release
```

The binary will be at `./target/release/bustop`.

## Usage

```bash
# Basic usage - updates every second
bustop

# Custom interval (milliseconds)
bustop -i 500

# Collect a specific number of samples
bustop -n 10

# JSON output for scripting/parsing
bustop -j

# Append mode (doesn't clear screen between updates)
bustop -a

# Combine options
bustop -i 2000 -n 5 -j
```

### Options

| Option | Long | Description | Default |
|--------|------|-------------|---------|
| `-i` | `--interval` | Sample interval in milliseconds | 1000 |
| `-n` | `--count` | Number of samples (0 = infinite) | 0 |
| `-j` | `--json` | Output in JSON format | false |
| `-a` | `--append` | Append mode (no screen clearing) | false |
| `-h` | `--help` | Print help | |
| `-V` | `--version` | Print version | |

## Output

### Default Format

```
bustop - Bus/Interconnect Monitor                    Interval: 1000ms

MEMORY
     used_GB      free_GB     wired_GB   pressure      swap_GB       faults/s
       14.63         0.44         3.40     normal         0.00           3754

CPU FABRIC
cluster        freq_MHz    active%      idle%    power_W
E-Cluster             -       12.3       87.7       0.00
P-Cluster             -       45.2       54.8       0.00

GPU FABRIC
device         freq_MHz    active%    power_W
gpu0                  -       23.1       0.00

STORAGE
device          read_MB/s   write_MB/s    r_ops/s    w_ops/s
disk0                2.91         1.15        118         47

SYSTEM
     total_W        cpu_W        gpu_W       dram_W          thermal
        0.00         0.00         0.00         0.00          nominal
```

### JSON Format

```json
{
  "timestamp_ms": 1768316675814,
  "interval_ms": 1005,
  "memory": {
    "total_bytes": 17179869184,
    "used_bytes": 16101048320,
    "free_bytes": 204226560,
    "pressure": "normal"
  },
  "cpu_clusters": [
    {"name": "E-Cluster", "active_pct": 12.3, "idle_pct": 87.7},
    {"name": "P-Cluster", "active_pct": 45.2, "idle_pct": 54.8}
  ],
  "disks": [
    {"name": "disk0", "read_bytes_per_sec": 1846847, "write_bytes_per_sec": 170104}
  ],
  "system": {
    "thermal_pressure": "nominal"
  }
}
```

### Append Mode

```
mem: 14.6GB used, 0.4GB free | E-Cluster: 12.3% | P-Cluster: 45.2% | gpu: 23.1% | disk0: 2.9/1.2 MB/s
mem: 14.5GB used, 0.5GB free | E-Cluster: 8.1% | P-Cluster: 32.4% | gpu: 18.7% | disk0: 1.1/0.8 MB/s
```

## How It Works

bustop uses several macOS-specific APIs to gather metrics:

| Subsystem | API | Details |
|-----------|-----|---------|
| Memory | `host_statistics64` | Mach kernel VM statistics |
| Memory Pressure | `kern.memorystatus_vm_pressure_level` | sysctl |
| CPU/GPU Stats | IOReport | Private Apple framework |
| Disk I/O | IOKit | `IOBlockStorageDriver` statistics |
| Thermal | `kern.thermalpressure` | sysctl |
| Hardware Info | sysctl | `hw.memsize`, `hw.pagesize`, etc. |

### Why No Direct Bandwidth Numbers?

macOS 13+ deprecated direct memory bandwidth monitoring in `powermetrics`. bustop provides:

- Memory pressure indicators (normal/warn/critical)
- Page fault rates as a proxy for memory activity
- Power consumption metrics (when available) which correlate with bandwidth

For detailed bandwidth analysis, consider using Instruments.app with the appropriate PMC counters.

## Requirements

- macOS 12.0 or later
- Apple Silicon (M1/M2/M3/M4) recommended
- Intel Macs supported with reduced functionality

## Building

```bash
# Debug build
cargo build

# Release build (optimized, stripped)
cargo build --release

# Run tests
cargo test
```

## License

MIT License

## Acknowledgments

- [macmon](https://github.com/vladkens/macmon) for IOReport API research
- [ibireme's kperf gist](https://gist.github.com/ibireme/173517c208c7dc333ba962c1f0d67d12) for PMU documentation
- [Brendan Gregg's USE Method](https://www.brendangregg.com/USEmethod/use-macosx.html) for methodology
