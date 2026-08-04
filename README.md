# Mini System Monitor

A lightweight system monitor for 3.5-inch Turing Smart Screen USB displays
(320×480) that runs in the Windows system tray. It renders CPU / GPU / memory /
disk / network / weather statistics to the LCD panel and ships with a
Tauri-based settings & theme editor.

> **Credits / acknowledgements.** The **theme structure** used by this project
> is taken from the
> [turing-smart-screen-python](https://github.com/mathoudebine/turing-smart-screen-python)
> project by mathoudebine — full credit for the theme format goes there. This
> project also depends on
> [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor)
> for reading detailed hardware sensors (temperatures, fan speeds, etc.); it
> must be present for those readings to work.

> **Important disclaimer.** This project is provided **as-is**, without any
> warranty, express or implied. You use it entirely **at your own risk and
> responsibility**. The authors take **no responsibility** for any damage,
> data loss, hardware malfunction, or any other adverse outcome that may result
> from building, installing, or using this software. Everyone is free to use it
> however they like, but any consequence of doing so is the sole responsibility
> of the user.

---

## Table of Contents

- [Overview](#overview)
- [Project layout](#project-layout)
- [Requirements](#requirements)
- [How it all fits together](#how-it-all-fits-together)
- [LibreHardwareMonitor](#librehardwaremonitor)
- [Installing Rust](#installing-rust)
- [Building the monitor](#building-the-monitor)
- [Building the settings app](#building-the-settings-app)
- [Building everything](#building-everything)
- [Running](#running)
- [Configuration](#configuration)
- [Themes](#themes)
- [Troubleshooting](#troubleshooting)
- [Disclaimer](#disclaimer)

---

## Overview

The project contains **two independent applications**:

1. **`mini-system-monitor`** — the native daemon. It detects the USB LCD, opens
   the serial port, and continuously draws the theme (background + live stats)
   to the screen. It is a plain Rust binary, **not** a Tauri app, and lives in
   the system tray.
2. **`configure-app`** — a Tauri desktop settings & theme editor. It reads and
   writes `config.yaml`, lets you edit theme YAML both visually and as raw YAML,
   shows a live preview, and can Start/Stop the monitor plus configure Windows
   startup.

Rust is used for both applications. The settings UI uses HTML/CSS/JS, and the
config/themes/data are plain YAML/PNG/TTF files.

---

## Project layout

```
mini/
├── Cargo.toml                     # Workspace root (monitor crate + config-app member)
├── build.rs                       # Copies config.yaml + res/ to the build output
├── config.yaml                    # Main settings (COM port, theme, weather, display)
├── res/
│   ├── fonts/                     # Shared fonts
│   ├── icons/weather/             # Weather icons (sunny, cloudy, rainy, ...)
│   └── themes/                    # Theme folders (each has theme.yaml + background)
│       └── NexusMeter/            # Default theme (theme.yaml + background.png)
├── src/
│   ├── main.rs                    # Monitor daemon (tray, render loop, LCD)
│   ├── config.rs                  # Config + theme YAML loading
│   ├── lcd_comm.rs                # Serial protocol to the USB LCD
│   ├── renderer.rs                # Fonts, text, bars, graphs, icons
│   ├── preview.rs                 # Offline theme preview (shared with config app)
│   ├── stats.rs / sensors.rs      # System statistics (CPU/GPU/disk/network)
│   ├── lhm.rs                     # LibreHardwareMonitor integration (optional)
│   ├── weather.rs                 # Open-Meteo weather fetch + cache
│   └── tray.rs                    # Win32 system tray icon & menu
└── configure-app/
    ├── src/                       # Frontend (HTML/CSS/JS)
    │   ├── index.html
    │   ├── css/style.css
    │   └── js/main.js
    └── src-tauri/
        ├── Cargo.toml
        ├── tauri.conf.json        # Tauri/WebView/Bundler settings
        └── src/
            ├── main.rs            # Tauri entry point (registers commands)
            ├── commands.rs        # Rust commands invoked from the UI
            └── paths.rs
```

---

## Requirements

- **Windows 10 / 11** (primary target; the monitor also compiles on other
  desktop OSes where the serialport crate works, but the system-tray and
  scheduled-task code are Windows-oriented).
- **Rust toolchain** (stable). See [Installing Rust](#installing-rust).
- **A 3.5" Turing Smart Screen USB LCD** (320×480, serial command protocol).
  Auto-detection looks for the `USB35INCHIPSV2` serial number or
  VID/PID `1a86:5722`; otherwise it falls back to scanning open COM ports.
- **For the settings app**: Microsoft Edge **WebView2** runtime — pre-installed
  on Windows 10/11 (what Tauri uses to render the UI).
- **LibreHardwareMonitor** (for detailed hardware readings such as CPU/GPU
  temperatures and fan speeds). See
  [LibreHardwareMonitor below](#librehardwaremonitor).
- **Optional:** Visual Studio Build Tools if you want to rebuild the
  `lhm_reader.exe` helper from source (otherwise a pre-built helper and the
  `LibreHardwareMonitorLib.dll` are used).

---

## How it all fits together

- `cargo build --release` compiles the monitor and (as part of the workspace)
  the settings app.
- `build.rs` automatically copies `config.yaml` and the whole `res/` tree into
  the `target/release/` (or `target/debug/`) folder **next to the compiled
  executable**, so the monitor can find its assets regardless of the current
  working directory.
- The monitor also sets its own working directory to the executable's folder at
  startup, then resolves assets relative to the executable first, falling back
  to the project root.

---

## LibreHardwareMonitor

Detailed hardware sensor readings (CPU/GPU/memory) are provided through
[**LibreHardwareMonitor**](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor).
It must be available for those readings to show up on the screen.

How it is used here:

- A small helper, **`lhm_reader.exe`**, runs LibreHardwareMonitorLib to read
  hardware metrics (temperatures, load, fan speeds, …). It is written in C# and
  its source lives in the `lhm_reader/` folder.
- The helper reads the sensors through `LibreHardwareMonitorLib.dll`, which must
  be located in the `LibreHardwareMonitor/` folder.
- A config option (`lhm: LHM_PATH` in `config.yaml`) lets you point to the
  LibreHardwareMonitor folder if it lives somewhere other than the default
  location next to the executable.

**Obtaining it:**

- Download and unzip
  [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/releases).
- Copy `LibreHardwareMonitorLib.dll` (and the other files it needs) into the
  `LibreHardwareMonitor/` folder next to the executable (or set `LHM_PATH` to
  that folder).

**Rebuilding the helper (optional):** the C# helper is compiled automatically
by `build.rs` using whichever `csc.exe` it can find via Visual Studio / Build
Tools. If `LibreHardwareMonitorLib.dll` or `lhm_reader/Program.cs` is missing,
the build skips the helper with a warning and only the non-LHM readings are
used.

## Installing Rust

If you don't have Rust yet:

1. Install [rustup](https://rustup.rs/) (follow the on-screen instructions) to
   get `cargo` and `rustc`.
2. On Windows ensure the **MSVC toolchain** (`x86_64-pc-windows-msvc`) is
   installed and that the **Visual Studio C++ Build Tools** are present (the
   Rust installer can prompt you about this).
3. Verify with:
   ```sh
   cargo --version
   rustc --version
   ```

---

## Building the monitor

```sh
# Debug build (fast to iterate, for testing)
cargo build

# Optimized release build (smaller, faster, stripped)
cargo build --release
```

After building, the output lives here:

- Debug: `target/debug/mini-system-monitor.exe`
- Release: `target/release/mini-system-monitor.exe`

`config.yaml` and `res/` are copied into the same directory automatically, so
the executable is self-contained there — you can copy the whole
`target/release/` folder to another machine.

---

## Building the settings app

The settings app is a Tauri project. First run once to let the Tauri build
scripts fetch/compile their dependencies:

```sh
cargo build --release --manifest-path configure-app/src-tauri/Cargo.toml
```

To create an installable installer (MSI/NSIS), use the Tauri CLI:

```sh
# Install the Tauri CLI (once)
cargo install tauri-cli --version "^2" --locked

# Build a bundled installer
cargo tauri build
```

The installer will be produced under
`configure-app/src-tauri/target/release/bundle/`.

> **Note:** `frontendDist` in `tauri.conf.json` points to `../src`, so the HTML
> frontend is embedded from the source files directly — no separate frontend
> build step is required.

---

## Building everything

Because `Cargo.toml` already defines a workspace that includes
`configure-app/src-tauri`, a single command compiles both the monitor and the
settings app:

```sh
cargo build --workspace --release
```

Each binary ends up in its own `target/release/` folder. The Tauri things also
need the WebView2 runtime at runtime (already present on modern Windows).

---

## Running

**Monitor (the daemon that draws to the LCD):**

```sh
cd target/release
./mini-system-monitor.exe
```

It will:

1. Load `config.yaml`.
2. Auto-detect the COM port for the LCD (or use `COM_PORT` if set).
3. Initialize the display, send the background, then begin the live update loop.
4. Minimize into the system tray. Use the tray menu to open settings or exit.

**Settings app (recommended to configure everything):**

Launch `configure.exe` from the settings app build output, then:

- General tab: set COM port, theme, network interfaces, weather, brightness,
  display options and Windows-startup behavior.
- **Start Monitor** / **Stop Monitor**: start or stop the daemon from the UI.
- Theme Editor tab: edit the active theme visually and/or as YAML with a live
  preview.

---

## Configuration

`config.yaml` (top-level, copied next to the exe) controls behavior:

```yaml
config:
  COM_PORT: 'AUTO'        # e.g. COM3 on Windows, or AUTO for detection
  THEME: 'NexusMeter'     # theme folder name under res/themes

  ETH: ''                 # network interface(s) for NET stats (empty = all)
  WLO: ''
  PING: '8.8.8.8'         # address used for latency checks

  WEATHER_LATITUDE: 41.01
  WEATHER_LONGITUDE: 28.95
  WEATHER_UNITS: 'metric' # metric (°C) or imperial (°F)

display:
  BRIGHTNESS: 20          # 0-100
  DISPLAY_REVERSE: false
  RESET_ON_STARTUP: false  # keep false: a reset re-enumerates USB and can
                           # invalidate the open COM handle (blank screen)
  SHOW_CONSOLE: false      # set true to see debug log output

startup:
  RUN_ON_STARTUP: false    # register/unregister a Windows scheduled task
  DELAY: 30                # task delay in seconds

lhm:
  LHM_PATH: ''             # optional LibreHardwareMonitor folder
```

> **Important:** keep `RESET_ON_STARTUP: false`. Sending the hardware reset at
> startup makes the USB screen re-enumerate, which breaks the open serial-port
> handle, so every subsequent frame write fails and the screen stays blank.

---

## Themes

Themes live under `res/themes/<Name>/` and each contains:

- `theme.yaml` — positions, fonts, colors and formats for every on-screen
  element (CPU/GPU/mem/disk/net/date/weather).
- `background.png` — the static background image the monitor renders first.
- Font files (`.ttf`) used by the theme.

The **default theme** is `NexusMeter`. You can:

- Switch themes via the settings app (`THEME` value).
- Create your own theme folder by copying an existing one and editing its
  `theme.yaml`.
- Use the **Theme Editor** in the settings app for a live, clickable and
  draggable preview, or edit the raw YAML directly.

Supported element value formats include things like `{VALUE}°C`, `UP {VALUE}
KB/s`, etc., so units are fully under your control.

---

## Troubleshooting

- **Screen stays blank / "device does not recognize the command" (os error 22):**
  This is almost always caused by `RESET_ON_STARTUP: true`. Set it back to
  `false` and restart the monitor. If the display still needs to be cleared,
  reboot the machine / unplug-replug the USB once.
- **"COM port: an invalid argument / no ports":** make sure the display is
  connected (visible in Device Manager as a COM port) and try setting an
  explicit `COM_PORT` in `config.yaml` (e.g. `COM3`).
- **No image on Windows startup but fine when run manually:** check that
  `RESET_ON_STARTUP` is `false` and that the scheduled task points at the same
  executable you use manually. A 30s startup delay is recommended so the USB
  device has time to enumerate at logon.
- **Monitor won't start / instant exit:** check for a `panic.txt` file next to
  the executable — it captures any runtime panic message and backtrace.
- **Settings app preview differs from the real screen:** the screen writes are
  pixel-identical to the preview; differences are usually a missing/corrupt
  background or the reset issue above.
- **Weather not showing:** `WEATHER_LATITUDE`/`WEATHER_LONGITUDE` must be set,
  and the machine needs an internet connection (uses the free Open-Meteo API).
- **Log file:** a `monitor.log` file next to the executable records startup
  steps and serial/display errors to help diagnose issues.

---

## Disclaimer

This software is provided **"as is"**, without warranty of any kind, express or
implied, including but not limited to the warranties of merchantability,
fitness for a particular purpose, and noninfringement. **In no event shall the
authors be liable for any claim, damages, or other liability**, whether in an
action of contract, tort, or otherwise, arising from, out of, or in connection
with the software or the use or other dealings in the software.

Use it freely, on your own risk, at your own responsibility.