//! Pochette du titre en cours (grande taille, recadrée en carré) et couleurs dominantes
//! pour le fond de l'écran « Lecture en cours ».

use eframe::egui::{Color32, ColorImage};
use image::imageops::FilterType;

pub struct Art {
    pub image: ColorImage,
    /// Quatre teintes sombres tirées des quarts de l'image (fond « liquide »).
    pub colors: [Color32; 4],
    /// Teinte claire et saturée (halo, ligne de paroles).
    pub light: Color32,
}

const SIZE: u32 = 512;

fn download(url: &str) -> Option<image::DynamicImage> {
    let resp = ehttp::fetch_blocking(&ehttp::Request::get(url)).ok()?;
    if !resp.ok {
        return None;
    }
    image::load_from_memory(&resp.bytes).ok()
}

/// Bloquant : à appeler depuis un fil secondaire. `thumb` = URL mqdefault de la vidéo.
pub fn fetch(thumb: &str) -> Option<Art> {
    // maxresdefault (1280×720) n'existe pas pour toutes les vidéos ; hqdefault (480×360)
    // a des bandes noires en haut et en bas, qu'on retire en ne gardant que le 16:9.
    let img = download(&thumb.replace("mqdefault", "maxresdefault"))
        .or_else(|| download(&thumb.replace("mqdefault", "hqdefault")).map(|i| {
            let (w, h) = (i.width(), i.height());
            let h169 = w * 9 / 16;
            i.crop_imm(0, (h.saturating_sub(h169)) / 2, w, h169.min(h))
        }))
        .or_else(|| download(thumb))?;

    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    let square = img.crop_imm((w - side) / 2, (h - side) / 2, side, side).resize_exact(SIZE, SIZE, FilterType::Triangle);
    let rgba = square.to_rgba8();

    let h = SIZE / 2;
    let colors = [region_color(&rgba, 0, 0, 120.0), region_color(&rgba, h, 0, 120.0), region_color(&rgba, 0, h, 120.0), region_color(&rgba, h, h, 120.0)];
    // Teinte claire : le quart le plus coloré, éclairci.
    let light = (0..4)
        .map(|i| region_color(&rgba, (i % 2) * h, (i / 2) * h, 235.0))
        .max_by_key(|c| c.r().max(c.g()).max(c.b()) as i32 - c.r().min(c.g()).min(c.b()) as i32)
        .unwrap_or(Color32::from_rgb(0xcd, 0xb4, 0xff));
    let image = ColorImage::from_rgba_unmultiplied([SIZE as usize, SIZE as usize], rgba.as_raw());
    Some(Art { image, colors, light })
}

/// Moyenne d'un quart de l'image, saturée puis ramenée à la luminosité `peak`.
fn region_color(img: &image::RgbaImage, x0: u32, y0: u32, peak: f32) -> Color32 {
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for y in (y0..y0 + SIZE / 2).step_by(4) {
        for x in (x0..x0 + SIZE / 2).step_by(4) {
            let p = img.get_pixel(x, y);
            r += p[0] as u64;
            g += p[1] as u64;
            b += p[2] as u64;
            n += 1;
        }
    }
    let (r, g, b) = ((r / n) as f32, (g / n) as f32, (b / n) as f32);
    // Accentue l'écart à la moyenne (saturation), puis ramène vers une luminosité sombre.
    let mean = (r + g + b) / 3.0;
    let sat = |c: f32| (mean + (c - mean) * 1.6).clamp(0.0, 255.0);
    let (r, g, b) = (sat(r), sat(g), sat(b));
    let max = r.max(g).max(b).max(1.0);
    let k = peak / max;
    Color32::from_rgb((r * k) as u8, (g * k) as u8, (b * k) as u8)
}
