use std::ffi::CString;
use std::mem;
use std::ptr;

pub struct SysctlInfo {
    pub cpu_brand: String,
    pub cpu_cores: u32,
    pub cpu_cores_perf: u32,
    pub cpu_cores_eff: u32,
    pub physical_memory: u64,
    pub page_size: u64,
}

impl SysctlInfo {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            cpu_brand: get_sysctl_string("machdep.cpu.brand_string")
                .unwrap_or_else(|_| "Unknown".into()),
            cpu_cores: get_sysctl_u32("hw.ncpu").unwrap_or(0),
            cpu_cores_perf: get_sysctl_u32("hw.perflevel0.logicalcpu").unwrap_or(0),
            cpu_cores_eff: get_sysctl_u32("hw.perflevel1.logicalcpu").unwrap_or(0),
            physical_memory: get_sysctl_u64("hw.memsize").unwrap_or(0),
            page_size: get_sysctl_u64("hw.pagesize").unwrap_or(4096),
        })
    }
}

fn get_sysctl_string(name: &str) -> Result<String, String> {
    unsafe {
        let name_c = CString::new(name).map_err(|e| e.to_string())?;
        let mut size: libc::size_t = 0;

        // First call to get size
        let ret = libc::sysctlbyname(name_c.as_ptr(), ptr::null_mut(), &mut size, ptr::null_mut(), 0);
        if ret != 0 {
            return Err(format!("sysctlbyname failed for {}: {}", name, ret));
        }

        // Allocate buffer
        let mut buf: Vec<u8> = vec![0; size];

        // Second call to get value
        let ret = libc::sysctlbyname(
            name_c.as_ptr(),
            buf.as_mut_ptr() as *mut libc::c_void,
            &mut size,
            ptr::null_mut(),
            0,
        );
        if ret != 0 {
            return Err(format!("sysctlbyname failed for {}: {}", name, ret));
        }

        // Remove null terminator if present
        if let Some(&0) = buf.last() {
            buf.pop();
        }

        String::from_utf8(buf).map_err(|e| e.to_string())
    }
}

fn get_sysctl_u32(name: &str) -> Result<u32, String> {
    unsafe {
        let name_c = CString::new(name).map_err(|e| e.to_string())?;
        let mut value: u32 = 0;
        let mut size: libc::size_t = mem::size_of::<u32>();

        let ret = libc::sysctlbyname(
            name_c.as_ptr(),
            &mut value as *mut u32 as *mut libc::c_void,
            &mut size,
            ptr::null_mut(),
            0,
        );
        if ret != 0 {
            return Err(format!("sysctlbyname failed for {}: {}", name, ret));
        }

        Ok(value)
    }
}

fn get_sysctl_u64(name: &str) -> Result<u64, String> {
    unsafe {
        let name_c = CString::new(name).map_err(|e| e.to_string())?;
        let mut value: u64 = 0;
        let mut size: libc::size_t = mem::size_of::<u64>();

        let ret = libc::sysctlbyname(
            name_c.as_ptr(),
            &mut value as *mut u64 as *mut libc::c_void,
            &mut size,
            ptr::null_mut(),
            0,
        );
        if ret != 0 {
            return Err(format!("sysctlbyname failed for {}: {}", name, ret));
        }

        Ok(value)
    }
}
