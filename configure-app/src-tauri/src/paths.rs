// All path resolution goes through here, and all of it defers to
// `mini_system_monitor::config::resolve_theme_path`, which is the
// robust variant (tries compile-time CARGO_MANIFEST_DIR, then walks up
// from the exe dir, then falls back to cwd) - NOT the plain
// `resolve_path`, which only checks a single cwd-relative location and
// was a source of "works here, breaks there" bugs depending on how the
// app was launched (double-click vs terminal vs `cargo tauri dev`).

use mini_system_monitor::config::resolve_theme_path;
use std::path::PathBuf;

pub fn config_yaml_path() -> PathBuf {
    resolve_theme_path("config.yaml")
}

pub fn themes_root() -> PathBuf {
    resolve_theme_path("res/themes")
}

pub fn theme_dir(theme: &str) -> PathBuf {
    themes_root().join(theme)
}

pub fn theme_yaml_path(theme: &str) -> PathBuf {
    theme_dir(theme).join("theme.yaml")
}

pub fn monitor_exe_path() -> PathBuf {
    let exe_name = if cfg!(windows) {
        "mini-system-monitor.exe"
    } else {
        "mini-system-monitor"
    };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(exe_name)))
        .unwrap_or_else(|| PathBuf::from(exe_name))
}
