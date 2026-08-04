use crate::paths;
use base64::Engine;
use mini_system_monitor::preview;
use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::process::Command;
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConfigDto {
    pub com_port: String,
    pub theme: String,
    pub eth: String,
    pub wlo: String,
    pub ping: String,
    pub weather_latitude: f64,
    pub weather_longitude: f64,
    pub weather_units: String, // "metric" | "imperial"
    pub brightness: u8,
    pub display_reverse: bool,
    pub reset_on_startup: bool,
    pub show_console: bool,
    pub run_on_startup: bool,
    pub startup_delay: u32,
    pub lhm_path: String,
}

impl Default for ConfigDto {
    fn default() -> Self {
        ConfigDto {
            com_port: "AUTO".into(),
            theme: "NexusMeter".into(),
            eth: String::new(),
            wlo: String::new(),
            ping: "8.8.8.8".into(),
            weather_latitude: 41.01,
            weather_longitude: 28.95,
            weather_units: "metric".into(),
            brightness: 20,
            display_reverse: false,
            reset_on_startup: false,
            show_console: false,
            run_on_startup: false,
            startup_delay: 30,
            lhm_path: String::new(),
        }
    }
}

fn yaml_str(v: &serde_yaml::Value, keys: &[&str]) -> Option<String> {
    let mut cur = v;
    for k in keys {
        cur = cur.get(k)?;
    }
    cur.as_str().map(str::to_string)
}
fn yaml_f64(v: &serde_yaml::Value, keys: &[&str]) -> Option<f64> {
    let mut cur = v;
    for k in keys {
        cur = cur.get(k)?;
    }
    cur.as_f64()
}
fn yaml_u64(v: &serde_yaml::Value, keys: &[&str]) -> Option<u64> {
    let mut cur = v;
    for k in keys {
        cur = cur.get(k)?;
    }
    cur.as_u64()
}
fn yaml_bool(v: &serde_yaml::Value, keys: &[&str]) -> Option<bool> {
    let mut cur = v;
    for k in keys {
        cur = cur.get(k)?;
    }
    cur.as_bool()
}

/// Loads config.yaml tolerantly: missing keys fall back to sane
/// defaults instead of failing the whole parse (a single malformed or
/// unexpected field used to be able to break the strict-typed loader).
#[tauri::command]
pub fn load_config() -> Result<ConfigDto, String> {
    let path = paths::config_yaml_path();
    let mut dto = ConfigDto::default();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(dto); // no file yet -> defaults, not an error
    };
    let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return Err(format!("config.yaml has invalid YAML syntax ({})", path.display()));
    };

    if let Some(v) = yaml_str(&yaml, &["config", "COM_PORT"]) {
        dto.com_port = v;
    }
    if let Some(v) = yaml_str(&yaml, &["config", "THEME"]) {
        dto.theme = v;
    }
    if let Some(v) = yaml_str(&yaml, &["config", "ETH"]) {
        dto.eth = v;
    }
    if let Some(v) = yaml_str(&yaml, &["config", "WLO"]) {
        dto.wlo = v;
    }
    if let Some(v) = yaml_str(&yaml, &["config", "PING"]) {
        dto.ping = v;
    }
    if let Some(v) = yaml_f64(&yaml, &["config", "WEATHER_LATITUDE"]) {
        dto.weather_latitude = v;
    }
    if let Some(v) = yaml_f64(&yaml, &["config", "WEATHER_LONGITUDE"]) {
        dto.weather_longitude = v;
    }
    if let Some(v) = yaml_str(&yaml, &["config", "WEATHER_UNITS"]) {
        dto.weather_units = v;
    }
    if let Some(v) = yaml_u64(&yaml, &["display", "BRIGHTNESS"]) {
        dto.brightness = v.clamp(0, 100) as u8;
    }
    if let Some(v) = yaml_bool(&yaml, &["display", "DISPLAY_REVERSE"]) {
        dto.display_reverse = v;
    }
    if let Some(v) = yaml_bool(&yaml, &["display", "RESET_ON_STARTUP"]) {
        dto.reset_on_startup = v;
    }
    if let Some(v) = yaml_bool(&yaml, &["display", "SHOW_CONSOLE"]) {
        dto.show_console = v;
    }
    if let Some(v) = yaml_bool(&yaml, &["startup", "RUN_ON_STARTUP"]) {
        dto.run_on_startup = v;
    }
    if let Some(v) = yaml_u64(&yaml, &["startup", "DELAY"]) {
        dto.startup_delay = v as u32;
    }
    if let Some(v) = yaml_str(&yaml, &["lhm", "LHM_PATH"]) {
        dto.lhm_path = v;
    }
    Ok(dto)
}

