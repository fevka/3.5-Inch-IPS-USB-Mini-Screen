// This library exists to share code between main.rs (the monitor) and
// src/bin/configure.rs (the settings app): config.yaml loading/saving,
// and now the text/graphics renderer too, so the settings app's theme
// previews are pixel-accurate (same code path as the real screen).
pub mod config;
pub mod preview;
pub mod renderer;
