use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::array::CFArrayRef;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::number::CFNumberRef;
use core_foundation_sys::base::CFRelease;
use std::ffi::c_void;
use std::ptr;

type IOReportSubscriptionRef = *mut c_void;

#[link(name = "IOReport", kind = "dylib")]
extern "C" {
    fn IOReportCopyChannelsInGroup(
        group: CFStringRef,
        subgroup: CFStringRef,
        a: u64,
        b: u64,
        c: u64,
    ) -> CFDictionaryRef;
    fn IOReportMergeChannels(a: CFDictionaryRef, b: CFDictionaryRef, nil: CFTypeRef);
    fn IOReportCreateSubscription(
        a: *const c_void,
        channels: CFDictionaryRef,
        b: *mut CFDictionaryRef,
        c: u64,
        d: CFTypeRef,
    ) -> IOReportSubscriptionRef;
    fn IOReportCreateSamples(
        sub: IOReportSubscriptionRef,
        a: CFDictionaryRef,
        b: CFTypeRef,
    ) -> CFDictionaryRef;
    fn IOReportCreateSamplesDelta(
        a: CFDictionaryRef,
        b: CFDictionaryRef,
        c: CFTypeRef,
    ) -> CFDictionaryRef;
}

// CFDictionary helper functions
extern "C" {
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: *const c_void) -> *const c_void;
    fn CFArrayGetCount(array: CFArrayRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, idx: isize) -> *const c_void;
    fn CFStringGetCStringPtr(string: CFStringRef, encoding: u32) -> *const i8;
    fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut i8,
        buffer_size: isize,
        encoding: u32,
    ) -> bool;
    fn CFNumberGetValue(number: CFNumberRef, number_type: i32, value_ptr: *mut c_void) -> bool;
}

const IOREPORT_PATH: &str = "/System/Library/PrivateFrameworks/IOReport.framework/IOReport";
const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;
const K_CF_NUMBER_SINT64_TYPE: i32 = 4;

#[derive(Debug, Clone)]
pub struct IOReportSample {
    pub group: String,
    pub subgroup: String,
    pub channel: String,
    pub value: i64,
    pub unit: String,
}

pub struct IOReport {
    subscription: IOReportSubscriptionRef,
    channels: CFDictionaryRef,
    prev_sample: CFDictionaryRef,
}

unsafe impl Send for IOReport {}

impl IOReport {
    pub fn new(channel_groups: &[(&str, Option<&str>)]) -> Result<Self, String> {
        unsafe {
            // Load the IOReport framework dynamically
            let path = std::ffi::CString::new(IOREPORT_PATH).unwrap();
            let handle = libc::dlopen(path.as_ptr(), libc::RTLD_NOW);
            if handle.is_null() {
                return Err("Failed to load IOReport framework".into());
            }

            let mut merged_channels: CFDictionaryRef = ptr::null();

            for (group, subgroup) in channel_groups {
                let group_cf = CFString::new(group);
                let subgroup_cf = subgroup.map(|s| CFString::new(s));

                let channels_dict = IOReportCopyChannelsInGroup(
                    group_cf.as_concrete_TypeRef(),
                    subgroup_cf
                        .as_ref()
                        .map(|s| s.as_concrete_TypeRef())
                        .unwrap_or(ptr::null()),
                    0,
                    0,
                    0,
                );

                if channels_dict.is_null() {
                    continue;
                }

                if merged_channels.is_null() {
                    merged_channels = channels_dict;
                } else {
                    IOReportMergeChannels(merged_channels, channels_dict, ptr::null());
                    CFRelease(channels_dict as *const c_void);
                }
            }

            if merged_channels.is_null() {
                return Err("No valid channels found".into());
            }

            let mut sub_dict: CFDictionaryRef = ptr::null();
            let subscription = IOReportCreateSubscription(
                ptr::null(),
                merged_channels,
                &mut sub_dict,
                0,
                ptr::null(),
            );

            if subscription.is_null() {
                CFRelease(merged_channels as *const c_void);
                return Err("Failed to create IOReport subscription".into());
            }

            Ok(Self {
                subscription,
                channels: merged_channels,
                prev_sample: ptr::null(),
            })
        }
    }

