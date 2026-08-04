// Always hidden - no DOS box behind the settings window, in dev or release.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod commands;
mod paths;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::load_config,
            commands::save_config,
            commands::list_ports,
            commands::list_interfaces,
            commands::list_themes,
            commands::list_fonts,
            commands::load_theme_yaml,
            commands::save_theme_yaml,
            commands::render_preview,
            commands::theme_layout,
            commands::render_theme_default_preview,
            commands::search_cities,
            commands::launch_monitor,
            commands::stop_monitor,
            commands::set_startup,
            commands::check_startup,
            commands::select_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the configure app");
}
