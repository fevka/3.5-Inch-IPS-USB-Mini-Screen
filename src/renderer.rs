use ab_glyph::{Font, FontArc, PxScale, Point, ScaleFont};
use image::{Rgb, RgbImage, RgbaImage};
use std::collections::HashMap;

pub struct Renderer {
    default_font: FontArc,
    pub fonts: HashMap<String, FontArc>,
}

impl Renderer {
    pub fn new() -> Self {
        let font_data: &[u8] = include_bytes!("../res/RobotoMono-Regular.ttf");
        let font = FontArc::try_from_slice(font_data).expect("font");
        let mut fonts = HashMap::new();
        fonts.insert("default".to_string(), font.clone());
        Renderer {
            default_font: font,
            fonts,
        }
    }

    pub fn load_font(&mut self, path: &str) -> Option<FontArc> {
        let font_path = crate::config::resolve_theme_path(path);
        if let Ok(data) = std::fs::read(&font_path) {
            let leaked: &'static [u8] = Box::leak(data.into_boxed_slice());
            if let Ok(font) = FontArc::try_from_slice(leaked) {
                self.fonts.insert(path.to_string(), font.clone());
                return Some(font);
            }
        }
        None
    }

    fn get_font(&self, name: Option<&str>) -> &FontArc {
        if let Some(n) = name {
            if let Some(f) = self.fonts.get(n) {
                return f;
            }
        }
        &self.default_font
    }

    pub fn draw_text(
        &self,
        img: &mut RgbImage,
        text: &str,
        x: i32,
        y: i32,
        size: f32,
        color: Rgb<u8>,
    ) {
        self.draw_text_font(img, text, x, y, size, color, None);
    }

    pub fn measure_text(&self, text: &str, size: f32, font_name: Option<&str>) -> (u32, u32) {
        let font = self.get_font(font_name);
        let px_scale = PxScale::from(size);
        let scaled = font.as_scaled(px_scale);
        let h = scaled.height() as u32;
        let mut w = 0.0f32;
        for c in text.chars() {
            w += scaled.h_advance(font.glyph_id(c));
        }
        (w.ceil() as u32 + 2, h + 2)
    }

    pub fn draw_text_font(
        &self,
        img: &mut RgbImage,
        text: &str,
        x: i32,
        y: i32,
        size: f32,
        color: Rgb<u8>,
        font_name: Option<&str>,
    ) {
        let font = self.get_font(font_name);
        let px_scale = PxScale::from(size);
        let scaled_font = font.as_scaled(px_scale);
        // ONEMLI: baseline'i kutunun ALT KENARINA (y+height) degil,
        // y+ascent'e yerlestiriyoruz. Eskiden baseline tam alt kenardaydi,
        // bu da "g, y, p, ç, ş" gibi alt cikintisi (descender) olan
        // harflerin kutunun disina tasip KIRPILMASINA sebep oluyordu -
        // kutunun altinda descender icin hic yer birakilmiyordu.
        let ascent = scaled_font.ascent();

        let mut px = x as f32;
        for c in text.chars() {
            let glyph_id = font.glyph_id(c as char);
            let mut glyph = glyph_id.with_scale(px_scale);
            glyph.position = Point {
                x: px,
                y: y as f32 + ascent,
            };
            if let Some(outline) = font.outline_glyph(glyph) {
                let bb = outline.px_bounds();
                outline.draw(|gx, gy, coverage| {
                    let ix = bb.min.x as i32 + gx as i32;
                    let iy = bb.min.y as i32 + gy as i32;
                    if ix >= 0 && iy >= 0 && ix < img.width() as i32 && iy < img.height() as i32
                    {
                        // ESKIDEN: v > 0.3 ise TAM renk, degilse HIC piksel -
                        // sert bir esik degeri, anti-aliasing yoktu. Bu,
                        // ekranda kaba/pikselli/"kotu" gorunen yazilarin
                        // asil sebebiydi. Simdi kismi kapsama (coverage)
                        // degerine gore mevcut arka plan pikseliyle
                        // ORANTILI KARISTIRIYORUZ (alpha blend) - kenarlar
                        // pürüzsüz cikar.
                        let alpha = coverage.clamp(0.0, 1.0);
                        if alpha > 0.02 {
                            let bg = *img.get_pixel(ix as u32, iy as u32);
                            let blended = Rgb([
                                (color[0] as f32 * alpha + bg[0] as f32 * (1.0 - alpha)).round() as u8,
                                (color[1] as f32 * alpha + bg[1] as f32 * (1.0 - alpha)).round() as u8,
                                (color[2] as f32 * alpha + bg[2] as f32 * (1.0 - alpha)).round() as u8,
                            ]);
                            img.put_pixel(ix as u32, iy as u32, blended);
                        }
                    }
                });
            }
            px += scaled_font.h_advance(glyph_id);
        }
    }

