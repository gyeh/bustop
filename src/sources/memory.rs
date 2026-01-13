use crate::types::{MemoryMetrics, MemoryPressure};
use std::ffi::CString;
use std::mem::{self, MaybeUninit};
use std::ptr;

// vm_statistics64 uses natural_t which is u32 on ARM64
#[repr(C)]
#[derive(Debug, Default)]
struct VmStatistics64 {
    free_count: u32,
    active_count: u32,
    inactive_count: u32,
    wire_count: u32,
    zero_fill_count: u64,
    reactivations: u64,
    pageins: u64,
    pageouts: u64,
    faults: u64,
    cow_faults: u64,
    lookups: u64,
    hits: u64,
    purges: u64,
    purgeable_count: u32,
    speculative_count: u32,
    decompressions: u64,
    compressions: u64,
    swapins: u64,
    swapouts: u64,
    compressor_page_count: u32,
    throttled_count: u32,
    external_page_count: u32,
    internal_page_count: u32,
    total_uncompressed_pages_in_compressor: u64,
}

#[repr(C)]
struct XswUsage {
    xsu_total: u64,
    xsu_avail: u64,
    xsu_used: u64,
    xsu_pagesize: u32,
    xsu_encrypted: bool,
}

extern "C" {
    fn mach_host_self() -> u32;
    fn host_statistics64(
        host: u32,
        flavor: i32,
        host_info: *mut VmStatistics64,
        count: *mut u32,
    ) -> i32;
}

const HOST_VM_INFO64: i32 = 4;
const HOST_VM_INFO64_COUNT: u32 =
    (std::mem::size_of::<VmStatistics64>() / std::mem::size_of::<i32>()) as u32;

pub struct MemoryStats {
    page_size: u64,
    total_memory: u64,
    prev_pageins: u64,
    prev_pageouts: u64,
    prev_faults: u64,
}

impl MemoryStats {
    pub fn new(page_size: u64, total_memory: u64) -> Self {
        Self {
            page_size,
            total_memory,
            prev_pageins: 0,
            prev_pageouts: 0,
            prev_faults: 0,
        }
    }

    pub fn get_metrics(&mut self) -> MemoryMetrics {
        let stats = self.get_vm_stats();
        let swap = self.get_swap_usage();

        let free = (stats.free_count as u64) * self.page_size;
        let active = (stats.active_count as u64) * self.page_size;
        let inactive = (stats.inactive_count as u64) * self.page_size;
        let wired = (stats.wire_count as u64) * self.page_size;
        let compressed = (stats.compressor_page_count as u64) * self.page_size;
        let speculative = (stats.speculative_count as u64) * self.page_size;

        // Calculate used memory (similar to Activity Monitor)
        let used = active + wired + compressed + inactive - speculative;

        // Calculate deltas for page activity
        let page_ins_delta = if stats.pageins >= self.prev_pageins {
            stats.pageins - self.prev_pageins
        } else {
            stats.pageins
        };
        let page_outs_delta = if stats.pageouts >= self.prev_pageouts {
            stats.pageouts - self.prev_pageouts
        } else {
            stats.pageouts
        };
        let page_faults_delta = if stats.faults >= self.prev_faults {
            stats.faults - self.prev_faults
        } else {
            stats.faults
        };

        self.prev_pageins = stats.pageins;
        self.prev_pageouts = stats.pageouts;
        self.prev_faults = stats.faults;

        // Determine memory pressure
        let pressure = self.get_memory_pressure();

        MemoryMetrics {
            total_bytes: self.total_memory,
            used_bytes: used,
            free_bytes: free,
            active_bytes: active,
            wired_bytes: wired,
            compressed_bytes: compressed,
            swap_used_bytes: swap.0,
            swap_total_bytes: swap.1,
            page_ins: page_ins_delta,
            page_outs: page_outs_delta,
            page_faults: page_faults_delta,
            pressure,
        }
    }

    fn get_vm_stats(&self) -> VmStatistics64 {
        unsafe {
            let mut stats = MaybeUninit::<VmStatistics64>::uninit();
            let mut count = HOST_VM_INFO64_COUNT;

            let result = host_statistics64(
                mach_host_self(),
                HOST_VM_INFO64,
                stats.as_mut_ptr(),
                &mut count,
            );

            if result != 0 {
                return VmStatistics64::default();
            }

            stats.assume_init()
        }
    }

    fn get_swap_usage(&self) -> (u64, u64) {
        unsafe {
            let name = CString::new("vm.swapusage").unwrap();
            let mut swap = MaybeUninit::<XswUsage>::uninit();
            let mut size = mem::size_of::<XswUsage>();

            let result = libc::sysctlbyname(
                name.as_ptr(),
                swap.as_mut_ptr() as *mut libc::c_void,
                &mut size,
                ptr::null_mut(),
                0,
            );

            if result != 0 {
                return (0, 0);
            }

            let swap = swap.assume_init();
            (swap.xsu_used, swap.xsu_total)
        }
    }

    fn get_memory_pressure(&self) -> MemoryPressure {
        // Try to read memory pressure level from sysctl
        unsafe {
            let name = CString::new("kern.memorystatus_vm_pressure_level").unwrap();
            let mut level: i32 = 0;
            let mut size = mem::size_of::<i32>();

            let result = libc::sysctlbyname(
                name.as_ptr(),
                &mut level as *mut i32 as *mut libc::c_void,
                &mut size,
                ptr::null_mut(),
                0,
            );

            if result == 0 {
                match level {
                    1 => MemoryPressure::Normal,
                    2 => MemoryPressure::Warn,
                    4 => MemoryPressure::Critical,
                    _ => MemoryPressure::Normal,
                }
            } else {
                MemoryPressure::Normal
            }
        }
    }
}
