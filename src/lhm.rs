use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use serde::Deserialize;

static CONFIGURED_PATH: Mutex<Option<String>> = Mutex::new(None);

pub fn set_path(path: String) {
    if let Ok(mut p) = CONFIGURED_PATH.lock() {
        *p = Some(path);
    }
}

fn lhm_exe_path() -> PathBuf {
    if let Ok(guard) = CONFIGURED_PATH.lock() {
        if let Some(ref p) = *guard {
            let candidate = PathBuf::from(p).join("lhm_reader.exe");
            if candidate.exists() {
                return candidate;
            }
            let candidate2 = PathBuf::from(p).join("LibreHardwareMonitor.exe");
            if candidate2.exists() {
                return candidate2;
            }
        }
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let proj_root = manifest.join("LibreHardwareMonitor").join("lhm_reader.exe");
    if proj_root.exists() {
        return proj_root;
    }

    let mut path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("mini-system-monitor.exe"));
    path.pop();
    path.push("LibreHardwareMonitor");
    path.push("lhm_reader.exe");
    if path.exists() {
        return path;
    }

    if let Ok(guard) = CONFIGURED_PATH.lock() {
        if let Some(ref p) = *guard {
            return PathBuf::from(p).join("LibreHardwareMonitor.exe");
        }
    }

    path
}

#[derive(Default, Clone)]
pub struct LhmData {
    pub cpu_temp: f32,
    pub gpu_temp: f32,
    pub gpu_mem_junction_temp: f32,
    pub gpu_load: f32,
    pub gpu_mem_used_mb: f32,
    pub gpu_mem_total_mb: f32,
    pub gpu_freq_mhz: f32,
}

#[derive(Deserialize)]
struct LhmJsonRoot {
    hardware: Vec<LhmJsonHardware>,
}

#[derive(Deserialize)]
struct LhmJsonHardware {
    #[serde(rename = "HardwareType")]
    hardware_type: String,
    name: String,
    sensors: Vec<LhmJsonSensor>,
}

#[derive(Deserialize)]
struct LhmJsonSensor {
    name: String,
    #[serde(rename = "Type")]
    sensor_type: String,
    value: Option<f64>,
}

static CACHE: Lazy<Mutex<Option<(Instant, LhmData)>>> = Lazy::new(|| Mutex::new(None));
static REFRESH_INTERVAL_MS: Mutex<u64> = Mutex::new(3000);

/// Adjusts how often lhm_reader.exe is polled. Default 3000 (3s).
pub fn set_refresh_interval_ms(ms: u64) {
    if let Ok(mut g) = REFRESH_INTERVAL_MS.lock() {
        *g = ms;
    }
}

pub fn refresh() -> LhmData {
    let interval = REFRESH_INTERVAL_MS.lock().map(|g| *g).unwrap_or(3000);
    {
        let cache = CACHE.lock().unwrap();
        if let Some((last, ref data)) = *cache {
            if last.elapsed() < Duration::from_millis(interval) {
                return data.clone();
            }
        }
    }

    let exe = lhm_exe_path();
    let is_lhm_main = exe.file_name().map(|n| n.eq_ignore_ascii_case("LibreHardwareMonitor.exe")).unwrap_or(false);

    // lhm_reader.exe is a console app - without this flag, Windows
    // flashes its console window open-and-closed on every single
    // refresh (every few seconds), which is what was showing up as a
    // constantly flickering DOS box.
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let output = if is_lhm_main {
        let mut cmd = Command::new(&exe);
        cmd.args(["/sensors"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.output()
    } else {
        let mut cmd = Command::new(&exe);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.output()
    };

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            log::warn!("LHM: could not run {:?}: {}", exe, e);
            return LhmData::default();
        }
    };

    if !output.status.success() && !is_lhm_main {
        log::warn!("LHM: process exited with error: {:?}", output.status);
        return LhmData::default();
    }

    let raw = String::from_utf8_lossy(&output.stdout);

    let data = if is_lhm_main {
        parse_lhm_json(&raw)
    } else {
        parse_lhm_pipe(&raw)
    };

    if data.cpu_temp <= 0.0 {
        log::warn!("LHM: CPU temperature sensor not available (try running the monitor as Administrator, or enable 'Run on startup' in Settings for admin via Task Scheduler)");
    }

    {
        let mut cache = CACHE.lock().unwrap();
        *cache = Some((Instant::now(), data.clone()));
    }

    data
}

