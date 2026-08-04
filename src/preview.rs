// Theme preview rendering, shared by anything that needs to turn a
// theme.yaml + background image into a rendered PNG: the monitor itself
// (indirectly, via the same Renderer) and the configure app's live
// preview. This code has ZERO UI-framework dependency (no egui, no
// Tauri) - it only knows about serde_yaml + image + the Renderer type,
// so it can be called from any front end.

use crate::config;
use crate::renderer::Renderer;
use image::{ImageEncoder, RgbaImage, Rgb, RgbImage};
use serde::Serialize;
use std::path::Path;

/// Walks a dotted/segmented path (e.g. ["STATS","CPU","PERCENTAGE","TEXT"])
/// down a parsed YAML tree.
pub fn node_at<'a>(yaml: &'a serde_yaml::Value, path: &[String]) -> Option<&'a serde_yaml::Value> {
    let mut cur = yaml;
    for p in path {
        cur = cur.get(p.as_str())?;
    }
    Some(cur)
}

/// Finds every node in the theme tree that has both "X" and "Y" fields,
/// regardless of depth - this automatically catches both static_text
/// entries and every STATS.*.TEXT/GRAPH block without hardcoding paths.
pub fn scan_elements(value: &serde_yaml::Value, path: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    if let serde_yaml::Value::Mapping(map) = value {
        let has_xy = map.contains_key(&serde_yaml::Value::String("X".to_string()))
            && map.contains_key(&serde_yaml::Value::String("Y".to_string()));
        if has_xy {
            out.push(path.clone());
        }
        for (k, v) in map {
            if let serde_yaml::Value::String(key) = k {
                path.push(key.clone());
                scan_elements(v, path, out);
                path.pop();
            }
        }
    }
}

