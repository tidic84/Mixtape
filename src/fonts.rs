//! Polices de secours pour les écritures non latines (japonais, coréen, chinois, thaï,
//! devanagari). Les polices Windows correspondantes pèsent 5 à 20 Mo : elles ne sont
//! chargées qu'au premier texte qui en a besoin.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use eframe::egui::{self, epaint::text::{FontInsert, FontPriority, InsertFontFamily}, FontFamily};

/// (nom, fichiers candidats dans C:\Windows\Fonts, par ordre de préférence)
const JAPANESE: (&str, &[&str]) = ("jp", &["YuGothM.ttc", "meiryo.ttc", "msgothic.ttc"]);
const KOREAN: (&str, &[&str]) = ("ko", &["malgun.ttf", "gulim.ttc"]);
const CHINESE: (&str, &[&str]) = ("zh", &["msyh.ttc", "simsun.ttc"]);
const THAI: (&str, &[&str]) = ("th", &["LeelawUI.ttf", "tahoma.ttf"]);
const INDIC: (&str, &[&str]) = ("in", &["Nirmala.ttc", "Nirmala.ttf"]);

fn loaded() -> &'static Mutex<HashSet<&'static str>> {
    static L: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Charge les polices nécessaires pour afficher `text`, si ce n'est pas déjà fait.
pub fn ensure(ctx: &egui::Context, text: &str) {
    let (mut kana, mut han, mut hangul, mut thai, mut indic) = (false, false, false, false, false);
    for c in text.chars() {
        match c as u32 {
            0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9F => kana = true,
            0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x3000..=0x303F | 0xFF00..=0xFF65 => han = true,
            0xAC00..=0xD7A3 | 0x1100..=0x11FF | 0x3130..=0x318F => hangul = true,
            0x0E00..=0x0E7F => thai = true,
            0x0900..=0x097F => indic = true,
            _ => {}
        }
    }
    // Des kanjis avec des kanas = japonais ; des idéogrammes seuls = chinois.
    let mut wanted = vec![];
    if kana || (han && !hangul && is_loaded("jp")) {
        wanted.push(JAPANESE);
    } else if han {
        wanted.push(CHINESE);
    }
    if hangul {
        wanted.push(KOREAN);
    }
    if thai {
        wanted.push(THAI);
    }
    if indic {
        wanted.push(INDIC);
    }
    for (name, files) in wanted {
        load(ctx, name, files);
    }
}

fn is_loaded(name: &str) -> bool {
    loaded().lock().unwrap().contains(name)
}

fn load(ctx: &egui::Context, name: &'static str, files: &[&str]) {
    if !loaded().lock().unwrap().insert(name) {
        return;
    }
    let Some(bytes) = files.iter().find_map(|f| std::fs::read(format!(r"C:\Windows\Fonts\{f}")).ok()) else { return };
    let families = [FontFamily::Proportional, FontFamily::Name("bold".into()), FontFamily::Name("heavy".into())]
        .into_iter()
        .map(|family| InsertFontFamily { family, priority: FontPriority::Lowest })
        .collect();
    // Les polices CJK de Windows ont une ligne de base plus haute que Google Sans : sans
    // correction, kanjis et crochets 「」【】 flottent au-dessus du texte latin.
    let mut data = egui::FontData::from_owned(bytes);
    let default = if matches!(name, "jp" | "zh" | "ko") { 0.11 } else { 0.0 };
    data.tweak.y_offset_factor = std::env::var("MIXTAPE_CJK_OFFSET").ok().and_then(|v| v.parse().ok()).unwrap_or(default);
    ctx.add_font(FontInsert { name: format!("script-{name}"), data, families });
}
