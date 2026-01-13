#![allow(dead_code)]

use std::ffi::c_void;
use std::mem::MaybeUninit;

const KERNEL_INDEX_SMC: i32 = 2;

const SMC_CMD_READ_BYTES: u8 = 5;
const SMC_CMD_READ_KEYINFO: u8 = 9;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct SmcKeyData {
    key: u32,
    vers: [u8; 6],
    p_limit_data: [u8; 16],
    key_info: SmcKeyInfoData,
    result: u8,
    status: u8,
    data8: u8,
    data32: u32,
    bytes: [u8; 32],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct SmcKeyInfoData {
    data_size: u32,
    data_type: u32,
    data_attributes: u8,
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceGetMatchingService(
        master_port: u32,
        matching: *const c_void,
    ) -> u32;
    fn IOServiceMatching(name: *const i8) -> *const c_void;
    fn IOServiceOpen(
        service: u32,
        owning_task: u32,
        conn_type: u32,
        connection: *mut u32,
    ) -> i32;
    fn IOServiceClose(connection: u32) -> i32;
    fn IOConnectCallStructMethod(
        connection: u32,
        selector: u32,
        input_struct: *const c_void,
        input_struct_cnt: usize,
        output_struct: *mut c_void,
        output_struct_cnt: *mut usize,
    ) -> i32;
    fn mach_task_self() -> u32;
}

fn fourcc(s: &str) -> u32 {
    let bytes = s.as_bytes();
    if bytes.len() != 4 {
        return 0;
    }
    ((bytes[0] as u32) << 24)
        | ((bytes[1] as u32) << 16)
        | ((bytes[2] as u32) << 8)
        | (bytes[3] as u32)
}

#[allow(dead_code)]
fn fourcc_to_str(val: u32) -> String {
    let bytes = [
        ((val >> 24) & 0xFF) as u8,
        ((val >> 16) & 0xFF) as u8,
        ((val >> 8) & 0xFF) as u8,
        (val & 0xFF) as u8,
    ];
    String::from_utf8_lossy(&bytes).to_string()
}

pub struct Smc {
    connection: u32,
}

impl Smc {
    pub fn new() -> Result<Self, String> {
        unsafe {
            let service_name = b"AppleSMC\0".as_ptr() as *const i8;
            let matching = IOServiceMatching(service_name);
            if matching.is_null() {
                return Err("Failed to create matching dictionary".into());
            }

            let service = IOServiceGetMatchingService(0, matching);
            if service == 0 {
                return Err("Failed to find AppleSMC service".into());
            }

            let mut connection: u32 = 0;
            let result = IOServiceOpen(service, mach_task_self(), 0, &mut connection);
            if result != 0 {
                return Err(format!("Failed to open SMC connection: {}", result));
            }

            Ok(Self { connection })
        }
    }

    fn read_key_info(&self, key: u32) -> Option<SmcKeyInfoData> {
        unsafe {
            let mut input = SmcKeyData::default();
            input.key = key;
            input.data8 = SMC_CMD_READ_KEYINFO;

            let mut output = MaybeUninit::<SmcKeyData>::uninit();
            let mut output_size = std::mem::size_of::<SmcKeyData>();

            let result = IOConnectCallStructMethod(
                self.connection,
                KERNEL_INDEX_SMC as u32,
                &input as *const _ as *const c_void,
                std::mem::size_of::<SmcKeyData>(),
                output.as_mut_ptr() as *mut c_void,
                &mut output_size,
            );

            if result != 0 {
                return None;
            }

            let output = output.assume_init();
            Some(output.key_info)
        }
    }

    fn read_key(&self, key: u32) -> Option<[u8; 32]> {
        let key_info = self.read_key_info(key)?;

        unsafe {
            let mut input = SmcKeyData::default();
            input.key = key;
            input.key_info.data_size = key_info.data_size;
            input.data8 = SMC_CMD_READ_BYTES;

            let mut output = MaybeUninit::<SmcKeyData>::uninit();
            let mut output_size = std::mem::size_of::<SmcKeyData>();

            let result = IOConnectCallStructMethod(
                self.connection,
                KERNEL_INDEX_SMC as u32,
                &input as *const _ as *const c_void,
                std::mem::size_of::<SmcKeyData>(),
                output.as_mut_ptr() as *mut c_void,
                &mut output_size,
            );

            if result != 0 {
                return None;
            }

            let output = output.assume_init();
            Some(output.bytes)
        }
    }

    pub fn read_temp(&self, key: &str) -> Option<f64> {
        let key_code = fourcc(key);
        let bytes = self.read_key(key_code)?;

        // Temperature values are typically in fp78 (7.8 fixed point) or flt (float)
        // For most temp sensors, use signed 16-bit with 8 fractional bits
        let raw = ((bytes[0] as i16) << 8) | (bytes[1] as i16);
        Some(raw as f64 / 256.0)
    }

    pub fn read_power(&self, key: &str) -> Option<f64> {
        let key_code = fourcc(key);
        let bytes = self.read_key(key_code)?;

        // Power values are typically float (4 bytes, little-endian on Apple Silicon)
        if bytes[0..4] == [0, 0, 0, 0] {
            return None;
        }

        let raw = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Some(raw as f64)
    }

    pub fn read_fan_speed(&self, key: &str) -> Option<f64> {
        let key_code = fourcc(key);
        let bytes = self.read_key(key_code)?;

        // Fan speed in fpe2 format (14.2 fixed point)
        let raw = ((bytes[0] as u16) << 8) | (bytes[1] as u16);
        Some(raw as f64 / 4.0)
    }

    // Common temperature sensors
    pub fn cpu_temp(&self) -> Option<f64> {
        // Try common CPU temperature keys
        self.read_temp("Tc0c")
            .or_else(|| self.read_temp("TC0P"))
            .or_else(|| self.read_temp("TC0D"))
    }

    pub fn gpu_temp(&self) -> Option<f64> {
        self.read_temp("Tg0p")
            .or_else(|| self.read_temp("TG0P"))
            .or_else(|| self.read_temp("TG0D"))
    }
}

impl Drop for Smc {
    fn drop(&mut self) {
        unsafe {
            IOServiceClose(self.connection);
        }
    }
}

// Make Smc Send-safe (connection handle is just an integer)
unsafe impl Send for Smc {}
