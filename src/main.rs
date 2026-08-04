// Hidden by default (no more DOS box sitting open behind the display).
// If something goes wrong OR the user turns on DISPLAY.SHOW_CONSOLE in
// config.yaml, a console is allocated at runtime (see main()) so
// errors and debug logs are never silently swallowed.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod lcd_comm;
mod lhm;
mod sensors;
mod stats;
mod tray;
mod weather;

use anyhow::Result;
use image::Rgb;
use lcd_comm::LcdComm;
use std::collections::HashMap;
use mini_system_monitor::config::{self, AppConfig, Theme};
use mini_system_monitor::renderer::Renderer;
use stats::SystemStats;
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 480;

struct ThemeRenderer {
    renderer: Renderer,
    theme: Theme,
    bg: image::RgbImage,
    icon_cache: HashMap<String, image::RgbaImage>,
}

impl ThemeRenderer {
    fn new(theme: Theme) -> Self {
        let mut r = ThemeRenderer {
            renderer: Renderer::new(),
            theme,
            bg: image::RgbImage::new(WIDTH, HEIGHT),
            icon_cache: HashMap::new(),
        };
        r.load_fonts();
        r.load_background();
        r.draw_static_text_labels();
        r
    }

    fn load_fonts(&mut self) {
        // Temadaki TUM "FONT: ..." referanslarini (sadece static_text
        // degil, STATS altindaki her yerdeki) tara ve yukle.
        let mut font_names = Vec::new();
        collect_font_names(&self.theme.raw, &mut font_names);
        for font in font_names {
            self.load_theme_font(&font);
        }
    }

    /// Fontu yukler ve ONEMLI: temada gecen DUZ isimle (orn.
    /// "generale-mono/GeneraleMonoA.ttf") cache'ler. Eskiden font,
    /// path-onekli farkli bir anahtar altinda kaydediliyordu
    /// (Renderer::load_font, verilen tam yolu anahtar olarak
    /// kullaniyordu), ama cizim sirasinda tema dosyasindaki DUZ isimle
    /// arama yapiliyordu - anahtarlar hic eslesmiyordu, bu yuzden her
    /// metin sessizce gomulu varsayilan fonta (RobotoMono) dusuyordu.
    fn load_theme_font(&mut self, font: &str) {
        if self.renderer.fonts.contains_key(font) {
            return;
        }
        let candidate1 = format!("{}/{}", self.theme.path.display(), font);
        if let Some(f) = self.renderer.load_font(&candidate1) {
            self.renderer.fonts.insert(font.to_string(), f);
            return;
        }
        let candidate2 = format!("res/fonts/{}", font);
        if let Some(f) = self.renderer.load_font(&candidate2) {
            self.renderer.fonts.insert(font.to_string(), f);
        }
    }

    fn load_background(&mut self) {
        if let Some(bg) = get(&self.theme.raw, &["static_images", "BACKGROUND"]) {
            if let Some(path) = get_str(bg, &["PATH"]) {
                let full = self.theme.file_path(&path);
                if let Ok(img) = image::open(&full) {
                    self.bg = img.to_rgb8();
                }
            }
        }
    }

    /// static_text altindaki sabit etiketleri (orn. "CPU", "GPU", "MEM",
    /// "NET") arka plan resmine gomer. Eskiden bu bolum SADECE font
    /// on-yuklemesi icin okunuyordu, etiketler hicbir zaman fiilen
    /// CIZILMIYORDU - background.png icinde hazir degillerse ekranda
    /// hic gorunmuyorlardi.
    fn draw_static_text_labels(&mut self) {
        let entries: Vec<serde_yaml::Value> =
            if let Some(texts) = get(&self.theme.raw, &["static_text"]).and_then(|v| v.as_mapping()) {
                texts.values().cloned().collect()
            } else {
                Vec::new()
            };

        for val in entries {
            let text = match get_str(&val, &["TEXT"]) {
                Some(t) if !t.is_empty() => t,
                _ => continue,
            };
            let x = get(&val, &["X"]).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let y = get(&val, &["Y"]).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let size = get(&val, &["FONT_SIZE"]).and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
            let color = get_str(&val, &["FONT_COLOR"])
                .map(|s| config::parse_color(&s))
                .unwrap_or(Rgb([255, 255, 255]));
            let font = get_str(&val, &["FONT"]);
            let anchor = get_str(&val, &["ANCHOR"]).unwrap_or_else(|| "lt".to_string());

            let (tw, th) = self.renderer.measure_text(&text, size, font.as_deref());
            let mut chars = anchor.chars();
            let h_anchor = chars.next().unwrap_or('l');
            let v_anchor = chars.next().unwrap_or('t');
            let draw_x = match h_anchor {
                'r' => x - tw as i32,
                'm' => x - (tw as i32) / 2,
                _ => x,
            }.max(0);
            let draw_y = match v_anchor {
                'b' => y - th as i32,
                'm' => y - (th as i32) / 2,
                _ => y,
            }.max(0);

            self.renderer.draw_text_font(&mut self.bg, &text, draw_x, draw_y, size, color, font.as_deref());
        }
    }

