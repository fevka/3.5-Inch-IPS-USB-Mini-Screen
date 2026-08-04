use anyhow::Result;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::cell::RefCell;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use wmi::WMIConnection;

use crate::lhm;

static CACHE: Lazy<Mutex<SensorCache>> = Lazy::new(|| Mutex::new(SensorCache::new()));

thread_local! {
    static WMI: RefCell<Option<WMIConnection>> = const { RefCell::new(None) };
}

fn with_wmi<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&WMIConnection) -> Result<T>,
{
    WMI.with(|cell| {
        if cell.borrow().is_none() {
            *cell.borrow_mut() = Some(WMIConnection::new()?);
        }
        // Drop borrow_mut, get immutable borrow
        let guard = cell.borrow();
        f(guard.as_ref().unwrap())
    })
}

struct SensorCache {
    cpu_temp: f32,
    gpu_name: String,
    gpu_vendor: String,
    gpu_load: f32,
    gpu_temp: f32,
    gpu_mem_junction_temp: f32,
    gpu_mem_used_mb: f32,
    gpu_mem_total_mb: f32,
    gpu_freq_mhz: u64,
    gpu_cores: u64,
    cpu_brand: String,
    cpu_physical_cores: u64,
    cpu_logical_cores: u64,
}

impl SensorCache {
    fn new() -> Self {
        let (gpu_name, gpu_vendor, gpu_mem_total_mb, gpu_freq_mhz, gpu_cores) = load_gpu_static();
        let (cpu_brand, cpu_physical, cpu_logical) = load_cpu_static();
        SensorCache {
            cpu_temp: 0.0,
            gpu_name,
            gpu_vendor,
            gpu_load: 0.0,
            gpu_temp: 0.0,
            gpu_mem_junction_temp: 0.0,
            gpu_mem_used_mb: 0.0,
            gpu_mem_total_mb,
            gpu_freq_mhz,
            gpu_cores,
            cpu_brand,
            cpu_physical_cores: cpu_physical,
            cpu_logical_cores: cpu_logical,
        }
    }
}

fn load_gpu_static() -> (String, String, f32, u64, u64) {
    // hwinfo kaldirildi (proje artik sadece LHM + WMI + sysinfo kullaniyor).
    // GPU adi ve toplam VRAM zaten refresh_all()'daki WMI Win32_VideoController
    // sorgusuyla ayrica dolduruluyor; burada bos baslatip vendor'u isimden
    // tahmin ediyoruz. Frekans/cekirdek sayisi hwinfo olmadan guvenilir
    // sekilde alinamiyor, theme.yaml zaten bunlari kullanmiyor.
    (String::new(), String::new(), 0.0, 0, 0)
}

fn guess_gpu_vendor(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("rtx") || lower.contains("gtx") {
        "NVIDIA".to_string()
    } else if lower.contains("amd") || lower.contains("radeon") {
        "AMD".to_string()
    } else if lower.contains("intel") {
        "Intel".to_string()
    } else {
        String::new()
    }
}

fn load_cpu_static() -> (String, u64, u64) {
    // hwinfo kaldirildi - sysinfo'dan tek seferlik bir okuma yeterli.
    let mut sys = System::new_with_specifics(
        RefreshKind::everything().with_cpu(CpuRefreshKind::everything()),
    );
    sys.refresh_cpu_all();
    let brand = sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();
    let logical = sys.cpus().len() as u64;
    let physical = sys.physical_core_count().map(|n| n as u64).unwrap_or(logical);
    (brand, physical, logical)
}

#[derive(Deserialize)]
struct ThermalZone {
    #[serde(rename = "CurrentTemperature")]
    current_temp: Option<f64>,
}

#[derive(Deserialize)]
struct GpuEnginePerf {
    #[serde(rename = "UtilizationPercentage")]
    utilization: Option<u64>,
    #[serde(rename = "Name")]
    name: Option<String>,
}

#[derive(Deserialize)]
struct GpuAdapterMemory {
    #[serde(rename = "DedicatedUsage")]
    dedicated_usage: Option<u64>,
    #[serde(rename = "SharedUsage")]
    shared_usage: Option<u64>,
    #[serde(rename = "Name")]
    name: Option<String>,
}

// The background thread calls refresh_all() every 200ms so fast,
// sysinfo-based stats (CPU%, network) stay responsive - but the WMI
// queries below are COM calls that cost real CPU time each, and
// temperature/GPU-load readings don't need better than ~2s
// granularity on a physical status screen. Without this throttle,
// refresh_all() was doing 4 full WMI queries five times a SECOND,
// forever, which was the dominant resource cost in the whole program
// (far more than the lhm_reader.exe subprocess, which already had its
// own 3s cache).
static LAST_WMI_REFRESH: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));
const WMI_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