/// Finds every "FONT: ..." value anywhere in the theme tree.
pub fn collect_font_names(value: &serde_yaml::Value, out: &mut Vec<String>) {
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

/// Loads every font referenced by the theme, caching each under its
/// PLAIN name (as written in theme.yaml) so lookups at draw time match.
pub fn load_theme_fonts(renderer: &mut Renderer, theme_dir: &Path, yaml: &serde_yaml::Value) {
    let mut names = Vec::new();
    collect_font_names(yaml, &mut names);
    for font in names {
        if renderer.fonts.contains_key(&font) {
            continue;
        }
        let candidate1 = theme_dir.join(&font);
        if let Some(f) = renderer.load_font(&candidate1.to_string_lossy()) {
            renderer.fonts.insert(font.clone(), f);
            continue;
        }
        let candidate2 = config::resolve_path(&format!("res/fonts/{}", font));
        if let Some(f) = renderer.load_font(&candidate2.to_string_lossy()) {
            renderer.fonts.insert(font, f);
        }
    }
}

/// Recursively finds every .ttf under `dir`, returned as paths relative
/// to `root` with forward slashes (matching theme.yaml's FONT convention).
fn scan_fonts_dir(root: &Path, dir: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_fonts_dir(root, &path, out);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ttf"))
                .unwrap_or(false)
            {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
}

/// Every .ttf available to a theme: the theme's own folder plus the
/// shared res/fonts directory. Useful for building a font picker.
pub fn scan_available_fonts(theme_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    scan_fonts_dir(theme_dir, theme_dir, &mut out);
    let res_fonts = config::resolve_path("res/fonts");
    if res_fonts.exists() {
        scan_fonts_dir(&res_fonts, &res_fonts, &mut out);
    }
    out.sort();
    out.dedup();
    out
}

pub fn parse_rgb_csv(s: &str) -> Rgb<u8> {
    let parts: Vec<i64> = s.split(',').filter_map(|p| p.trim().parse::<i64>().ok()).collect();
    if parts.len() == 3 {
        Rgb([parts[0].clamp(0, 255) as u8, parts[1].clamp(0, 255) as u8, parts[2].clamp(0, 255) as u8])
    } else {
        Rgb([255, 255, 255])
    }
}

pub fn rgb_to_csv(rgb: [u8; 3]) -> String {
    format!("{}, {}, {}", rgb[0], rgb[1], rgb[2])
}

/// Representative sample text for STATS fields that don't have a
/// literal TEXT value (their real content comes from live sensors at
/// runtime) - lets the preview look realistic without wiring up actual
/// sensor polling inside the settings app.
pub fn placeholder_text(path: &[String]) -> String {
    let joined = path.join(".").to_uppercase();
    if joined.contains("HOUR") {
        chrono::Local::now().format("%H:%M").to_string()
    } else if joined.contains("DAY") {
        chrono::Local::now().format("%a %d %b").to_string()
    } else if joined.contains("UPTIME") || joined.contains("FORMATTED") {
        "02:14:33".to_string()
    } else if joined.contains("TEMPERATURE_FELT") {
        "54°C".to_string()
    } else if joined.contains("TEMPERATURE") {
        "56°C".to_string()
    } else if joined.contains("FREQUENCY") || joined.contains("CLOCK") {
        "3200 MHz".to_string()
    } else if joined.contains("UPLOAD") {
        "UP 512 KB/s".to_string()
    } else if joined.contains("DOWNLOAD") {
        "DN 2.1 MB/s".to_string()
    } else if joined.contains("HUMIDITY") {
        "48%".to_string()
    } else if joined.contains("USED") {
        "8.2 GB".to_string()
    } else if joined.contains("PERCENT") || joined.contains("LOAD") || joined.contains("VIRTUAL") {
        "42%".to_string()
    } else if joined.contains("WEATHER") {
        "21°".to_string()
    } else {
        String::new()
    }
}

pub fn load_bg_image(path: &Path) -> Option<RgbImage> {
    image::open(path).ok().map(|i| i.to_rgb8())
}

fn load_dummy_weather_icon() -> Option<RgbaImage> {
    let paths = [
        std::path::PathBuf::from("res/icons/weather/sunny.png"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("res/icons/weather/sunny.png"),
    ];
    for p in &paths {
        if let Ok(img) = image::open(p) {
            return Some(img.to_rgba8());
        }
    }
    None
}

/// Renders the theme (background + every visible element, using real
/// fonts/colors/positions and representative sample values) into a
/// single image - the same drawing code path the real monitor uses.
pub fn render_theme_preview(
    renderer: &mut Renderer,
    bg: &RgbImage,
    yaml: &serde_yaml::Value,
    elements: &[Vec<String>],
) -> RgbImage {
    let mut img = bg.clone();
    for path in elements {
        let Some(node) = node_at(yaml, path) else { continue };
        let show = node.get("SHOW").and_then(|v| v.as_bool()).unwrap_or(true);
        if !show {
            continue;
        }
        let x = node.get("X").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let y = node.get("Y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        if node.get("WIDTH").is_some() && node.get("HEIGHT").is_some() && node.get("PATH").is_none() {
            let w = node.get("WIDTH").and_then(|v| v.as_i64()).unwrap_or(80) as u32;
            let h = node.get("HEIGHT").and_then(|v| v.as_i64()).unwrap_or(10) as u32;
            // Check if this is an icon (has WIDTH/HEIGHT but no BAR_COLOR)
            if node.get("BAR_COLOR").is_some() {
                let bar_color = node
                    .get("BAR_COLOR")
                    .and_then(|v| v.as_str())
                    .map(parse_rgb_csv)
                    .unwrap_or(Rgb([180, 80, 255]));
                renderer.draw_progress_bar(&mut img, x, y, w, h, 100.0, bar_color);
            } else {
                // Icon: try loading dummy weather icon, fallback to border
                if let Some(icon) = load_dummy_weather_icon() {
                    let resized = image::imageops::resize(&icon, w, h, image::imageops::FilterType::Lanczos3);
                    for py in 0..h {
                        for px in 0..w {
                            let ix = (x + px as i32).max(0) as u32;
                            let iy = (y + py as i32).max(0) as u32;
                            if ix >= img.width() || iy >= img.height() { continue; }
                            let p = resized.get_pixel(px, py);
                            if p[3] > 0 {
                                let bg = img.get_pixel(ix, iy);
                                let a = p[3] as f32 / 255.0;
                                let r = (p[0] as f32 * a + bg[0] as f32 * (1.0 - a)) as u8;
                                let g = (p[1] as f32 * a + bg[1] as f32 * (1.0 - a)) as u8;
                                let b = (p[2] as f32 * a + bg[2] as f32 * (1.0 - a)) as u8;
                                img.put_pixel(ix, iy, Rgb([r, g, b]));
                            }
                        }
                    }
                } else {
                    let accent = Rgb([0, 191, 165]);
                    for py in 0..h {
                        for px in 0..w {
                            if px == 0 || px == w - 1 || py == 0 || py == h - 1 {
                                let dx = (x + px as i32).max(0) as u32;
                                let dy = (y + py as i32).max(0) as u32;
                                if dx < img.width() && dy < img.height() {
                                    img.put_pixel(dx, dy, accent);
                                }
                            }
                        }
                    }
                }
            }
            continue;
        }

        let font = node.get("FONT").and_then(|v| v.as_str()).map(|s| s.to_string());
        let font_size = node.get("FONT_SIZE").and_then(|v| v.as_f64()).unwrap_or(14.0) as f32;
        let color = node
            .get("FONT_COLOR")
            .and_then(|v| v.as_str())
            .map(parse_rgb_csv)
            .unwrap_or(Rgb([255, 255, 255]));
        let anchor = node.get("ANCHOR").and_then(|v| v.as_str()).unwrap_or("lt").to_string();
        let text = node
            .get("TEXT")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| placeholder_text(path));

        if !text.is_empty() {
            let (tw, th) = renderer.measure_text(&text, font_size, font.as_deref());
            let mut chars = anchor.chars();
            let ha = chars.next().unwrap_or('l');
            let va = chars.next().unwrap_or('t');
            let dx = match ha {
                'r' => x - tw as i32,
                'm' => x - (tw as i32) / 2,
                _ => x,
            };
            let dy = match va {
                'b' => y - th as i32,
                'm' => y - (th as i32) / 2,
                _ => y,
            };
            renderer.draw_text_font(&mut img, &text, dx, dy, font_size, color, font.as_deref());
        }
    }
    img
}

/// One selectable/draggable element's on-screen geometry, in the
/// theme's native pixel space (same space as X/Y/WIDTH/HEIGHT in the
/// yaml). The frontend scales these to whatever size the preview
/// image is actually rendered at.
#[derive(Serialize, Clone)]
pub struct ElementBox {
    pub path: Vec<String>,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub hidden: bool,
}

#[derive(Serialize, Clone)]
pub struct LayoutInfo {
    pub width: u32,
    pub height: u32,
    pub boxes: Vec<ElementBox>,
}

/// Same geometry logic as `render_theme_preview`, but returns bounding
/// boxes instead of drawing pixels - this is what powers "click/drag
/// the element directly on the preview" in the theme editor.
pub fn compute_layout(theme_dir: &Path, yaml_text: &str) -> Result<LayoutInfo, String> {
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(yaml_text).map_err(|e| format!("YAML error: {e}"))?;

    let mut elements = Vec::new();
    let mut path_buf = Vec::new();
    scan_elements(&yaml, &mut path_buf, &mut elements);

    let bg_rel = node_at(&yaml, &["static_images".to_string(), "BACKGROUND".to_string()])
        .and_then(|n| n.get("PATH"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "theme has no static_images.BACKGROUND.PATH".to_string())?;
    let bg = load_bg_image(&theme_dir.join(&bg_rel))
        .ok_or_else(|| format!("could not load background image: {bg_rel}"))?;

    let mut renderer = Renderer::new();
    load_theme_fonts(&mut renderer, theme_dir, &yaml);

    let mut boxes = Vec::new();
    for path in &elements {
        let Some(node) = node_at(&yaml, path) else { continue };
        let hidden = !node.get("SHOW").and_then(|v| v.as_bool()).unwrap_or(true);
        let x = node.get("X").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let y = node.get("Y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        if node.get("WIDTH").is_some() && node.get("HEIGHT").is_some() && node.get("PATH").is_none() {
            let w = node.get("WIDTH").and_then(|v| v.as_i64()).unwrap_or(80) as u32;
            let h = node.get("HEIGHT").and_then(|v| v.as_i64()).unwrap_or(10) as u32;
            boxes.push(ElementBox { path: path.clone(), x, y, w, h, hidden });
            continue;
        }

        let font = node.get("FONT").and_then(|v| v.as_str()).map(|s| s.to_string());
        let font_size = node.get("FONT_SIZE").and_then(|v| v.as_f64()).unwrap_or(14.0) as f32;
        let anchor = node.get("ANCHOR").and_then(|v| v.as_str()).unwrap_or("lt").to_string();
        let text = node
            .get("TEXT")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| placeholder_text(path));

        let fallback = text.is_empty();
        let display_text = if fallback { "123" } else { &text };
        let (tw, th) = renderer.measure_text(display_text, font_size, font.as_deref());
        let mut chars = anchor.chars();
        let ha = chars.next().unwrap_or('l');
        let va = chars.next().unwrap_or('t');
        let dx = match ha {
            'r' => x - tw as i32,
            'm' => x - (tw as i32) / 2,
            _ => x,
        };
        let dy = match va {
            'b' => y - th as i32,
            'm' => y - (th as i32) / 2,
            _ => y,
        };
        boxes.push(ElementBox { path: path.clone(), x: dx, y: dy, w: tw.max(4), h: th.max(4), hidden });
    }

    Ok(LayoutInfo { width: bg.width(), height: bg.height(), boxes })
}

/// raw theme.yaml text, produce a rendered PNG (as bytes). Returns a
/// descriptive error string on invalid YAML or a missing background,
/// so the caller can surface it directly in a status bar.
pub fn render_preview_png(theme_dir: &Path, yaml_text: &str) -> Result<Vec<u8>, String> {
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(yaml_text).map_err(|e| format!("YAML error: {e}"))?;

    let mut elements = Vec::new();
    let mut path_buf = Vec::new();
    scan_elements(&yaml, &mut path_buf, &mut elements);

    let bg_rel = node_at(&yaml, &["static_images".to_string(), "BACKGROUND".to_string()])
        .and_then(|n| n.get("PATH"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "theme has no static_images.BACKGROUND.PATH".to_string())?;

    let bg = load_bg_image(&theme_dir.join(&bg_rel))
        .ok_or_else(|| format!("could not load background image: {bg_rel}"))?;

    let mut renderer = Renderer::new();
    load_theme_fonts(&mut renderer, theme_dir, &yaml);
    let rendered = render_theme_preview(&mut renderer, &bg, &yaml, &elements);

    let mut bytes: Vec<u8> = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            rendered.as_raw(),
            rendered.width(),
            rendered.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("PNG encode error: {e}"))?;
    Ok(bytes)
}