#[tauri::command]
pub fn save_config(cfg: ConfigDto) -> Result<(), String> {
    let text = format!(
        r#"---
config:
  # Set your COM port e.g. COM3 for Windows, /dev/ttyACM0 for Linux...
  # Use AUTO for COM port auto-discovery
  COM_PORT: '{com_port}'

  # Theme to use (folder name under res/themes)
  THEME: '{theme}'

  # Network interfaces for NET stats. Leave empty '' to sum ALL interfaces.
  ETH: '{eth}'
  WLO: '{wlo}'

  # Address used for ping/latency checks
  PING: '{ping}'

  # Weather location and units (metric/imperial)
  WEATHER_LATITUDE: {weather_latitude}
  WEATHER_LONGITUDE: {weather_longitude}
  WEATHER_UNITS: '{weather_units}'

display:
  # Display brightness percentage (0-100)
  BRIGHTNESS: {brightness}

  # Reverse display orientation: true/false
  DISPLAY_REVERSE: {display_reverse}

  # Reset display on startup: true/false (can change the COM port on some displays)
  RESET_ON_STARTUP: {reset_on_startup}

  # Shows the debug console window (hidden by default). Errors are
  # always shown regardless of this setting.
  SHOW_CONSOLE: {show_console}

startup:
  # Run the monitor as admin on Windows startup via Task Scheduler
  RUN_ON_STARTUP: {run_on_startup}
  DELAY: {startup_delay}

lhm:
  # Path to LibreHardwareMonitor folder (leave empty for default)
  LHM_PATH: '{lhm_path}'
"#,
        com_port = cfg.com_port,
        theme = cfg.theme,
        eth = cfg.eth,
        wlo = cfg.wlo,
        ping = cfg.ping,
        weather_latitude = cfg.weather_latitude,
        weather_longitude = cfg.weather_longitude,
        weather_units = cfg.weather_units,
        brightness = cfg.brightness,
        display_reverse = cfg.display_reverse,
        reset_on_startup = cfg.reset_on_startup,
        show_console = cfg.show_console,
        run_on_startup = cfg.run_on_startup,
        startup_delay = cfg.startup_delay,
        lhm_path = cfg.lhm_path,
    );
    let path = paths::config_yaml_path();
    std::fs::write(&path, text).map_err(|e| format!("Save error ({}): {e}", path.display()))
}

#[tauri::command]
pub fn list_ports() -> Vec<String> {
    serialport::available_ports()
        .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
        .unwrap_or_default()
}

#[tauri::command]
pub fn list_interfaces() -> Vec<String> {
    let networks = sysinfo::Networks::new_with_refreshed_list();
    let mut names: Vec<String> = networks.iter().map(|(name, _)| name.clone()).collect();
    names.sort();
    names
}

#[tauri::command]
pub fn list_themes() -> Vec<String> {
    let root = paths::themes_root();
    let mut themes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("theme.yaml").exists() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    themes.push(name.to_string());
                }
            }
        }
    }
    themes.sort();
    themes
}

#[tauri::command]
fn theme_name(theme: &str) -> &str {
    if theme.is_empty() { "NexusMeter" } else { theme }
}

#[tauri::command]
pub fn list_fonts(theme: String) -> Vec<String> {
    preview::scan_available_fonts(&paths::theme_dir(theme_name(&theme)))
}