pub fn refresh_all() {
    {
        let mut last = LAST_WMI_REFRESH.lock().unwrap();
        let due = match *last {
            Some(t) => t.elapsed() >= WMI_REFRESH_INTERVAL,
            None => true,
        };
        if !due {
            // LHM already has its own internal cache/throttle, but
            // skip calling it too so we're not even doing that
            // lock-check-and-maybe-spawn dance 5x/sec for nothing.
            return;
        }
        *last = Some(Instant::now());
    }

    // WMI sorgulari (tek baglanti, thread-local'de cache'lenir)
    let _ = with_wmi(|wmi| {
        // CPU sicakligi
        if let Ok(results) = wmi.raw_query::<ThermalZone>(
            "SELECT * FROM MSAcpi_ThermalZoneTemperature"
        ) {
            for tz in &results {
                if let Some(temp_k) = tz.current_temp {
                    let temp_c = (temp_k / 10.0) - 273.15;
                    if temp_c > 0.0 && temp_c < 150.0 {
                        if let Ok(mut cache) = CACHE.lock() {
                            cache.cpu_temp = temp_c as f32;
                        }
                        break;
                    }
                }
            }
        }
        // GPU yuku
        if let Ok(engines) = wmi.raw_query::<GpuEnginePerf>(
            "SELECT * FROM Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine"
        ) {
            let total: u64 = engines.iter()
                .filter_map(|e| {
                    if e.name.as_deref().unwrap_or("").contains("engtype_3D") { e.utilization } else { None }
                })
                .sum();
            if let Ok(mut cache) = CACHE.lock() {
                cache.gpu_load = total.min(100) as f32;
            }
        }
        // GPU bellek kullanimi
        if let Ok(memories) = wmi.raw_query::<GpuAdapterMemory>(
            "SELECT * FROM Win32_PerfFormattedData_GPUPerformanceCounters_GPUAdapterMemory"
        ) {
            for mem in &memories {
                if let Some(ded) = mem.dedicated_usage {
                    if ded > 0 {
                        if let Ok(mut cache) = CACHE.lock() {
                            cache.gpu_mem_used_mb = ded as f32 / (1024.0 * 1024.0);
                        }
                        break;
                    }
                }
            }
        }
        // GPU bilgileri
        if let Ok(controllers) = wmi.raw_query::<VideoController>(
            "SELECT * FROM Win32_VideoController"
        ) {
            if let Ok(mut cache) = CACHE.lock() {
                for gpu in &controllers {
                    if let Some(ram) = gpu.adapter_ram {
                        if ram > 0 { cache.gpu_mem_total_mb = ram as f32 / (1024.0 * 1024.0); }
                    }
                    if cache.gpu_name.is_empty() {
                        if let Some(ref name) = gpu.name {
                            cache.gpu_name = name.clone();
                            if cache.gpu_vendor.is_empty() {
                                cache.gpu_vendor = guess_gpu_vendor(name);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    });

    // LHM verisi
    let lhm_data = lhm::refresh();
    if let Ok(mut cache) = CACHE.lock() {
        if lhm_data.cpu_temp > 0.0 { cache.cpu_temp = lhm_data.cpu_temp; }
        if lhm_data.gpu_temp > 0.0 { cache.gpu_temp = lhm_data.gpu_temp; }
        if lhm_data.gpu_mem_junction_temp > 0.0 { cache.gpu_mem_junction_temp = lhm_data.gpu_mem_junction_temp; }
        if lhm_data.gpu_load > 0.0 { cache.gpu_load = lhm_data.gpu_load; }
        if lhm_data.gpu_mem_used_mb > 0.0 { cache.gpu_mem_used_mb = lhm_data.gpu_mem_used_mb; }
        if lhm_data.gpu_mem_total_mb > 0.0 { cache.gpu_mem_total_mb = lhm_data.gpu_mem_total_mb; }
    }
}

#[derive(Deserialize)]
pub struct VideoController {
    #[serde(rename = "AdapterRAM")]
    pub adapter_ram: Option<u64>,
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "VideoProcessor")]
    pub video_processor: Option<String>,
    #[serde(rename = "DriverVersion")]
    pub driver_version: Option<String>,
}

pub fn get_cpu_temperature() -> f32 {
    CACHE.lock().map(|c| c.cpu_temp).unwrap_or(0.0)
}

pub fn get_cpu_brand() -> String {
    CACHE.lock().map(|c| c.cpu_brand.clone()).unwrap_or_default()
}

pub fn get_cpu_physical_cores() -> u64 {
    CACHE.lock().map(|c| c.cpu_physical_cores).unwrap_or(0)
}

pub fn get_cpu_logical_cores() -> u64 {
    CACHE.lock().map(|c| c.cpu_logical_cores).unwrap_or(0)
}

pub fn get_gpu_name() -> String {
    CACHE.lock().map(|c| c.gpu_name.clone()).unwrap_or_default()
}

pub fn get_gpu_vendor() -> String {
    CACHE.lock().map(|c| c.gpu_vendor.clone()).unwrap_or_default()
}

pub fn get_gpu_load() -> f32 {
    CACHE.lock().map(|c| c.gpu_load).unwrap_or(0.0)
}

pub fn get_gpu_temperature() -> f32 {
    CACHE.lock().map(|c| c.gpu_temp).unwrap_or(0.0)
}

pub fn get_gpu_mem_junction_temperature() -> f32 {
    CACHE.lock().map(|c| c.gpu_mem_junction_temp).unwrap_or(0.0)
}

pub fn get_gpu_memory_used_mb() -> f32 {
    CACHE.lock().map(|c| c.gpu_mem_used_mb).unwrap_or(0.0)
}

pub fn get_gpu_memory_total_mb() -> f32 {
    CACHE.lock().map(|c| c.gpu_mem_total_mb).unwrap_or(0.0)
}

pub fn get_gpu_frequency_mhz() -> u64 {
    CACHE.lock().map(|c| c.gpu_freq_mhz).unwrap_or(0)
}

pub fn get_gpu_cores() -> u64 {
    CACHE.lock().map(|c| c.gpu_cores).unwrap_or(0)
}

pub fn get_cpu_frequencies() -> Vec<i64> {
    // hwinfo kaldirildi. Bos donuyoruz - cagiran taraf (stats.rs
    // cpu_frequency_mhz) bos gelince zaten sysinfo'nun kendi
    // cpu.frequency() degerine dusuyor, davranis degismedi.
    Vec::new()
}