    fn bg_portion(&self, x: i32, y: i32, w: u32, h: u32) -> image::RgbImage {
        let mut img = image::RgbImage::new(w.max(1), h.max(1));
        for py in 0..h {
            for px in 0..w {
                let bx = (x + px as i32).clamp(0, WIDTH as i32 - 1) as u32;
                let by = (y + py as i32).clamp(0, HEIGHT as i32 - 1) as u32;
                img.put_pixel(px as u32, py as u32, *self.bg.get_pixel(bx, by));
            }
        }
        img
    }

    fn get_text_cfg(&self, keys: &[&str]) -> Option<(i32, i32, f32, Rgb<u8>, Option<String>, String)> {
        if !self.theme.get_bool(&[keys, &["SHOW"]].concat()).unwrap_or(false) { return None; }
        let x = self.theme.get_i32(&[keys, &["X"]].concat())?;
        let y = self.theme.get_i32(&[keys, &["Y"]].concat())?;
        let size = self.theme.get_f32(&[keys, &["FONT_SIZE"]].concat()).unwrap_or(10.0);
        let color = self.theme.get_color(&[keys, &["FONT_COLOR"]].concat()).unwrap_or(Rgb([255, 255, 255]));
        let font = self.theme.get_str(&[keys, &["FONT"]].concat());
        // ANCHOR: ilk harf yatay (l=left, m=middle, r=right),
        // ikinci harf dikey (t=top, m=middle, b=bottom) referans noktasi.
        // Tema dosyasindaki X,Y bu referans noktasidir - "sol-ust kose"
        // degil. Belirtilmemisse "lt" (eski davranisla ayni) varsayilir.
        let anchor = self.theme.get_str(&[keys, &["ANCHOR"]].concat()).unwrap_or_else(|| "lt".to_string());
        Some((x, y, size, color, font, anchor))
    }

    fn get_graph_cfg(&self, keys: &[&str]) -> Option<(i32, i32, u32, u32, f32, f32, Rgb<u8>)> {
        if !self.theme.get_bool(&[keys, &["SHOW"]].concat()).unwrap_or(false) { return None; }
        let x = self.theme.get_i32(&[keys, &["X"]].concat())?;
        let y = self.theme.get_i32(&[keys, &["Y"]].concat())?;
        let w = self.theme.get_u32(&[keys, &["WIDTH"]].concat()).unwrap_or(100);
        let h = self.theme.get_u32(&[keys, &["HEIGHT"]].concat()).unwrap_or(10);
        let min_v = self.theme.get_f32(&[keys, &["MIN_VALUE"]].concat()).unwrap_or(0.0);
        let max_v = self.theme.get_f32(&[keys, &["MAX_VALUE"]].concat()).unwrap_or(100.0);
        let color = self.theme.get_color(&[keys, &["BAR_COLOR"]].concat()).unwrap_or(Rgb([255, 255, 255]));
        Some((x, y, w, h, min_v, max_v, color))
    }

    fn get_line_cfg(&self, keys: &[&str]) -> Option<(i32, i32, u32, u32, f32, f32, Rgb<u8>)> {
        if !self.theme.get_bool(&[keys, &["SHOW"]].concat()).unwrap_or(false) { return None; }
        let x = self.theme.get_i32(&[keys, &["X"]].concat())?;
        let y = self.theme.get_i32(&[keys, &["Y"]].concat())?;
        let w = self.theme.get_u32(&[keys, &["WIDTH"]].concat()).unwrap_or(100);
        let h = self.theme.get_u32(&[keys, &["HEIGHT"]].concat()).unwrap_or(30);
        let min_v = self.theme.get_f32(&[keys, &["MIN_VALUE"]].concat()).unwrap_or(0.0);
        let max_v = self.theme.get_f32(&[keys, &["MAX_VALUE"]].concat()).unwrap_or(100.0);
        let color = self.theme.get_color(&[keys, &["LINE_COLOR"]].concat()).unwrap_or(Rgb([0, 200, 255]));
        Some((x, y, w, h, min_v, max_v, color))
    }