#[tauri::command]
pub fn load_theme_yaml(theme: String) -> Result<String, String> {
    let theme = theme_name(&theme);
    let path = paths::theme_yaml_path(theme);
    std::fs::read_to_string(&path).map_err(|e| format!("Load error ({}): {e}", path.display()))
}

#[tauri::command]
pub fn save_theme_yaml(theme: String, yaml_text: String) -> Result<(), String> {
    let theme = theme_name(&theme);
    serde_yaml::from_str::<serde_yaml::Value>(&yaml_text).map_err(|e| format!("Invalid YAML, not saved: {e}"))?;
    let path = paths::theme_yaml_path(theme);
    std::fs::write(&path, &yaml_text).map_err(|e| format!("Save error ({}): {e}", path.display()))
}

/// Renders whatever YAML text is currently in the editor - saved or
/// not - into a PNG data URL, for the live preview pane. This is what
/// makes the visual editor and the raw YAML editor feel like the same
/// surface: both just call this with the latest text.
#[tauri::command]
pub fn render_preview(theme: String, yaml_text: String) -> Result<String, String> {
    let theme_dir = paths::theme_dir(theme_name(&theme));
    let png = preview::render_preview_png(&theme_dir, &yaml_text)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    Ok(format!("data:image/png;base64,{b64}"))
}

/// Element geometry for the current (unsaved) yaml text, used to draw
/// the click/drag overlay directly on the live preview image.
#[tauri::command]
pub fn theme_layout(theme: String, yaml_text: String) -> Result<preview::LayoutInfo, String> {
    let theme_dir = paths::theme_dir(theme_name(&theme));
    preview::compute_layout(&theme_dir, &yaml_text)
}

/// Renders whatever is currently SAVED on disk for a theme - used by
/// the General tab's preview pane, which only needs to reflect the
/// selected theme, not an in-progress edit.
#[tauri::command]
pub fn render_theme_default_preview(theme: String) -> Result<String, String> {
    let theme = theme_name(&theme);
    let theme_dir = paths::theme_dir(theme);
    let path = paths::theme_yaml_path(theme);
    let yaml_text = std::fs::read_to_string(&path).map_err(|e| format!("Load error ({}): {e}", path.display()))?;
    let png = preview::render_preview_png(&theme_dir, &yaml_text)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    Ok(format!("data:image/png;base64,{b64}"))
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CityResult {
    pub name: String,
    pub admin1: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// Open-Meteo's free geocoding endpoint - same one the original egui
/// app used for city search.
#[tauri::command]
pub fn search_cities(query: String) -> Result<Vec<CityResult>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=8&language=en&format=json",
        urlencoding_simple(trimmed)
    );
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(8))
        .call()
        .map_err(|e| format!("City search error: {e}"))?;
    let json: serde_json::Value = resp.into_json().map_err(|e| format!("Response error: {e}"))?;

    let mut out = Vec::new();
    if let Some(arr) = json.get("results").and_then(|v| v.as_array()) {
        for item in arr {
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            out.push(CityResult {
                name,
                admin1: item.get("admin1").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                country: item.get("country").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                latitude: item.get("latitude").and_then(|v| v.as_f64()).unwrap_or(0.0),
                longitude: item.get("longitude").and_then(|v| v.as_f64()).unwrap_or(0.0),
            });
        }
    }
    Ok(out)
}

