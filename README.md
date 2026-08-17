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
  - [Visual theme editor](#visual-theme-editor)
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
   writes `config.yaml`, edits theme YAML **visually** (a live preview where you
   can add, drag, align and tweak elements) or as raw YAML, and can Start/Stop
   the monitor plus configure Windows startup.

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
- `build.rs` automatically copies `config.yaml`, the whole `res/` tree and the
  `LibreHardwareMonitor/` folder (DLLs + the compiled `lhm_reader.exe`) into the
  `target/release/` (or `target/debug/`) folder **next to the compiled
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
  display options, sensor refresh rate and Windows-startup behavior.
- **Start Monitor** / **Stop Monitor**: start or stop the daemon from the UI.
- Theme Editor tab: a full **visual theme editor** with a live preview (see
  [Visual theme editor](#visual-theme-editor)), plus raw YAML editing.

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

  SENSOR_INTERVAL_MS: 2000  # how often WMI/LHM sensor reads refresh (ms).
                            # Lower values poll hardware more often and
                            # raise CPU/IO load; 2000-3000 recommended.
                            # Values below 1000ms are discouraged.

display:
  BRIGHTNESS: 20          # 0-100
  DISPLAY_REVERSE: false
  RESET_ON_STARTUP: false  # keep false: a reset re-enumerates USB and can
                           # invalidate the open COM handle (blank screen)
  SHOW_CONSOLE: false      # set true to see debug log output

startup:
  RUN_ON_STARTUP: false    # register/unregister a Windows scheduled task
                           # (runs the monitor elevated at logon)
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

### Visual theme editor

The **Theme Editor** tab of the settings app edits the active theme through a
live preview instead of raw YAML. It is built around a single, consistent
concept: **every element with an `X`/`Y` position in the theme is an editable
box on the preview**.

- **Element palette** — a compact list of ready-to-add blocks (CPU %, GPU, MEM,
  NET, WEATHER, DATE, static text/labels, bars, icons). Click one to open an
  "Add element" dialog where you set the type, exact `X`/`Y`, text, font size,
  colors and bar/icon width & height before it is inserted into the theme.
- **Drag to move** — drag any box directly on the preview. The element follows
  the cursor exactly (anchor-aware, so right/center-anchored text like
  `ANCHOR: rt` / `mt` moves correctly) and the underlying YAML is updated.
- **Click to select** — clicking a box on the preview (or an entry in the
  element list on the left) selects that element; the two views stay in sync.
  Selecting opens that element's details panel, where `X`/`Y`, text, colors,
  font, size and `SHOW` can be edited numerically with immediate live feedback.
- **Alignment toolbar** — with an element selected, snap it to the screen:
  **Align L/R** (screen left/right edge), **Align T/B** (top/bottom edge), and
  **Center X/Y** (screen center). Every action is applied to the single selected
  element and is immediately visible on the preview.
- **Delete** — remove an element with its delete button in the element list.
- **Raw YAML** — the YAML editor stays in sync with every visual change, so you
  can fine-tune anything by hand and see it reflected on the preview instantly.

> The editor uses a **single-selection** model (one element at a time), which
> keeps the layout structure of `theme.yaml` intact — each box corresponds to
> one element block (e.g. a stat's `TEXT`/`GRAPH`/`ICON` sub-block).

### Supported theme elements

A theme can place any combination of the following on the screen. Each block
is positioned with `X`/`Y`, sized, aligned (`ALIGN`), anchored (`ANCHOR`: `lt`,
`rt`, `mt`, …) and styled with a font + color. Every stat supports `SHOW: true`
or `SHOW: false`, a per-stat update `INTERVAL` (seconds), and can be rendered as
plain `TEXT`, a percentage `GRAPH`/bar, or both.

- **CPU** — percentage (text + bar graph), frequency, temperature.
- **GPU** — percentage (text + bar graph), memory, temperature.
- **MEMORY** — used (bytes + percent), virtual-memory graph.
- **NET** — upload / download speed per interface (WLO/ETH), as text.
- **WEATHER** — current temperature, "feels like", humidity, a text
  description, **and a weather icon** (see below).
- **DATE** — time (hour), day of the week, with editable `FORMAT`
  (`short` / `long` / custom).
- **static text / static images** — fixed labels and overlays that never change
  during runtime.

### Weather icons

`res/icons/weather/` ships ready-to-use weather icons (PNG). The weather block's
`ICON` entry (`X`/`Y`/`WIDTH`/`HEIGHT`/`SHOW`) tells the monitor where to draw
the matching icon. The icon is chosen automatically from the current conditions:

| Condition            | Icon file       |
|----------------------|-----------------|
| Clear / sunny        | `sunny.png`     |
| Partly / full cloud  | `cloudy.png`    |
| Rain                 | `rainy.png`     |
| Thunderstorms        | `stormy.png`    |
| Snow                 | `snowy.png`     |
| Mist / fog           | `foggy.png`     |

Weather data comes from the free Open-Meteo API (no API key needed) using
`WEATHER_LATITUDE` / `WEATHER_LONGITUDE` and updates every `WEATHER`
`INTERVAL` seconds. The icon is alpha-blended onto the theme.

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
  steps and serial/display errors to help diagnose issues. The same messages
  also go to the console when `SHOW_CONSOLE: true`.
- **Temperature/load values update slowly:** sensor values are polled at
  `SENSOR_INTERVAL_MS` (default 2000 ms). Raise/lower it in `config.yaml` or in
  the settings app under **Advanced → Sensor refresh rate**. Lower values poll
  the hardware more often at the cost of extra CPU/IO load; on a physical
  status screen 2000–3000 ms is the sweet spot.

### Run as Administrator is required for temperatures

Windows does **not** expose temperature/fan readings to a normal (non-elevated)
process the way it gives you CPU usage or memory. To read hardware temperatures
you must run the monitor **as Administrator**, otherwise the temperature values
stay unavailable/blank (the monitor itself logs a warning about this).

- Recommended: enable **"Run on startup"** in the Settings app — the scheduled
  task is created to run the monitor **with highest privileges**, so it starts
  elevated on Windows logon without asking.
- If you run the monitor manually, right-click the `.exe` → **Run as
  administrator** so temperature readings appear.

### Hardware sensors: HWiNFO / PawnIO

Windows does not provide the low-level thermal/fan data directly either. This
project relies on **LibreHardwareMonitor** (via `LibreHardwareMonitorLib.dll` +
`lhm_reader.exe`) to read sensor values. On some systems LibreHardwareMonitor in
turn depends on **HWiNFO** (referred to here as "PawnIO") being installed and
running, because its sensor driver is what Windows needs to read the thermal
chips. So for reliable CPU/GPU temperatures:

1. Install/run **HWiNFO** (PawnIO sensor driver) so the thermal data is exposed.
2. Make sure **LibreHardwareMonitor** is present (see the
   [LibreHardwareMonitor](#librehardwaremonitor) section).
3. Run the monitor as Administrator (required for elevated access to sensors).

Without the sensor layer in place, the monitor still runs and draws CPU/memory/
network/stats, but temperatures and fan readings will be missing.

### Startup / Task Scheduler issues

Enabling **"Run on startup"** registers a Windows Task Scheduler task that
launches the monitor elevated at logon with a configurable delay (`DELAY`).
Common problems and fixes:

- **Task is "Activation" is not enabled / shows no trigger.** The task must have
  an enabled **At logon** trigger. Recreate it from the Settings app (uncheck
  then re-check "Run on startup") so the trigger is written correctly.
- **Task is created but never runs.** Verify it appears in **Task Scheduler →
  Task Scheduler Library → \\{monitor folder}** with status *Ready* and that the
  **Run with highest privileges** checkbox is checked. Check the **Last Run
  Result** code there.
- **A console window flashes at logon.** The monitor is launched hidden; a brief
  flash is normal. If it persists, ensure the task's program path points at the
  actual `mini-system-monitor.exe` (working directory set to its folder).
- **Monitor starts but screen stays black at logon.** The USB display may not
  have finished enumerating yet — increase `startup: DELAY` in `config.yaml`
  (e.g. 30–45 s) so the serial port is available. Only **one** monitor process
  may run at a time — a second instance is rejected via a named mutex, since
  two processes writing the same COM port corrupt the screen. If the tray icon
  disappears right after launch, an earlier instance is already running.
- **Task not created because not running as admin.** Registering a scheduled
  task that runs with highest privileges requires an administrator account. Run
  the Settings app as administrator to create/modify the task.
- **Temperatures missing when auto-started from Task Scheduler.** Make sure the
  task runs the monitor elevated ("highest privileges"), same as the manual
  Administrator requirement above.

---

## Disclaimer

This software is provided **"as is"**, without warranty of any kind, express or
implied, including but not limited to the warranties of merchantability,
fitness for a particular purpose, and noninfringement. **In no event shall the
authors be liable for any claim, damages, or other liability**, whether in an
action of contract, tort, or otherwise, arising from, out of, or in connection
with the software or the use or other dealings in the software.

Use it freely, on your own risk, at your own responsibility.