fn parse_lhm_pipe(raw: &str) -> LhmData {
    log::debug!("LHM raw output:\n{}", raw);
    let mut map: HashMap<&str, &str> = HashMap::new();
    for line in raw.lines() {
        if let Some(idx) = line.find('|') {
            let key = &line[..idx];
            let val = line[idx + 1..].trim();
            map.insert(key, val);
        }
    }
    log::debug!("LHM parsed keys: {:?}", map.keys().collect::<Vec<_>>());

    let mut data = LhmData::default();
    data.cpu_temp = find_sensor(&map, &["Cpu"], "Temperature", &["package", "tctl", "tdie", "core max", "core average", "core"]).unwrap_or(0.0);

    let gpu_hw = ["GpuNvidia", "GpuAmd", "GpuIntel"];
    data.gpu_temp = find_sensor(&map, &gpu_hw, "Temperature", &["gpu core", "core", "hot spot"]).unwrap_or(0.0);
    data.gpu_mem_junction_temp = find_sensor(&map, &gpu_hw, "Temperature", &["memory junction", "junction"]).unwrap_or(0.0);
    data.gpu_load = find_sensor(&map, &gpu_hw, "Load", &["gpu core", "core"]).unwrap_or(0.0);
    data.gpu_mem_used_mb = find_sensor(&map, &gpu_hw, "SmallData", &["gpu memory used", "memory used"]).unwrap_or(0.0);
    data.gpu_mem_total_mb = find_sensor(&map, &gpu_hw, "SmallData", &["gpu memory total", "memory total"]).unwrap_or(0.0);
    data.gpu_freq_mhz = find_sensor(&map, &gpu_hw, "Clock", &["gpu core", "core"]).unwrap_or(0.0);
    data
}

fn parse_lhm_json(raw: &str) -> LhmData {
    log::debug!("LHM JSON output:\n{}", raw);
    let parsed: LhmJsonRoot = match serde_json::from_str(raw) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("LHM: failed to parse JSON output: {}", e);
            return LhmData::default();
        }
    };

    let mut data = LhmData::default();

    for hw in &parsed.hardware {
        let hw_type_lower = hw.hardware_type.to_lowercase();
        let _name_lower = hw.name.to_lowercase();

        for s in &hw.sensors {
            let val = s.value.unwrap_or(0.0) as f32;
            if val <= 0.0 || val > 10000.0 {
                continue;
            }
            let sensor_type = s.sensor_type.to_lowercase();
            let sensor_name = s.name.to_lowercase();

            if sensor_type == "temperature" {
                if hw_type_lower.contains("cpu") {
                    if data.cpu_temp <= 0.0 {
                        data.cpu_temp = val;
                    }
                } else if hw_type_lower.contains("gpu") || hw_type_lower.contains("nvidia") || hw_type_lower.contains("amd") {
                    if sensor_name.contains("memory junction") || sensor_name.contains("junction") {
                        if data.gpu_mem_junction_temp <= 0.0 {
                            data.gpu_mem_junction_temp = val;
                        }
                    } else if sensor_name.contains("hot spot") || sensor_name.contains("hotspot") {
                        // prefer gpu core temp over hot spot
                        if data.gpu_temp <= 0.0 {
                            data.gpu_temp = val;
                        }
                    } else if sensor_name.contains("core") || sensor_name.contains("gpu") {
                        if data.gpu_temp <= 0.0 {
                            data.gpu_temp = val;
                        }
                    }
                }
            } else if sensor_type == "load" {
                if hw_type_lower.contains("gpu") || hw_type_lower.contains("nvidia") || hw_type_lower.contains("amd") {
                    if (sensor_name.contains("core") || sensor_name.contains("gpu")) && data.gpu_load <= 0.0 {
                        data.gpu_load = val;
                    }
                }
            } else if sensor_type == "clock" {
                if (hw_type_lower.contains("gpu") || hw_type_lower.contains("nvidia") || hw_type_lower.contains("amd"))
                    && (sensor_name.contains("core") || sensor_name.contains("gpu"))
                    && data.gpu_freq_mhz <= 0.0
                {
                    data.gpu_freq_mhz = val;
                }
            } else if sensor_type == "smalldata" || sensor_type == "data" {
                if hw_type_lower.contains("gpu") || hw_type_lower.contains("nvidia") || hw_type_lower.contains("amd") {
                    let converted = if val > 1_000_000_000.0 { val / 1024.0 / 1024.0 } else { val };
                    if sensor_name.contains("memory used") && data.gpu_mem_used_mb <= 0.0 {
                        data.gpu_mem_used_mb = converted;
                    }
                    if sensor_name.contains("memory total") && data.gpu_mem_total_mb <= 0.0 {
                        data.gpu_mem_total_mb = converted;
                    }
                }
            }
        }
    }

    data
}

fn parse_f32_locale(s: &str) -> Option<f32> {
    // Try normal parse first (with . decimal separator)
    if let Ok(f) = s.parse::<f32>() {
        return Some(f);
    }
    // Fallback: replace comma with period (Turkish/German locale)
    let normalized = s.replace(',', ".");
    normalized.parse::<f32>().ok()
}

/// Matches sensors from pipe-delimited LHM output (lhm_reader.exe format)
fn find_sensor(
    map: &HashMap<&str, &str>,
    hw_prefixes: &[&str],
    sensor_type: &str,
    name_priority: &[&str],
) -> Option<f32> {
    let mut candidates: Vec<(String, f32)> = Vec::new();
    for (k, v) in map.iter() {
        for hw in hw_prefixes {
            let prefix = format!("{}:{}:", hw, sensor_type);
            if let Some(name) = k.strip_prefix(prefix.as_str()) {
                if let Some(f) = parse_f32_locale(v) {
                    if f > 0.0 && f < 10000.0 {
                        candidates.push((name.to_lowercase(), f));
                    }
                }
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    for wanted in name_priority {
        if let Some((_, val)) = candidates.iter().find(|(name, _)| name.contains(wanted)) {
            return Some(*val);
        }
    }
    candidates.first().map(|(_, v)| *v)
}