    pub fn get_sample(&mut self) -> Vec<IOReportSample> {
        unsafe {
            let sample_dict = IOReportCreateSamples(
                self.subscription,
                self.channels,
                ptr::null(),
            );

            if sample_dict.is_null() {
                return vec![];
            }

            let result = if !self.prev_sample.is_null() {
                let delta_dict = IOReportCreateSamplesDelta(
                    self.prev_sample,
                    sample_dict,
                    ptr::null(),
                );

                if delta_dict.is_null() {
                    vec![]
                } else {
                    let samples = Self::parse_samples(delta_dict);
                    CFRelease(delta_dict as *const c_void);
                    samples
                }
            } else {
                vec![]
            };

            // Release previous sample and store new one
            if !self.prev_sample.is_null() {
                CFRelease(self.prev_sample as *const c_void);
            }
            self.prev_sample = sample_dict;

            result
        }
    }

    fn parse_samples(dict: CFDictionaryRef) -> Vec<IOReportSample> {
        let mut samples = Vec::new();

        unsafe {
            let channels_key = CFString::new("IOReportChannels");
            let channels_array = CFDictionaryGetValue(
                dict,
                channels_key.as_concrete_TypeRef() as *const c_void,
            ) as CFArrayRef;

            if channels_array.is_null() {
                return samples;
            }

            let count = CFArrayGetCount(channels_array);
            for i in 0..count {
                let channel = CFArrayGetValueAtIndex(channels_array, i) as CFDictionaryRef;
                if !channel.is_null() {
                    if let Some(sample) = Self::parse_channel(channel) {
                        samples.push(sample);
                    }
                }
            }
        }

        samples
    }

    fn parse_channel(dict: CFDictionaryRef) -> Option<IOReportSample> {
        let group = Self::get_string(dict, "IOReportGroupName").unwrap_or_default();
        let subgroup = Self::get_string(dict, "IOReportSubGroupName").unwrap_or_default();
        let channel_name = Self::get_string(dict, "IOReportChannelName").unwrap_or_default();
        let unit = Self::get_string(dict, "IOReportChannelUnit").unwrap_or_default();
        let value = Self::get_value(dict);

        Some(IOReportSample {
            group,
            subgroup,
            channel: channel_name,
            value,
            unit,
        })
    }

    fn get_string(dict: CFDictionaryRef, key: &str) -> Option<String> {
        unsafe {
            let key_cf = CFString::new(key);
            let value = CFDictionaryGetValue(dict, key_cf.as_concrete_TypeRef() as *const c_void)
                as CFStringRef;

            if value.is_null() {
                return None;
            }

            // Try fast path first
            let cstr = CFStringGetCStringPtr(value, K_CF_STRING_ENCODING_UTF8);
            if !cstr.is_null() {
                return Some(
                    std::ffi::CStr::from_ptr(cstr)
                        .to_string_lossy()
                        .to_string(),
                );
            }

            // Fallback: copy to buffer
            let mut buffer = [0i8; 256];
            if CFStringGetCString(
                value,
                buffer.as_mut_ptr(),
                buffer.len() as isize,
                K_CF_STRING_ENCODING_UTF8,
            ) {
                Some(
                    std::ffi::CStr::from_ptr(buffer.as_ptr())
                        .to_string_lossy()
                        .to_string(),
                )
            } else {
                None
            }
        }
    }

    fn get_value(dict: CFDictionaryRef) -> i64 {
        unsafe {
            // First try simple value
            let key = CFString::new("IOReportSimpleValue");
            let num = CFDictionaryGetValue(dict, key.as_concrete_TypeRef() as *const c_void)
                as CFNumberRef;

            if !num.is_null() {
                let mut value: i64 = 0;
                if CFNumberGetValue(num, K_CF_NUMBER_SINT64_TYPE, &mut value as *mut _ as *mut c_void)
                {
                    return value;
                }
            }

            0
        }
    }
}

impl Drop for IOReport {
    fn drop(&mut self) {
        unsafe {
            if !self.channels.is_null() {
                CFRelease(self.channels as *const c_void);
            }
            if !self.prev_sample.is_null() {
                CFRelease(self.prev_sample as *const c_void);
            }
        }
    }
}
