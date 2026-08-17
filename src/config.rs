use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub config: ConfigSection,
    pub display: DisplayConfig,
    pub startup: Option<StartupConfig>,
    pub lhm: Option<LhmConfig>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct ConfigSection {
    pub com_port: Option<String>,
    pub theme: Option<String>,
    pub eth: Option<String>,
    pub wlo: Option<String>,
    pub ping: Option<String>,
    pub weather_latitude: Option<f64>,
    pub weather_longitude: Option<f64>,
    pub weather_units: Option<String>,
    /// How often WMI/LHM sensor reads are refreshed, in milliseconds.
    /// Default 2000 (2s). Lower values poll hardware more often and
    /// raise CPU/IO load; values below 1000 are discouraged on a
    /// physical status screen.
    pub sensor_interval_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct DisplayConfig {
    pub brightness: Option<u8>,
    pub display_reverse: Option<bool>,
    pub reset_on_startup: Option<bool>,
    /// Shows the debug console window (normally hidden). Errors are
    /// always shown regardless of this setting - this only affects
    /// whether ongoing log output (log::info!/warn! etc.) is visible
    /// during normal operation.
    pub show_console: Option<bool>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct StartupConfig {
    pub run_on_startup: Option<bool>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct LhmConfig {
    pub lhm_path: Option<String>,
}

pub struct Theme {
    pub raw: serde_yaml::Value,
    pub path: PathBuf,
}

impl Theme {
    pub fn load(name: &str) -> Result<Self> {
        let path_str = format!("res/themes/{}/theme.yaml", name);
        let path = resolve_theme_path(&path_str);
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow!("Cannot read {}: {} (cwd: {:?})", path.display(), e, std::env::current_dir()))?;
        let raw: serde_yaml::Value = serde_yaml::from_str(&content)?;
        // Get base theme path
        let theme_dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
        Ok(Theme { raw, path: theme_dir.to_path_buf() })
    }

    pub fn get_str(&self, keys: &[&str]) -> Option<String> {
        get_yaml_str(&self.raw, keys)
    }

    pub fn get_i32(&self, keys: &[&str]) -> Option<i32> {
        get_yaml_i32(&self.raw, keys)
    }

    pub fn get_u32(&self, keys: &[&str]) -> Option<u32> {
        get_yaml_u32(&self.raw, keys)
    }

    pub fn get_f32(&self, keys: &[&str]) -> Option<f32> {
        get_yaml_f32(&self.raw, keys)
    }

    pub fn get_bool(&self, keys: &[&str]) -> Option<bool> {
        get_yaml_bool(&self.raw, keys)
    }

    pub fn get_color(&self, keys: &[&str]) -> Option<image::Rgb<u8>> {
        self.get_str(keys).map(|s| parse_color(&s))
    }

    pub fn file_path(&self, name: &str) -> PathBuf {
        if name.is_empty() {
            self.path.clone()
        } else {
            self.path.join(name)
        }
    }

    pub fn section_exists(&self, keys: &[&str]) -> bool {
        get_yaml_value(&self.raw, keys).is_some()
    }
}

fn get_yaml_value<'a>(value: &'a serde_yaml::Value, keys: &[&str]) -> Option<&'a serde_yaml::Value> {
    let mut current = value;
    for key in keys {
        match current {
            serde_yaml::Value::Mapping(map) => {
                if let Some(v) = map.get(&serde_yaml::Value::String(key.to_string())) {
                    current = v;
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(current)
}

fn get_yaml_str(value: &serde_yaml::Value, keys: &[&str]) -> Option<String> {
    get_yaml_value(value, keys).and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn get_yaml_i32(value: &serde_yaml::Value, keys: &[&str]) -> Option<i32> {
    get_yaml_value(value, keys).and_then(|v| v.as_i64().map(|n| n as i32))
}

fn get_yaml_u32(value: &serde_yaml::Value, keys: &[&str]) -> Option<u32> {
    get_yaml_value(value, keys).and_then(|v| v.as_i64().map(|n| n as u32))
}

fn get_yaml_f32(value: &serde_yaml::Value, keys: &[&str]) -> Option<f32> {
    get_yaml_value(value, keys).and_then(|v| v.as_f64().map(|n| n as f32))
}

fn get_yaml_bool(value: &serde_yaml::Value, keys: &[&str]) -> Option<bool> {
    get_yaml_value(value, keys).and_then(|v| v.as_bool())
}

impl AppConfig {
    pub fn load(path: &str) -> Result<Self> {
        let p = resolve_path(path);
        let content = std::fs::read_to_string(&p)
            .map_err(|e| anyhow!("Cannot read {}: {}", p.display(), e))?;
        let config: AppConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

pub fn parse_color(s: &str) -> image::Rgb<u8> {
    let parts: Vec<&str> = s.split(',').map(|x| x.trim()).collect();
    if parts.len() == 3 {
        let r = parts[0].parse::<u8>().unwrap_or(0);
        let g = parts[1].parse::<u8>().unwrap_or(0);
        let b = parts[2].parse::<u8>().unwrap_or(0);
        image::Rgb([r, g, b])
    } else {
        image::Rgb([255, 255, 255])
    }
}

pub fn resolve_path(relative: &str) -> PathBuf {
    let p = PathBuf::from(relative);
    if p.exists() {
        return p;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let alternative = parent.join(relative);
            if alternative.exists() {
                return alternative;
            }
        }
    }
    PathBuf::from(".").join(relative)
}

pub fn resolve_theme_path(relative: &str) -> PathBuf {
    // 1) Try next to the exe (deployment: assets are copied here by build.rs)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(relative);
            if candidate.exists() {
                return candidate;
            }
        }
    }

    // 2) Try CARGO_MANIFEST_DIR (compile-time absolute project root)
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let p = manifest.join(relative);
    if p.exists() {
        return p;
    }

    // 3) Try from exe dir, walking up to find project root
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().unwrap_or(&manifest).to_path_buf();
        for _ in 0..4 {
            let candidate = dir.join(relative);
            if candidate.exists() {
                return candidate;
            }
            if let Some(parent) = dir.parent().map(|p| p.to_path_buf()) {
                dir = parent;
            } else {
                break;
            }
        }
    }

    // 4) Fallback: try from cwd
    resolve_path(relative)
}