/// Minimal query-string escaping - avoids pulling in a whole `url`
/// crate dependency just for one query param.
fn urlencoding_simple(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[tauri::command]
pub fn launch_monitor() -> Result<(), String> {
    let path = paths::monitor_exe_path();
    Command::new(&path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not start monitor ({}): {e}", path.display()))
}

#[tauri::command]
pub fn stop_monitor() -> Result<(), String> {
    let status = Command::new("taskkill")
        .args(["/im", "mini-system-monitor.exe", "/f"])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("Could not stop monitor: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Monitor not running or already stopped".into())
    }
}

const TASK_NAME: &str = "MiniSystemMonitor";

/// Retrieves the current user's SID dynamically via `whoami /user`.
/// Falls back to a well-known pattern if the command fails.
fn get_current_user_sid() -> String {
    let output = Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    if let Ok(out) = output {
        // CSV output: "DOMAIN\user","S-1-5-..."
        let text = String::from_utf8_lossy(&out.stdout);
        for part in text.split(',') {
            let trimmed = part.trim().trim_matches('"');
            if trimmed.starts_with("S-1-5-") {
                return trimmed.to_string();
            }
        }
    }
    // Fallback: use the BUILTIN\Users group (task will still run for the
    // interactive logon user with HighestAvailable run-level).
    "S-1-5-32-545".to_string()
}

#[tauri::command]
pub fn set_startup(enabled: bool, delay_seconds: u32) -> Result<(), String> {
    let exe = paths::monitor_exe_path();
    let exe_str = exe.to_string_lossy();
    let work_dir = exe.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    if enabled {
        let delay_str = if delay_seconds > 0 { format!("PT{}S", delay_seconds.min(3600)) } else { "PT0S".to_string() };
        let user_sid = get_current_user_sid();
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <URI>\{task}</URI>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <Delay>{delay}</Delay>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{sid}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>false</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{cmd}</Command>
      <WorkingDirectory>{wd}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>"#,
            task = TASK_NAME, delay = delay_str, sid = user_sid,
            cmd = exe_str, wd = work_dir
        );
        let temp = std::env::temp_dir().join("MiniSystemMonitor_task.xml");
        std::fs::write(&temp, &xml).map_err(|e| format!("Failed to write task XML: {e}"))?;

        // First try creating without elevation (works if configure-app
        // was already launched as admin).
        let status = Command::new("schtasks")
            .args(["/create", "/xml", &temp.to_string_lossy(), "/tn", TASK_NAME, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("Failed to create scheduled task: {e}"))?;

        if !status.success() {
            // Retry via PowerShell Start-Process -Verb RunAs to trigger
            // the UAC elevation prompt (the user clicks "Yes" once).
            let ps_cmd = format!(
                "Start-Process schtasks -ArgumentList '/create','/xml','\"{}\"','/tn','\"{}\"','/f' -Verb RunAs -Wait -WindowStyle Hidden",
                temp.to_string_lossy().replace('\'', "''"),
                TASK_NAME
            );
            let elevated = Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_cmd])
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .map_err(|e| format!("Failed to elevate scheduled task creation: {e}"))?;
            let _ = std::fs::remove_file(&temp);
            if elevated.success() {
                Ok(())
            } else {
                Err("Failed to create scheduled task (UAC elevation was denied or failed)".into())
            }
        } else {
            let _ = std::fs::remove_file(&temp);
            Ok(())
        }
    } else {
        // Try normal delete first, then elevated if needed
        let status = Command::new("schtasks")
            .args(["/delete", "/tn", TASK_NAME, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("Failed to remove scheduled task: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            let ps_cmd = format!(
                "Start-Process schtasks -ArgumentList '/delete','/tn','\"{}\"','/f' -Verb RunAs -Wait -WindowStyle Hidden",
                TASK_NAME
            );
            let elevated = Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_cmd])
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .map_err(|e| format!("Failed to elevate scheduled task deletion: {e}"))?;
            if elevated.success() {
                Ok(())
            } else {
                Err("Failed to remove scheduled task".into())
            }
        }
    }
}

#[tauri::command]
pub fn check_startup() -> Result<bool, String> {
    let output = Command::new("schtasks")
        .args(["/query", "/tn", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to query scheduled task: {e}"))?;
    Ok(output.status.success())
}

#[tauri::command]
pub fn select_folder() -> Result<Option<String>, String> {
    // Use PowerShell folder browser as a simple cross-Windows dialog
    let script = r#"
        Add-Type -AssemblyName System.Windows.Forms
        $d = New-Object System.Windows.Forms.FolderBrowserDialog
        $d.Description = "Select LibreHardwareMonitor folder"
        $d.ShowNewFolderButton = $false
        if ($d.ShowDialog() -eq "OK") { Write-Output $d.SelectedPath }
    "#;
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| format!("Failed to open folder dialog: {e}"))?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Ok(None)
        } else {
            Ok(Some(path))
        }
    } else {
        Ok(None)
    }
}