    fn render_text(&self, value: &str, keys: &[&str]) -> Option<(image::RgbImage, i32, i32)> {
        let (x, y, size, color, font, anchor) = self.get_text_cfg(keys)?;
        let (tw, th) = self.renderer.measure_text(value, size, font.as_deref());
        let w = tw.max(1);
        let h = th.max(1);
        // Pad width so text length changes don't leave old pixels on the display.
        // For right-anchored text the extra width goes left (prev draw_x offset),
        // for left-anchored it goes right (just wider portion, same draw_x).
        let pad = 40u32;
        let mut chars = anchor.chars();
        let h_anchor = chars.next().unwrap_or('l');
        let v_anchor = chars.next().unwrap_or('t');

        let draw_x = match h_anchor {
            'r' => (x - (w + pad) as i32).max(0),
            'm' => (x - (w + pad) as i32 / 2).max(0),
            _ => x.max(0),
        };
        let draw_y = match v_anchor {
            'b' => y - h as i32,
            'm' => y - (h as i32) / 2,
            _ => y,
        }.max(0);

        let portion_w = if h_anchor == 'r' { w + pad } else { w + pad };
        let mut img = self.bg_portion(draw_x, draw_y, portion_w, h);
        let text_offset_x = match h_anchor {
            'r' => pad as i32,  // pad on the left, text at right
            'm' => (pad / 2) as i32,  // pad equally on both sides
            _ => 0,  // text at left, pad on right
        };
        self.renderer.draw_text_font(&mut img, value, text_offset_x, 0, size, color, font.as_deref());
        Some((img, draw_x, draw_y))
    }

    fn render_graph_bar(&self, value: f32, keys: &[&str]) -> Option<(image::RgbImage, i32, i32)> {
        let (x, y, w, h, min_v, max_v, bar_color) = self.get_graph_cfg(keys)?;
        let fill = ((value - min_v) / (max_v - min_v).max(0.01) * w as f32).clamp(0.0, w as f32) as u32;
        let mut img = self.bg_portion(x, y, w, h);
        for py in 0..h {
            for px in 0..fill {
                img.put_pixel(px as u32, py as u32, bar_color);
            }
        }
        Some((img, x, y))
    }

    fn render_line_graph(&self, values: &[f32], keys: &[&str]) -> Option<(image::RgbImage, i32, i32)> {
        if values.len() < 2 { return None; }
        let (x, y, w, h, min_v, max_v, line_color) = self.get_line_cfg(keys)?;
        let mut img = self.bg_portion(x, y, w, h);
        let range = (max_v - min_v).max(0.01);
        let n = values.len();
        for i in 1..n {
            let x1 = ((i - 1) as f32 / (n - 1) as f32 * w as f32) as i32;
            let y1 = (h as f32 * (1.0 - ((values[i - 1] - min_v) / range))) as i32;
            let x2 = (i as f32 / (n - 1) as f32 * w as f32) as i32;
            let y2 = (h as f32 * (1.0 - ((values[i] - min_v) / range))) as i32;
            if values[i - 1].is_nan() || values[i].is_nan() { continue; }
            draw_line(&mut img, x1, y1, x2, y2, line_color, 2);
        }
        Some((img, x, y))
    }

    fn render_icon_img(&self, icon: &image::RgbaImage, keys: &[&str]) -> Option<(image::RgbImage, i32, i32)> {
        if !self.theme.get_bool(&[keys, &["SHOW"]].concat()).unwrap_or(false) { return None; }
        let x = self.theme.get_i32(&[keys, &["X"]].concat())?;
        let y = self.theme.get_i32(&[keys, &["Y"]].concat())?;
        let w = self.theme.get_u32(&[keys, &["WIDTH"]].concat()).unwrap_or(icon.width());
        let h = self.theme.get_u32(&[keys, &["HEIGHT"]].concat()).unwrap_or(icon.height());
        let resized = image::imageops::resize(icon, w, h, image::imageops::FilterType::Lanczos3);
        let mut img = self.bg_portion(x, y, w, h);
        self.renderer.draw_icon(&mut img, &resized, 0, 0);
        Some((img, x, y))
    }
}

