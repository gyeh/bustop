use crate::types::DiskMetrics;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::number::CFNumberRef;
use core_foundation::string::CFString;
use core_foundation_sys::base::CFRelease;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;

type IOIterator = u32;
type IOObject = u32;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceMatching(name: *const i8) -> *const c_void;
    fn IOServiceGetMatchingServices(
        master_port: u32,
        matching: *const c_void,
        iterator: *mut IOIterator,
    ) -> i32;
    fn IOIteratorNext(iterator: IOIterator) -> IOObject;
    fn IORegistryEntryGetName(entry: IOObject, name: *mut i8) -> i32;
    fn IORegistryEntryCreateCFProperties(
        entry: IOObject,
        properties: *mut CFDictionaryRef,
        allocator: *const c_void,
        options: u32,
    ) -> i32;
    fn IOObjectRelease(object: IOObject) -> i32;
}

extern "C" {
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: *const c_void) -> *const c_void;
    fn CFNumberGetValue(number: CFNumberRef, number_type: i32, value_ptr: *mut c_void) -> bool;
}

const K_CF_NUMBER_SINT64_TYPE: i32 = 4;

#[derive(Debug, Clone, Default)]
struct DiskSnapshot {
    read_bytes: u64,
    write_bytes: u64,
    read_ops: u64,
    write_ops: u64,
}

pub struct DiskStats {
    prev_snapshots: HashMap<String, DiskSnapshot>,
}

impl DiskStats {
    pub fn new() -> Self {
        Self {
            prev_snapshots: HashMap::new(),
        }
    }

    pub fn get_metrics(&mut self, interval_secs: f64) -> Vec<DiskMetrics> {
        let current = self.get_disk_snapshots();
        let mut metrics = Vec::new();

        for (name, current_snap) in &current {
            if let Some(prev_snap) = self.prev_snapshots.get(name) {
                let read_bytes_delta = current_snap.read_bytes.saturating_sub(prev_snap.read_bytes);
                let write_bytes_delta =
                    current_snap.write_bytes.saturating_sub(prev_snap.write_bytes);
                let read_ops_delta = current_snap.read_ops.saturating_sub(prev_snap.read_ops);
                let write_ops_delta = current_snap.write_ops.saturating_sub(prev_snap.write_ops);

                metrics.push(DiskMetrics {
                    name: name.clone(),
                    read_bytes_per_sec: (read_bytes_delta as f64 / interval_secs) as u64,
                    write_bytes_per_sec: (write_bytes_delta as f64 / interval_secs) as u64,
                    read_ops_per_sec: (read_ops_delta as f64 / interval_secs) as u64,
                    write_ops_per_sec: (write_ops_delta as f64 / interval_secs) as u64,
                });
            }
        }

        self.prev_snapshots = current;
        metrics
    }

    fn get_disk_snapshots(&self) -> HashMap<String, DiskSnapshot> {
        let mut snapshots = HashMap::new();

        unsafe {
            let class_name = b"IOBlockStorageDriver\0".as_ptr() as *const i8;
            let matching = IOServiceMatching(class_name);
            if matching.is_null() {
                return snapshots;
            }

            let mut iterator: IOIterator = 0;
            let result = IOServiceGetMatchingServices(0, matching, &mut iterator);
            if result != 0 {
                return snapshots;
            }

            loop {
                let service = IOIteratorNext(iterator);
                if service == 0 {
                    break;
                }

                let mut name_buf = [0i8; 128];
                if IORegistryEntryGetName(service, name_buf.as_mut_ptr()) == 0 {
                    let name = std::ffi::CStr::from_ptr(name_buf.as_ptr())
                        .to_string_lossy()
                        .to_string();

                    if let Some(snapshot) = self.get_driver_stats(service) {
                        // Use a simplified name
                        let disk_name = if name.contains("Media") {
                            "disk0".to_string()
                        } else {
                            format!("disk{}", snapshots.len())
                        };
                        snapshots.insert(disk_name, snapshot);
                    }
                }

                IOObjectRelease(service);
            }

            IOObjectRelease(iterator);
        }

        snapshots
    }

    fn get_driver_stats(&self, service: IOObject) -> Option<DiskSnapshot> {
        unsafe {
            let mut props_ref: CFDictionaryRef = ptr::null();
            let result = IORegistryEntryCreateCFProperties(
                service,
                &mut props_ref,
                ptr::null(),
                0,
            );

            if result != 0 || props_ref.is_null() {
                return None;
            }

            // Look for Statistics dictionary
            let stats_key = CFString::new("Statistics");
            let stats_dict = CFDictionaryGetValue(
                props_ref,
                stats_key.as_concrete_TypeRef() as *const c_void,
            ) as CFDictionaryRef;

            if stats_dict.is_null() {
                CFRelease(props_ref as *const c_void);
                return None;
            }

            let read_bytes = Self::get_number(stats_dict, "Bytes (Read)").unwrap_or(0);
            let write_bytes = Self::get_number(stats_dict, "Bytes (Write)").unwrap_or(0);
            let read_ops = Self::get_number(stats_dict, "Operations (Read)").unwrap_or(0);
            let write_ops = Self::get_number(stats_dict, "Operations (Write)").unwrap_or(0);

            CFRelease(props_ref as *const c_void);

            Some(DiskSnapshot {
                read_bytes,
                write_bytes,
                read_ops,
                write_ops,
            })
        }
    }

    fn get_number(dict: CFDictionaryRef, key: &str) -> Option<u64> {
        unsafe {
            let key_cf = CFString::new(key);
            let num = CFDictionaryGetValue(dict, key_cf.as_concrete_TypeRef() as *const c_void)
                as CFNumberRef;

            if num.is_null() {
                return None;
            }

            let mut value: i64 = 0;
            if CFNumberGetValue(num, K_CF_NUMBER_SINT64_TYPE, &mut value as *mut _ as *mut c_void) {
                Some(value as u64)
            } else {
                None
            }
        }
    }
}