    /// Draws a progress bar the same way the real device does: only the
    /// FILLED portion gets painted with `fg`. The unfilled remainder is left
    /// completely untouched, so whatever the background image already shows
    /// there (which may not be a flat color) stays visible - painting a flat
    /// fallback color over it would show as a false, disconnected-looking
    /// patch that doesn't match the real screen.
    pub fn draw_progress_bar(
        &self,
        img: &mut RgbImage,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pct: f32,
        fg: Rgb<u8>,
    ) {
        let fill = ((pct / 100.0).clamp(0.0, 1.0) * w as f32) as i32;
        for py in y..y + h as i32 {
            for px in x..x + fill {
                if px >= 0 && py >= 0 && px < img.width() as i32 && py < img.height() as i32 {
                    img.put_pixel(px as u32, py as u32, fg);
                }
            }
        }
    }

    pub fn draw_line_graph(
        &self,
        img: &mut RgbImage,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        data: &[f32],
        min_val: f32,
        max_val: f32,
        line_color: Rgb<u8>,
    ) {
        if data.len() < 2 {
            return;
        }
        let range = (max_val - min_val).max(1.0);
        let step = w as f32 / (data.len() - 1) as f32;
        let mut prev_x = x as i32;
        let mut prev_y = y as i32 + h as i32
            - ((data[0].clamp(min_val, max_val) - min_val) / range * h as f32) as i32;

        for i in 1..data.len() {
            let cx = x + (i as f32 * step) as i32;
            let cy = y as i32 + h as i32
                - ((data[i].clamp(min_val, max_val) - min_val) / range * h as f32) as i32;

            let (x0, x1) = (prev_x.min(cx), prev_x.max(cx));
            let (y0, y1) = (prev_y.min(cy), prev_y.max(cy));

            for px in x0..=x1 {
                for py in y0..=y1 {
                    let dx = x1 - x0;
                    let dy = y1 - y0;
                    let dist = if dx == 0 {
                        (py - y0).abs() as f32
                    } else if dy == 0 {
                        (px - x0).abs() as f32
                    } else {
                        let t = if dx > dy {
                            (px - x0) as f32 / dx as f32
                        } else {
                            (py - y0) as f32 / dy as f32
                        };
                        let lx = x0 as f32 + t * dx as f32;
                        let ly = y0 as f32 + t * dy as f32;
                        ((px as f32 - lx).powi(2) + (py as f32 - ly).powi(2)).sqrt()
                    };
                    if dist < 2.0
                        && px >= 0
                        && py >= 0
                        && px < img.width() as i32
                        && py < img.height() as i32
                    {
                        img.put_pixel(px as u32, py as u32, line_color);
                    }
                }
            }
            prev_x = cx;
            prev_y = cy;
        }
    }

    pub fn draw_image(
        &self,
        img: &mut RgbImage,
        overlay: &RgbImage,
        x: i32,
        y: i32,
    ) {
        for oy in 0..overlay.height() {
            for ox in 0..overlay.width() {
                let dx = x + ox as i32;
                let dy = y + oy as i32;
                if dx >= 0 && dy >= 0 && dx < img.width() as i32 && dy < img.height() as i32 {
                    let pixel = overlay.get_pixel(ox, oy);
                    if pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0 {
                        img.put_pixel(dx as u32, dy as u32, *pixel);
                    }
                }
            }
        }
    }

    /// Draws an RGBA icon onto the RGB image with alpha blending
    pub fn draw_icon(
        &self,
        img: &mut RgbImage,
        icon: &RgbaImage,
        x: i32,
        y: i32,
    ) {
        for oy in 0..icon.height() {
            for ox in 0..icon.width() {
                let dx = x + ox as i32;
                let dy = y + oy as i32;
                if dx >= 0 && dy >= 0 && dx < img.width() as i32 && dy < img.height() as i32 {
                    let pixel = icon.get_pixel(ox, oy);
                    let alpha = pixel[3] as f32 / 255.0;
                    if alpha > 0.02 {
                        let bg = *img.get_pixel(dx as u32, dy as u32);
                        let blended = Rgb([
                            (pixel[0] as f32 * alpha + bg[0] as f32 * (1.0 - alpha)).round() as u8,
                            (pixel[1] as f32 * alpha + bg[1] as f32 * (1.0 - alpha)).round() as u8,
                            (pixel[2] as f32 * alpha + bg[2] as f32 * (1.0 - alpha)).round() as u8,
                        ]);
                        img.put_pixel(dx as u32, dy as u32, blended);
                    }
                }
            }
        }
    }
}