static WEATHER_ICON_CACHE: Lazy<Mutex<HashMap<String, image::RgbaImage>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn load_weather_icon(icon_name: &str, theme_path: &std::path::Path) -> Option<image::RgbaImage> {
    {
        let cache = WEATHER_ICON_CACHE.lock().unwrap();
        if let Some(img) = cache.get(icon_name) {
            return Some(img.clone());
        }
    }
    let paths = [
        theme_path.join("icons").join("weather").join(format!("{}.png", icon_name)),
        std::path::PathBuf::from("res/icons/weather").join(format!("{}.png", icon_name)),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("res/icons/weather").join(format!("{}.png", icon_name)),
    ];
    for p in &paths {
        if let Ok(img) = image::open(p) {
            let rgba = img.to_rgba8();
            if let Ok(mut cache) = WEATHER_ICON_CACHE.lock() {
                cache.insert(icon_name.to_string(), rgba.clone());
            }
            return Some(rgba);
        }
    }
    None
}

fn draw_line(img: &mut image::RgbImage, x1: i32, y1: i32, x2: i32, y2: i32, color: Rgb<u8>, width: u32) {
    let dx = (x2 - x1).abs();
    let dy = -(y2 - y1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (x1, y1);
    loop {
        for wy in 0..width as i32 {
            for wx in 0..width as i32 {
                let px = x + wx - (width as i32 / 2);
                let py = y + wy - (width as i32 / 2);
                if px >= 0 && py >= 0 && px < img.width() as i32 && py < img.height() as i32 {
                    img.put_pixel(px as u32, py as u32, color);
                }
            }
        }
        if x == x2 && y == y2 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

fn get<'a>(value: &'a serde_yaml::Value, keys: &[&str]) -> Option<&'a serde_yaml::Value> {
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

fn get_str(value: &serde_yaml::Value, keys: &[&str]) -> Option<String> {
    get(value, keys).and_then(|v| v.as_str().map(|s| s.to_string()))
}

/// Tema agacinin tamaminda (static_text, STATS, vs. fark etmeksizin)
/// gecen her "FONT: ..." degerini toplar. Boylece STATS altindaki
/// metinlerin kullandigi fontlar da onceden yuklenip dogru anahtarla
/// cache'lenir (eskiden sadece static_text altindakiler yukleniyordu).
fn collect_font_names(value: &serde_yaml::Value, out: &mut Vec<String>) {
    if let serde_yaml::Value::Mapping(map) = value {
        for (k, v) in map {
            if let serde_yaml::Value::String(key) = k {
                if key == "FONT" {
                    if let Some(s) = v.as_str() {
                        out.push(s.to_string());
                    }
                    continue;
                }
            }
            collect_font_names(v, out);
        }
    }
}

#[cfg(windows)]
fn alloc_console() {
    unsafe {
        let _ = windows_sys::Win32::System::Console::AllocConsole();
    }
}
#[cfg(not(windows))]
fn alloc_console() {}

/// Lightweight, tolerant peek at config.yaml's DISPLAY.SHOW_CONSOLE -
/// deliberately independent of the strict `AppConfig::load` used by
/// `run()`, so a missing/broken config file just means "console stays
/// hidden" instead of it deciding console visibility AFTER already
/// needing the console to report that same error.
fn should_show_console() -> bool {
    let path = config::resolve_path("config.yaml");
    let Ok(text) = std::fs::read_to_string(&path) else { return false };
    let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&text) else { return false };
    yaml.get("display")
        .and_then(|d| d.get("SHOW_CONSOLE"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            },
        };
        let location = info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());
        
        let backtrace = std::backtrace::Backtrace::force_capture();
        
        let log_msg = format!(
            "Panic occurred at {}\nMessage: {}\nBacktrace:\n{}",
            location, msg, backtrace
        );
        
        // Save to panic.txt next to the executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let file_path = parent.join("panic.txt");
                let _ = std::fs::write(&file_path, &log_msg);
            }
        }
        
        // Also try writing to current working directory
        let _ = std::fs::write("panic.txt", &log_msg);
    }));

    if should_show_console() {
        alloc_console();
    }

    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    if let Err(e) = run() {
        // Make sure the error is visible even if the console was
        // hidden this whole run - silently failing with no visible
        // window would be far worse than an unwanted DOS box.
        alloc_console();
        eprintln!("\n=== ERROR ===");
        eprintln!("{}", e);
        eprintln!("\nProgram will exit. See details above.");
        eprintln!("Press Enter...");
        let _ = std::io::stdin().read_line(&mut String::new());
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cfg = AppConfig::load("config.yaml").map_err(|e| {
        anyhow::anyhow!("config.yaml not found or invalid: {}\n\nMake sure you are in the project directory.\nConfig is searched in:\n  - working directory\n  - alongside the executable", e)
    })?;

    // Set LHM path if configured
    if let Some(ref lhm_cfg) = cfg.lhm {
        if let Some(ref p) = lhm_cfg.lhm_path {
            if !p.is_empty() {
                lhm::set_path(p.clone());
            }
        }
    }

    let theme_name = cfg.config.theme.as_deref().unwrap_or("NexusMeter");
    let theme = Theme::load(theme_name).unwrap_or_else(|e| {
        log::warn!("Theme '{}' failed to load ({}), using empty theme", theme_name, e);
        Theme {
            raw: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            path: std::path::PathBuf::from("."),
        }
    });

    let theme_renderer = ThemeRenderer::new(theme);

    let com_port = match cfg.config.com_port.as_deref() {
        Some("AUTO") | None => match LcdComm::auto_detect() {
            Ok(p) => {
                log::info!("COM port: {}", p);
                p
            }
            Err(_) => {
                log::warn!("Auto-detect failed, trying COM3");
                "COM3".to_string()
            }
        },
        Some(p) => p.to_string(),
    };

    let display = match LcdComm::new(&com_port) {
        Ok(d) => {
            // Eskiden parlaklik hep 20'de SABIT kodluydu - config.yaml'daki
            // BRIGHTNESS degeri hicbir zaman okunmuyordu.
            let brightness = cfg.display.brightness.unwrap_or(20);
            if let Err(e) = d.initialize(brightness) {
                log::error!("LCD init error (continuing): {}", e);
            }
            Some(d)
        }
        Err(e) => {
            log::error!("LCD connection error (continuing, debug frame will be saved): {}", e);
            None
        }
    };

    if display.is_none() {
        log::error!("No LCD connection, exiting");
        return Err(anyhow::anyhow!("Could not connect to LCD. COM port: {}", com_port));
    }
    let display = display.unwrap();

    // Sistem tepsisi simgesi ("Ayarlar" -> configure.exe acar, "Cikis"
    // -> ekrani kapatip programi sonlandirir). Basarisiz olursa program
    // yine de calismaya devam eder (tray sadece bir kolaylik).
    let app_tray = match tray::AppTray::new() {
        Ok(t) => Some(t),
        Err(e) => {
            log::warn!("Tray icon could not be created (continuing): {}", e);
            None
        }
    };

    // Stats: arka plan thread'inde surekli guncellenir
    let stats = Arc::new(Mutex::new(SystemStats::new(
        cfg.config.eth.clone(),
        cfg.config.wlo.clone(),
    )));
    let stats_bg = stats.clone();
    thread::spawn(move || {
        loop {
            // ONEMLI: yavas olabilecek sensor okumalari (WMI sorgulari,
            // LHM subprocess cagrisi - saniyeler surebilir) mutex
            // KILITLI DEGILKEN yapilir. Aksi halde ana render dongusu
            // "stats.lock()" beklerken donuyor ve ekran real-time
            // gitmiyormus gibi goruniyordu.
            SystemStats::refresh_sensors_unlocked();

            // Hizli/lokal sysinfo yenilemesi icin kisa sureligine kilitle.
            if let Ok(mut s) = stats_bg.lock() {
                s.refresh();
            }
            thread::sleep(Duration::from_millis(200));
        }
    });

    // Hava durumu: TAMAMEN AYRI bir arka plan thread'i. Ag istegi
    // saniyeler surebilir - stats mutex'ine hic dokunmuyoruz (lhm.rs'deki
    // subprocess kilitlenme sorununun aynisina dusmemek icin). Sonuc
    // weather.rs icindeki kendi cache'inde tutulur, render dongusu
    // sadece bu cache'i okur.
    if let (Some(lat), Some(lon)) = (cfg.config.weather_latitude, cfg.config.weather_longitude) {
        let units = cfg.config.weather_units.clone().unwrap_or_else(|| "metric".to_string());
        log::info!("Starting weather thread: lat={}, lon={}, units={}", lat, lon, units);
        thread::spawn(move || {
            loop {
                weather::get(lat, lon, &units);
                thread::sleep(Duration::from_secs(300));
            }
        });
    } else {
        log::warn!("WEATHER_LATITUDE/WEATHER_LONGITUDE not set in config.yaml, weather will not be shown");
    }

    log::info!("Mini System Monitor started (theme: {})", theme_name);

    // Ilk frame: ekrani temizlemek icin arka plan resmini LCD'ye gonder.
    //
    // ONEMLI: Bunu TEK BUYUK 320x480 komutla yapmiyoruz. Bu ekranlarin
    // cogunda (Turing 3.5" tipi USB LCD'ler) tek bir cizim komutuyla
    // kabul edilebilecek bolge boyutunun bir siniri var; devasa bir
    // bolge (307.200 byte) gonderilince komut ya zaman asimina ugruyor
    // ya da cihaz tarafindan sessizce yok sayiliyor - yani ekranda
    // hicbir sey degismiyor, eski goruntu oldugu gibi kaliyor. Kodun
    // geri kalaninda (metin/grafik guncellemeleri) hep KUCUK bolgeler
    // gonderildigi icin bu sorun daha once hic ortaya cikmamisti.
    //
    // NOT: display.reset() komutu KASITLI OLARAK kullanilmiyor - bu
    // komut cihazi donanimsal olarak resetleyip USB'yi yeniden
    // enumerate ediyor, bu da o an acik olan seri port handle'ini
    // geçersiz kiliyor ve sonraki TUM yazmalar "device does not
    // recognize the command" hatasi ile basarisiz oluyor.
    //
    // Cozum: arka plani, tipki diger elemanlar gibi, kucuk yatay
    // seritler halinde gonderiyoruz - bu sadece pikselleri gercek
    // arka plan resmiyle "boyayarak" ekranin uzerine yazar, cihazi
    // resetlemez.
    //
    // NOT2: Bu islem SADECE program baslarken bir kere yapilir - normal
    // calisma sirasinda hala sadece degisen kucuk bolgeler gonderiliyor,
    // performans etkilenmez.
    {
        let mut debug_img = image::RgbImage::new(WIDTH, HEIGHT);
        for py in 0..HEIGHT {
            for px in 0..WIDTH {
                debug_img.put_pixel(px, py, *theme_renderer.bg.get_pixel(px, py));
            }
        }
        let _ = debug_img.save("debug_output.png");

        const BAND_HEIGHT: u32 = 32;
        let mut y = 0u32;
        while y < HEIGHT {
            let h = BAND_HEIGHT.min(HEIGHT - y);
            let mut band = image::RgbImage::new(WIDTH, h);
            for py in 0..h {
                for px in 0..WIDTH {
                    band.put_pixel(px, py, *theme_renderer.bg.get_pixel(px, y + py));
                }
            }
            if let Err(e) = display.display_image(&band, 0, y as u16) {
                log::error!("First background band could not be sent (y={}): {}", y, e);
            }
            y += BAND_HEIGHT;
        }
    }

    // Per-element zamanlayici — baslangic degerleri KAYDIRILMIS (staggered)
    // Boylece serial kuyruguna ayni anda patlamaz
    let mut last_cpu_pct = Instant::now();
    let mut last_cpu_freq = Instant::now() + Duration::from_millis(1000);
    let mut last_cpu_temp = Instant::now() + Duration::from_millis(2000);
    let mut last_gpu = Instant::now() + Duration::from_millis(300);
    let mut last_gpu_temp = Instant::now() + Duration::from_millis(3000);
    let mut last_mem = Instant::now() + Duration::from_millis(1500);
    let mut last_disk = Instant::now() + Duration::from_millis(2500);
    let mut last_date = Instant::now() + Duration::from_millis(600);
    let mut last_uptime = Instant::now() + Duration::from_millis(900);
    let mut last_net = Instant::now() + Duration::from_millis(1800);
    let mut last_weather = Instant::now()
        .checked_sub(Duration::from_secs(55))
        .unwrap_or_else(Instant::now);

    loop {
        let now = Instant::now();

        // Tepsi menusu ("Cikis" secildiyse donguden cik). Win32 mesaj
        // pompalamasi da burada yapiliyor - tray simgesinin calismasi
        // icin gerekli, 10ms'lik dongude neredeyse ucretsiz.
        if let Some(t) = &app_tray {
            if t.poll(&display) {
                break;
            }
        }

        // Anlik stat degerlerini oku (Mutex'ten, cok hizli - sadece
        // sysinfo/cache okumasi, agir sensor islemleri artik burada yok)
        let (cpu_freq, cpu_temp,
             gpu_load, gpu_mem, gpu_mem_total, gpu_freq, gpu_temp,
             mem_pct, mem_used, mem_total,
             disk_pct, disk_used, disk_total,
             dt_str, d_str, up_secs) = {
            let s = stats.lock().unwrap();
            (
                s.cpu_frequency_mhz(),
                s.cpu_temperature(),
                s.gpu_load(),
                s.gpu_memory_used_mb(),
                s.gpu_memory_total_mb(),
                s.gpu_frequency_mhz(),
                s.gpu_temperature(),
                s.memory_percent(),
                s.memory_used_mb(),
                s.memory_total_mb(),
                s.disk_percent(),
                s.disk_used_gb(),
                s.disk_total_gb(),
                s.datetime_str(),
                s.date_str(),
                s.uptime_seconds(),
            )
        };

        // CPU% (1sn) - okuma VE history push'u da burada, tam olarak
        // 1 saniyede bir yapiliyor (eskiden her 10ms'de bir push
        // ediliyordu ve 60 ornekli history ~600ms'de dolup tasiyordu)
        if now - last_cpu_pct >= Duration::from_secs(1) {
            let (cpu_pct, cpu_hist) = {
                let mut s = stats.lock().unwrap();
                let pct = s.cpu_percent_now();
                s.push_cpu_history(pct);
                (pct, s.cpu_history().to_vec())
            };
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{:.0}%", cpu_pct), &["STATS", "CPU", "PERCENTAGE", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_graph_bar(cpu_pct, &["STATS", "CPU", "PERCENTAGE", "GRAPH"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_line_graph(&cpu_hist, &["STATS", "CPU", "PERCENTAGE", "LINE_GRAPH"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            last_cpu_pct = now;
        }

        // CPU FREQ (5sn)
        if now - last_cpu_freq >= Duration::from_secs(5) {
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{:.2}GHz", cpu_freq as f32 / 1000.0), &["STATS", "CPU", "FREQUENCY", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            last_cpu_freq = now;
        }

        // CPU TEMP (5sn)
        if now - last_cpu_temp >= Duration::from_secs(5) {
            let s = if cpu_temp > 0.0 { format!("{:.0}°C", cpu_temp) } else { "--".to_string() };
            if let Some((img, x, y)) = theme_renderer.render_text(&s, &["STATS", "CPU", "TEMPERATURE", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            last_cpu_temp = now;
        }

        // GPU (1sn)
        if now - last_gpu >= Duration::from_secs(1) {
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{:.0}%", gpu_load), &["STATS", "GPU", "PERCENTAGE", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_graph_bar(gpu_load, &["STATS", "GPU", "PERCENTAGE", "GRAPH"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{:.0}M", gpu_mem), &["STATS", "GPU", "MEMORY", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            let gpu_mem_pct = if gpu_mem_total > 0.0 { (gpu_mem / gpu_mem_total) * 100.0 } else { 0.0 };
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{:.0}%", gpu_mem_pct), &["STATS", "GPU", "MEMORY_PERCENT", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_graph_bar(gpu_mem_pct, &["STATS", "GPU", "MEMORY_PERCENT", "GRAPH"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{:.0}MHz", gpu_freq), &["STATS", "GPU", "FREQUENCY", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            last_gpu = now;
        }

        // GPU TEMP (5sn)
        if now - last_gpu_temp >= Duration::from_secs(5) {
            let s = if gpu_temp > 0.0 { format!("{:.0}°C", gpu_temp) } else { "--".to_string() };
            if let Some((img, x, y)) = theme_renderer.render_text(&s, &["STATS", "GPU", "TEMPERATURE", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            last_gpu_temp = now;
        }

        // MEMORY (5sn)
        if now - last_mem >= Duration::from_secs(5) {
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{:.1}%", mem_pct), &["STATS", "MEMORY", "VIRTUAL", "PERCENT_TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{}M", mem_used), &["STATS", "MEMORY", "VIRTUAL", "USED"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            let mem_free = mem_total.saturating_sub(mem_used);
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{}M", mem_free), &["STATS", "MEMORY", "VIRTUAL", "FREE"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{}M", mem_total), &["STATS", "MEMORY", "VIRTUAL", "TOTAL"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_graph_bar(mem_pct, &["STATS", "MEMORY", "VIRTUAL", "GRAPH"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            last_mem = now;
        }

        // DISK (10sn)
        if now - last_disk >= Duration::from_secs(10) {
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{:.1}%", disk_pct), &["STATS", "DISK", "USED", "PERCENT_TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{}G", disk_used), &["STATS", "DISK", "USED", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            let disk_free = disk_total.saturating_sub(disk_used);
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{}G", disk_free), &["STATS", "DISK", "FREE"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_text(&format!("{}G", disk_total), &["STATS", "DISK", "TOTAL", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_graph_bar(disk_pct, &["STATS", "DISK", "USED", "GRAPH"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            last_disk = now;
        }

        // DATE (1sn)
        if now - last_date >= Duration::from_secs(1) {
            if let Some((img, x, y)) = theme_renderer.render_text(&dt_str, &["STATS", "DATE", "HOUR", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_text(&d_str, &["STATS", "DATE", "DAY", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            last_date = now;
        }

        // UPTIME (1sn)
        if now - last_uptime >= Duration::from_secs(1) {
            let s = format!("{:02}:{:02}:{:02}", up_secs / 3600, (up_secs % 3600) / 60, up_secs % 60);
            if let Some((img, x, y)) = theme_renderer.render_text(&s, &["STATS", "UPTIME", "FORMATTED", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            last_uptime = now;
        }

        // NET yukleme/indirme (1sn) - theme.yaml'de sadece WLO metin
        // bloklari tanimli; config.yaml'da ETH/WLO belirtilmemisse
        // (bos string) TUM arayuzlerin toplami gosterilir.
        if now - last_net >= Duration::from_secs(1) {
            let (up_kbps, down_kbps) = {
                let s = stats.lock().unwrap();
                (s.net_upload_kbps(), s.net_download_kbps())
            };
            let up_str = format!("UP {:.0} KB/s", up_kbps);
            let down_str = format!("DN {:.0} KB/s", down_kbps);
            if let Some((img, x, y)) = theme_renderer.render_text(&up_str, &["STATS", "NET", "WLO", "UPLOAD", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            if let Some((img, x, y)) = theme_renderer.render_text(&down_str, &["STATS", "NET", "WLO", "DOWNLOAD", "TEXT"]) {
                let _ = display.display_image(&img, x as u16, y as u16);
            }
            last_net = now;
        }

        // HAVA DURUMU (theme.yaml INTERVAL: 300) - burada AG ISTEGI
        // YAPILMAZ, sadece weather.rs'nin kendi arka plan thread'inin
        // doldurdugu cache okunur (get_cached asla bloklamaz).
        if now - last_weather >= Duration::from_secs(60) {
            if let Some(w) = weather::get_cached() {
                let temp_str = format!("{:.0}°{}", w.temperature, w.unit);
                if let Some((img, x, y)) = theme_renderer.render_text(&temp_str, &["STATS", "WEATHER", "TEMPERATURE", "TEXT"]) {
                    let _ = display.display_image(&img, x as u16, y as u16);
                }
                if let Some((img, x, y)) = theme_renderer.render_text(&w.description, &["STATS", "WEATHER", "WEATHER_DESCRIPTION", "TEXT"]) {
                    let _ = display.display_image(&img, x as u16, y as u16);
                }
                // Weather icon
                let icon_name = weather::wmo_icon_name(w.weather_code);
                if let Some(icon) = load_weather_icon(icon_name, &theme_renderer.theme.path) {
                    if let Some((img, x, y)) = theme_renderer.render_icon_img(&icon, &["STATS", "WEATHER", "WEATHER_DESCRIPTION", "ICON"]) {
                        let _ = display.display_image(&img, x as u16, y as u16);
                    }
                }
                let humidity_str = format!("{:.0}%", w.humidity_pct);
                if let Some((img, x, y)) = theme_renderer.render_text(&humidity_str, &["STATS", "WEATHER", "HUMIDITY", "TEXT"]) {
                    let _ = display.display_image(&img, x as u16, y as u16);
                }
                let felt_str = format!("{:.0}°{}", w.temperature_felt, w.unit);
                if let Some((img, x, y)) = theme_renderer.render_text(&felt_str, &["STATS", "WEATHER", "TEMPERATURE_FELT", "TEXT"]) {
                    let _ = display.display_image(&img, x as u16, y as u16);
                }
            }
            last_weather = now;
        }

        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}
