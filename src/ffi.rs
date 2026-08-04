#![allow(non_camel_case_types, dead_code)]

use std::ffi::CStr;
use std::os::raw::{c_char, c_double, c_int};

#[repr(C)]
struct CpuInfo {
    vendor: *mut c_char,
    model_name: *mut c_char,
    physical_cores: u64,
    logical_cores: u64,
    max_frequency_hz: i64,
}

#[repr(C)]
struct CpuInfoArray {
    count: c_int,
    cpus: *mut CpuInfo,
}

#[repr(C)]
struct DoubleArray {
    values: *mut c_double,
    count: c_int,
}

#[repr(C)]
struct Int64Array {
    values: *mut i64,
    count: c_int,
}

#[repr(C)]
struct GpuInfo {
    vendor: *mut c_char,
    name: *mut c_char,
    vendor_id: *mut c_char,
    device_id: *mut c_char,
    driver_version: *mut c_char,
    dedicated_memory_bytes: u64,
    shared_memory_bytes: u64,
    frequency_hz: u64,
    num_cores: u64,
}

#[repr(C)]
struct GpuInfoArray {
    count: c_int,
    gpus: *mut GpuInfo,
}

#[repr(C)]
struct RamInfo {
    total_bytes: u64,
    free_bytes: u64,
    available_bytes: u64,
}

#[repr(C)]
struct DiskInfo {
    vendor: *mut c_char,
    model: *mut c_char,
    serial_number: *mut c_char,
    size_bytes: u64,
    mount_point: *mut c_char,
    interface_type: c_int,
}

#[repr(C)]
struct DiskInfoArray {
    count: c_int,
    disks: *mut DiskInfo,
}

extern "C" {
    fn hwinfo_get_cpus() -> CpuInfoArray;
    fn hwinfo_free_cpu_info(arr: CpuInfoArray);

    fn hwinfo_cpu_utilization(sleep_ms: c_int) -> c_double;
    fn hwinfo_cpu_thread_utilization(sleep_ms: c_int) -> DoubleArray;
    fn hwinfo_cpu_thread_frequencies() -> Int64Array;
    fn hwinfo_free_double_array(arr: DoubleArray);
    fn hwinfo_free_int64_array(arr: Int64Array);

    fn hwinfo_get_gpus() -> GpuInfoArray;
    fn hwinfo_free_gpu_info(arr: GpuInfoArray);

    fn hwinfo_get_ram() -> RamInfo;

    fn hwinfo_get_disks() -> DiskInfoArray;
    fn hwinfo_free_disk_info(arr: DiskInfoArray);
}

unsafe fn ptr_to_string(p: *mut c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

pub struct Cpu {
    pub vendor: String,
    pub model_name: String,
    pub physical_cores: u64,
    pub logical_cores: u64,
    pub max_frequency_hz: i64,
}

pub fn get_cpus() -> Vec<Cpu> {
    unsafe {
        let arr = hwinfo_get_cpus();
        if arr.cpus.is_null() || arr.count <= 0 {
            return vec![];
        }
        let slice = std::slice::from_raw_parts(arr.cpus, arr.count as usize);
        let result: Vec<Cpu> = slice
            .iter()
            .map(|c| Cpu {
                vendor: ptr_to_string(c.vendor),
                model_name: ptr_to_string(c.model_name),
                physical_cores: c.physical_cores,
                logical_cores: c.logical_cores,
                max_frequency_hz: c.max_frequency_hz,
            })
            .collect();
        hwinfo_free_cpu_info(arr);
        result
    }
}

pub fn get_cpu_utilization(sleep_ms: i32) -> f64 {
    unsafe { hwinfo_cpu_utilization(sleep_ms as c_int) }
}

pub fn get_cpu_thread_frequencies() -> Vec<i64> {
    unsafe {
        let arr = hwinfo_cpu_thread_frequencies();
        if arr.values.is_null() || arr.count <= 0 {
            return vec![];
        }
        let slice = std::slice::from_raw_parts(arr.values, arr.count as usize);
        let result = slice.to_vec();
        hwinfo_free_int64_array(arr);
        result
    }
}

pub struct Gpu {
    pub vendor: String,
    pub name: String,
    pub vendor_id: String,
    pub device_id: String,
    pub driver_version: String,
    pub dedicated_memory_bytes: u64,
    pub shared_memory_bytes: u64,
    pub frequency_hz: u64,
    pub num_cores: u64,
}

pub fn get_gpus() -> Vec<Gpu> {
    unsafe {
        let arr = hwinfo_get_gpus();
        if arr.gpus.is_null() || arr.count <= 0 {
            return vec![];
        }
        let slice = std::slice::from_raw_parts(arr.gpus, arr.count as usize);
        let result: Vec<Gpu> = slice
            .iter()
            .map(|g| Gpu {
                vendor: ptr_to_string(g.vendor),
                name: ptr_to_string(g.name),
                vendor_id: ptr_to_string(g.vendor_id),
                device_id: ptr_to_string(g.device_id),
                driver_version: ptr_to_string(g.driver_version),
                dedicated_memory_bytes: g.dedicated_memory_bytes,
                shared_memory_bytes: g.shared_memory_bytes,
                frequency_hz: g.frequency_hz,
                num_cores: g.num_cores,
            })
            .collect();
        hwinfo_free_gpu_info(arr);
        result
    }
}

pub struct Ram {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
}

pub fn get_ram() -> Ram {
    unsafe {
        let r = hwinfo_get_ram();
        Ram {
            total_bytes: r.total_bytes,
            free_bytes: r.free_bytes,
            available_bytes: r.available_bytes,
        }
    }
}

pub struct Disk {
    pub vendor: String,
    pub model: String,
    pub serial_number: String,
    pub size_bytes: u64,
    pub mount_point: String,
    pub interface_type: i32,
}

pub fn get_disks() -> Vec<Disk> {
    unsafe {
        let arr = hwinfo_get_disks();
        if arr.disks.is_null() || arr.count <= 0 {
            return vec![];
        }
        let slice = std::slice::from_raw_parts(arr.disks, arr.count as usize);
        let result: Vec<Disk> = slice
            .iter()
            .map(|d| Disk {
                vendor: ptr_to_string(d.vendor),
                model: ptr_to_string(d.model),
                serial_number: ptr_to_string(d.serial_number),
                size_bytes: d.size_bytes,
                mount_point: ptr_to_string(d.mount_point),
                interface_type: d.interface_type,
            })
            .collect();
        hwinfo_free_disk_info(arr);
        result
    }
}
