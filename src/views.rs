//! Interface, dans l'esprit Material 3 Expressive de Swiftify : palette violet nuit et
//! accent lavande, boutons « cookie » festonnés, barres de progression ondulées, barre
//! de lecture flottante, vue immersive avec fond liquide tiré de la pochette.
//! Tout est dessiné ici (aucun code ni image de Swiftify). Les vues poussent des
//! `Action`, appliquées en fin d'image.

use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, FontFamily, FontId, Key, Layout, Margin, Pos2, Rect, RichText, Sense, Shape, Stroke,
    UiBuilder, Vec2,
};

use crate::library::{Library, Repeat, EQ_BANDS, EQ_PRESETS};
use crate::lyrics::{self, Lyrics};
use crate::mpv::Props;
use crate::search::{fmt_time, Track};
use crate::{Action, App, ImmTab, Loaded, RightPane, SearchStatus, Sleep, View};

// --- Palette -------------------------------------------------------------------------
pub const BG: Color32 = Color32::from_rgb(0x22, 0x20, 0x38);
const MAIN: Color32 = Color32::from_rgb(0x2a, 0x29, 0x43);
const SURFACE: Color32 = Color32::from_rgb(0x36, 0x34, 0x55);
const SURFACE_HI: Color32 = Color32::from_rgb(0x41, 0x3e, 0x63);
const SELECT: Color32 = Color32::from_rgb(0x52, 0x46, 0x6e);
const BAR: Color32 = Color32::from_rgb(0x3a, 0x38, 0x5c);
const PRIMARY: Color32 = Color32::from_rgb(0xd0, 0xbc, 0xff);
const ON_PRIMARY: Color32 = Color32::from_rgb(0x2e, 0x1a, 0x5c);
const CONTAINER: Color32 = Color32::from_rgb(0x5a, 0x4a, 0x86);
const TEXT: Color32 = Color32::from_rgb(0xe7, 0xe2, 0xf3);
const MUTED: Color32 = Color32::from_rgb(0xa8, 0xa2, 0xc4);
const FAINT: Color32 = Color32::from_rgb(0x6c, 0x67, 0x8c);
const ERROR: Color32 = Color32::from_rgb(0xff, 0x9e, 0xa8);

// --- Icônes Material Icons Round ------------------------------------------------------
const I_HOME: char = '\u{e88a}';
const I_SEARCH: char = '\u{e8b6}';
const I_BACK: char = '\u{e5c4}';
const I_HEART: char = '\u{e87e}';
const I_HEART_FILL: char = '\u{e87d}';
const I_ADD: char = '\u{e145}';
const I_CLOSE: char = '\u{e5cd}';
const I_DELETE: char = '\u{e872}';
const I_MORE: char = '\u{eae1}';
const I_PREV: char = '\u{e045}';
const I_PLAY: char = '\u{e037}';
const I_PAUSE: char = '\u{e034}';
const I_NEXT: char = '\u{e044}';
const I_SHUFFLE: char = '\u{e043}';
const I_REPEAT: char = '\u{e040}';
const I_REPEAT_ONE: char = '\u{e041}';
const I_VOLUME: char = '\u{e050}';
const I_MUTE: char = '\u{e04f}';
const I_QUEUE: char = '\u{e03d}';
const I_LYRICS: char = '\u{ec0b}';
const I_NOTE: char = '\u{e405}';
const I_HISTORY: char = '\u{e889}';
const I_CHEVRON: char = '\u{e5cc}';
const I_IMAGE: char = '\u{e3f4}';
const I_BEDTIME: char = '\u{ef44}';
const I_TUNE: char = '\u{e429}';
const I_SUN: char = '\u{e518}';
const I_MOON: char = '\u{e51c}';
const I_TWILIGHT: char = '\u{e1c6}';
const I_STAR: char = '\u{e838}';
const I_ARROW: char = '\u{e5c8}';
const I_GRID: char = '\u{e9b0}';
const I_WAIT: char = '\u{e88b}';
const I_PLAYLIST: char = '\u{e05f}';
const I_SETTINGS: char = '\u{e8b8}';
const I_GRAPH: char = '\u{e1b8}';
const I_IMPORT: char = '\u{e03b}';
const I_LINK: char = '\u{e157}';
const I_FILE: char = '\u{e24d}';
const I_CHECK: char = '\u{e5ca}';

// --- Style ---------------------------------------------------------------------------

pub fn setup_style(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let embedded: [(&str, &'static [u8]); 7] = [
        ("sans", include_bytes!("../assets/fonts/sans-400.ttf")),
        ("sans-ext", include_bytes!("../assets/fonts/sans-ext-400.ttf")),
        ("sans-sb", include_bytes!("../assets/fonts/sans-600.ttf")),
        ("sans-sb-ext", include_bytes!("../assets/fonts/sans-ext-600.ttf")),
        ("sans-xb", include_bytes!("../assets/fonts/sans-800.ttf")),
        ("sans-xb-ext", include_bytes!("../assets/fonts/sans-ext-800.ttf")),
        ("material", include_bytes!("../assets/fonts/MaterialIconsRound-Regular.otf")),
    ];
    for (name, bytes) in embedded {
        fonts.font_data.insert(name.into(), egui::FontData::from_static(bytes).into());
    }
    // Segoe UI en secours pour les caractères absents (symboles, autres alphabets).
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
        fonts.font_data.insert("segoe".into(), egui::FontData::from_owned(bytes).into());
    }
    let base: Vec<String> = fonts.families[&FontFamily::Proportional].clone();
    let fam = |names: &[&str]| -> Vec<String> {
        names.iter().filter(|k| fonts.font_data.contains_key(**k)).map(|k| k.to_string()).chain(base.iter().cloned()).collect()
    };
    let regular = fam(&["sans", "sans-ext", "segoe"]);
    let semibold = fam(&["sans-sb", "sans-sb-ext", "segoe"]);
    let heavy = fam(&["sans-xb", "sans-xb-ext", "segoe"]);
    let icons = fam(&["material"]);
    fonts.families.insert(FontFamily::Proportional, regular);
    fonts.families.insert(FontFamily::Name("bold".into()), semibold);
    fonts.families.insert(FontFamily::Name("heavy".into()), heavy);
    fonts.families.insert(FontFamily::Name("icons".into()), icons);
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE_HI;
    v.window_stroke = Stroke::NONE;
    v.window_corner_radius = 16.into();
    v.menu_corner_radius = 16.into();
    v.extreme_bg_color = SURFACE;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = CONTAINER;
    v.widgets.inactive.bg_fill = SURFACE;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.corner_radius = 12.into();
    v.widgets.hovered.weak_bg_fill = SELECT;
    v.widgets.hovered.bg_fill = SELECT;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.hovered.corner_radius = 12.into();
    v.widgets.active.weak_bg_fill = CONTAINER;
    v.widgets.active.corner_radius = 12.into();
    v.widgets.open.weak_bg_fill = SELECT;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 4.0);
        s.spacing.menu_margin = Margin::same(8);
        s.spacing.button_padding = Vec2::new(10.0, 6.0);
        // Le fondu des zones défilantes prend la couleur du panneau egui (BG), pas celle des
        // cartes dessinées à la main : il apparaissait comme une bande sombre.
        s.spacing.scroll.fade.strength = 0.0;
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(15.0));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(15.0));
    });
}

/// Contenu placé dans un rectangle déjà réservé, *sans* l'allouer à nouveau dans le
/// parent (`scope_builder` alloue, ce qui faisait reculer le curseur de mise en page et
/// chevaucher les éléments suivants).
fn place<R>(ui: &mut egui::Ui, b: UiBuilder, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut child = ui.new_child(b);
    f(&mut child)
}

/// Avancement 0→1 d'un survol, adouci (transition de ~0,2 s).
fn hover_t(ui: &egui::Ui, id: egui::Id, hovered: bool) -> f32 {
    let t = ui.ctx().animate_bool_with_time(id.with("hover"), hovered, 0.2);
    t * t * (3.0 - 2.0 * t)
}

fn ico(glyph: char, size: f32, color: Color32) -> RichText {
    RichText::new(glyph.to_string()).font(icon_font(size)).color(color)
}

fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("icons".into()))
}

fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

fn heavy(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("heavy".into()))
}

fn titres(n: usize) -> String {
    if n > 1 { format!("{n} titres") } else { format!("{n} titre") }
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), m(a.a(), b.a()))
}

fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Bruit lisse pour animer le halo et le fond (somme de sinusoïdes).
fn wobble(i: f32, t: f32) -> f32 {
    0.5 + 0.5 * ((i * 0.37 + t * 2.1).sin() * 0.5 + (i * 0.13 - t * 1.3).sin() * 0.3 + (i * 0.71 + t * 3.7).sin() * 0.2)
}

// --- Formes : cookie, vagues, dégradés -------------------------------------------------

/// Contour festonné (forme « cookie » de Material 3 Expressive).
fn cookie_points(c: Pos2, r: f32, lobes: f32, amp: f32, rot: f32) -> Vec<Pos2> {
    let n = ((r * 2.4) as usize).clamp(48, 220);
    (0..n)
        .map(|k| {
            let a = k as f32 / n as f32 * TAU;
            let rr = r * (1.0 + amp * (lobes * (a + rot)).cos()) / (1.0 + amp);
            Pos2::new(c.x + rr * a.cos(), c.y + rr * a.sin())
        })
        .collect()
}

fn cookie(c: Pos2, r: f32, lobes: f32, amp: f32, rot: f32, color: Color32) -> Shape {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, color);
    let pts = cookie_points(c, r, lobes, amp, rot);
    for p in &pts {
        mesh.colored_vertex(*p, color);
    }
    let n = pts.len() as u32;
    for k in 0..n {
        mesh.add_triangle(0, 1 + k, 1 + (k + 1) % n);
    }
    // Le maillage n'est pas lissé : un contour fin (que egui lisse) adoucit le bord.
    Shape::Vec(vec![Shape::mesh(mesh), Shape::closed_line(pts, Stroke::new(1.2, color))])
}

/// Image découpée en forme de cookie (UV calculées depuis le centre), bord lissé par
/// un contour de la couleur `edge`.
fn cookie_image(c: Pos2, r: f32, lobes: f32, amp: f32, rot: f32, tex: egui::TextureId, edge: Color32) -> Shape {
    let mut mesh = egui::Mesh::with_texture(tex);
    let uv = |p: Pos2| Pos2::new(0.5 + (p.x - c.x) / (2.0 * r), 0.5 + (p.y - c.y) / (2.0 * r));
    mesh.vertices.push(egui::epaint::Vertex { pos: c, uv: uv(c), color: Color32::WHITE });
    let pts = cookie_points(c, r, lobes, amp, rot);
    for p in &pts {
        mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv: uv(*p), color: Color32::WHITE });
    }
    let n = pts.len() as u32;
    for k in 0..n {
        mesh.add_triangle(0, 1 + k, 1 + (k + 1) % n);
    }
    Shape::Vec(vec![Shape::mesh(mesh), Shape::closed_line(pts, Stroke::new(1.5, edge))])
}

/// Tache de couleur à bord fondu (dégradé radial), pour le fond « liquide ».
fn blob(c: Pos2, r: f32, color: Color32) -> Shape {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, color);
    let n = 48;
    for k in 0..n {
        let a = k as f32 / n as f32 * TAU;
        mesh.colored_vertex(Pos2::new(c.x + r * a.cos(), c.y + r * a.sin()), alpha(color, 0));
    }
    for k in 0..n {
        mesh.add_triangle(0, 1 + k, 1 + (k + 1) % n);
    }
    Shape::mesh(mesh)
}

fn wave_points(x0: f32, x1: f32, y: f32, amp: f32, wavelength: f32, phase: f32) -> Vec<Pos2> {
    let mut pts = vec![];
    let mut x = x0;
    while x < x1 {
        pts.push(Pos2::new(x, y + amp * ((x - x0) / wavelength * TAU - phase).sin()));
        x += 2.0;
    }
    pts.push(Pos2::new(x1, y + amp * ((x1 - x0) / wavelength * TAU - phase).sin()));
    pts
}

/// Séparateur ondulé (à côté des titres de section).
fn wave_divider(ui: &mut egui::Ui, width: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width.max(0.0), 14.0), Sense::hover());
    if rect.width() > 8.0 {
        ui.painter().add(Shape::line(wave_points(rect.left(), rect.right(), rect.center().y, 2.5, 14.0, 0.0), Stroke::new(1.6, color)));
    }
}

// --- Widgets -------------------------------------------------------------------------

/// Bouton lecture/pause en cookie : forme douce à l'arrêt, 9 lobes en lecture (morphing),
/// rotation lente pendant la lecture, rebond de l'icône au changement d'état.
fn play_button(ui: &mut egui::Ui, d: f32, glyph: char, fill: Color32, icon: Color32, spin: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(d), Sense::click());
    let t = ui.input(|i| i.time) as f32;
    let id = resp.id;
    let morph = ui.ctx().animate_bool_with_time(id.with("morph"), spin, 0.35);
    let h = hover_t(ui, id, resp.hovered());
    let press = ui.ctx().animate_bool_with_time(id.with("press"), resp.is_pointer_button_down_on(), 0.08);
    // Angle accumulé : la rotation ralentit en douceur à la pause au lieu de sauter.
    // (Ne jamais appeler ui.input() dans ui.data_mut() : même verrou → interblocage.)
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    let rot = ui.data_mut(|m| {
        let a: &mut f32 = m.get_temp_mut_or_default(id.with("rot"));
        *a += dt * 0.45 * morph;
        *a
    });
    let amp = 0.025 + 0.04 * morph;
    let lobes = 9.0;
    let scale = 1.0 + 0.05 * h - 0.06 * press;
    ui.painter().add(cookie(rect.center(), d / 2.0 * scale, lobes, amp, rot, lerp(fill, Color32::WHITE, 0.12 * h)));
    // Rebond de l'icône quand l'état change.
    let changed_at: f32 = ui.data_mut(|m| {
        let st: &mut (bool, f32) = m.get_temp_mut_or_insert_with(id.with("state"), || (spin, -10.0));
        if st.0 != spin {
            *st = (spin, t);
        }
        st.1
    });
    let k = ((t - changed_at) / 0.35).clamp(0.0, 1.0);
    let pop = if k < 1.0 { 0.6 + 0.4 * (1.0 - (1.0 - k).powi(3)) + 0.12 * (k * std::f32::consts::PI).sin() } else { 1.0 };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph.to_string(), icon_font(d * 0.5 * pop), icon);
    if spin || k < 1.0 || morph > 0.0 && morph < 1.0 || h > 0.0 && h < 1.0 {
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    } else if spin {
        ui.ctx().request_repaint_after(Duration::from_millis(66));
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Bouton icône rond (fond optionnel quand actif).
fn icon_btn(ui: &mut egui::Ui, glyph: char, size: f32, color: Color32, active_bg: Option<Color32>, tip: &str) -> egui::Response {
    let d = size + 18.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(d), Sense::click());
    let h = hover_t(ui, resp.id, resp.hovered());
    let press = ui.ctx().animate_bool_with_time(resp.id.with("press"), resp.is_pointer_button_down_on(), 0.08);
    let r = d / 2.0 * (1.0 - 0.08 * press);
    match active_bg {
        Some(bg) => {
            ui.painter().circle_filled(rect.center(), r, lerp(bg, Color32::WHITE, 0.1 * h));
        }
        None if h > 0.0 => {
            ui.painter().circle_filled(rect.center(), r * (0.7 + 0.3 * h), Color32::from_white_alpha((20.0 * h) as u8));
        }
        None => {}
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph.to_string(), icon_font(size * (1.0 + 0.06 * h)), color);
    resp.on_hover_text(tip)
}

/// Barre de progression ondulée : (aperçu pendant le glissement, position à appliquer).
fn wavy_progress(ui: &mut egui::Ui, width: f32, pos: f64, dur: f64, playing: bool, color: Color32, track: Color32) -> (Option<f64>, Option<f64>) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width.max(40.0), 22.0), Sense::click_and_drag());
    let (x0, x1) = (rect.left() + 3.0, rect.right() - 3.0);
    let frac_at = |x: f32| ((x - x0) / (x1 - x0)).clamp(0.0, 1.0) as f64;
    let mut preview = None;
    let mut commit = None;
    let mut frac = if dur > 0.0 { (pos / dur).clamp(0.0, 1.0) } else { 0.0 };
    if dur > 0.0 {
        if let Some(p) = resp.interact_pointer_pos() {
            if resp.dragged() {
                frac = frac_at(p.x);
                preview = Some(frac * dur);
            }
            if resp.drag_stopped() || resp.clicked() {
                commit = Some(frac_at(p.x) * dur);
            }
        }
    }
    let xf = x0 + (x1 - x0) * frac as f32;
    let y = rect.center().y;
    let t = ui.input(|i| i.time) as f32;
    let phase = if playing { t * 4.0 } else { 0.0 };
    if xf > x0 + 1.0 {
        ui.painter().add(Shape::line(wave_points(x0, xf, y, 3.0, 22.0, phase), Stroke::new(3.5, color)));
    }
    if x1 > xf + 6.0 {
        ui.painter().line_segment([Pos2::new(xf + 6.0, y), Pos2::new(x1, y)], Stroke::new(3.5, track));
    }
    let hot = resp.hovered() || resp.dragged();
    ui.painter().rect_filled(Rect::from_center_size(Pos2::new(xf, y), Vec2::new(4.0, if hot { 20.0 } else { 14.0 })), 2.0, color);
    if hot {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    (preview, commit)
}

/// Curseur horizontal fin (volume) : renvoie la nouvelle valeur 0..=1 si modifiée.
fn thin_slider(ui: &mut egui::Ui, width: f32, value: f32, color: Color32, track: Color32, thick: f32) -> Option<f32> {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::click_and_drag());
    let (x0, x1) = (rect.left() + 2.0, rect.right() - 2.0);
    let mut v = value.clamp(0.0, 1.0);
    let mut changed = None;
    if let Some(p) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() {
            v = ((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
            changed = Some(v);
        }
    }
    let y = rect.center().y;
    let xv = x0 + (x1 - x0) * v;
    ui.painter().rect_filled(Rect::from_min_max(Pos2::new(x0, y - thick / 2.0), Pos2::new(x1, y + thick / 2.0)), thick / 2.0, track);
    ui.painter().rect_filled(Rect::from_min_max(Pos2::new(x0, y - thick / 2.0), Pos2::new(xv, y + thick / 2.0)), thick / 2.0, color);
    ui.painter().rect_filled(Rect::from_center_size(Pos2::new(xv, y), Vec2::new(4.0, thick + 12.0)), 2.0, color);
    changed
}

/// Interrupteur Material 3.
fn switch(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(56.0, 32.0), Sense::click());
    let k = ui.ctx().animate_bool(resp.id, on);
    let track = lerp(Color32::from_white_alpha(40), PRIMARY, k);
    ui.painter().rect_filled(rect, 16.0, track);
    let r = 8.0 + 4.0 * k;
    let cx = egui::lerp((rect.left() + 16.0)..=(rect.right() - 16.0), k);
    ui.painter().circle_filled(Pos2::new(cx, rect.center().y), r, lerp(Color32::from_white_alpha(200), ON_PRIMARY, k));
    resp
}

/// Curseur vertical de l'égaliseur (gain en dB, -12..=12).
fn eq_slider(ui: &mut egui::Ui, id: usize, gain: f32, height: f32) -> Option<f32> {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(44.0, height), Sense::click_and_drag());
    let (y_top, y_bot) = (rect.top() + 10.0, rect.bottom() - 10.0);
    let to_y = |g: f32| egui::remap(g, 12.0..=-12.0, y_top..=y_bot);
    let mut g = gain;
    let mut changed = None;
    if let Some(p) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() {
            g = egui::remap_clamp(p.y, y_top..=y_bot, 12.0..=-12.0);
            g = (g * 2.0).round() / 2.0;
            changed = Some(g);
        }
    }
    let _ = id;
    let cx = rect.center().x;
    ui.painter().rect_filled(Rect::from_min_max(Pos2::new(cx - 3.0, y_top), Pos2::new(cx + 3.0, y_bot)), 3.0, Color32::from_white_alpha(45));
    let y0 = to_y(0.0);
    let yg = to_y(g);
    ui.painter().rect_filled(Rect::from_min_max(Pos2::new(cx - 3.0, y0.min(yg)), Pos2::new(cx + 3.0, y0.max(yg))), 3.0, Color32::from_white_alpha(170));
    let thumb = Rect::from_center_size(Pos2::new(cx, yg), Vec2::new(34.0, 16.0));
    ui.painter().rect_filled(thumb, 8.0, Color32::WHITE);
    ui.painter().rect_filled(Rect::from_center_size(thumb.center(), Vec2::new(14.0, 2.5)), 1.0, Color32::from_gray(150));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    changed
}

/// Miniature carrée recadrée au centre (les vignettes YouTube sont en 16:9).
fn square_thumb(url: &str) -> egui::Image<'_> {
    let w = 9.0 / 16.0;
    egui::Image::new(url)
        .uv(Rect::from_min_max(Pos2::new(0.5 - w / 2.0, 0.0), Pos2::new(0.5 + w / 2.0, 1.0)))
        .maintain_aspect_ratio(false)
        .show_loading_spinner(false)
}

fn put_square_thumb(ui: &mut egui::Ui, rect: Rect, url: &str, radius: u8) {
    ui.painter().rect_filled(rect, radius as f32, SURFACE_HI);
    ui.put(rect, square_thumb(url).fit_to_exact_size(rect.size()).corner_radius(radius));
}

/// Mosaïque 2×2 des premières pochettes (cartes de playlists, comme Swiftify).
fn collage(ui: &mut egui::Ui, rect: Rect, tracks: &[Track], radius: u8, fallback: char) {
    ui.painter().rect_filled(rect, radius as f32, CONTAINER);
    match tracks.len() {
        0 => {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, fallback.to_string(), icon_font(rect.height() * 0.4), PRIMARY);
        }
        1..=3 => put_square_thumb(ui, rect, &tracks[0].thumb, radius),
        _ => {
            let h = rect.size() / 2.0;
            let corners = [
                egui::CornerRadius { nw: radius, ne: 0, sw: 0, se: 0 },
                egui::CornerRadius { nw: 0, ne: radius, sw: 0, se: 0 },
                egui::CornerRadius { nw: 0, ne: 0, sw: radius, se: 0 },
                egui::CornerRadius { nw: 0, ne: 0, sw: 0, se: radius },
            ];
            for (k, cr) in corners.into_iter().enumerate() {
                let r = Rect::from_min_size(rect.min + Vec2::new((k % 2) as f32 * h.x, (k / 2) as f32 * h.y), h);
                ui.put(r, square_thumb(&tracks[k].thumb).fit_to_exact_size(h).corner_radius(cr));
            }
        }
    }
}

/// Texte tronqué (titre + sous-titre) dans un rectangle.
fn text_in(ui: &mut egui::Ui, rect: Rect, title: RichText, subtitle: Option<RichText>) {
    place(ui, UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)), |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        let h = if subtitle.is_some() { 40.0 } else { 20.0 };
        ui.add_space(((rect.height() - h) / 2.0).max(0.0));
        ui.add(egui::Label::new(title).truncate().selectable(false));
        if let Some(s) = subtitle {
            ui.add(egui::Label::new(s).truncate().selectable(false));
        }
    });
}

/// Titre lisible pour l'affichage (« Artiste - Titre (Official Video) » → titre, artiste).
fn display_names(t: &Track) -> (String, String) {
    let (artist, title) = lyrics::display_artist_title(t);
    let title = if title.is_empty() { t.title.clone() } else { title };
    let artist = if artist.is_empty() { t.artist.clone() } else { artist };
    (title, artist)
}

// ---------------------------------------------------------------------------------
// Entrée principale
// ---------------------------------------------------------------------------------

pub fn render(app: &mut App, ui: &mut egui::Ui, p: &Props) {
    // Fond de toute la fenêtre (sinon la couleur d'effacement apparaît, quasi noire).
    let win = ui.max_rect();
    ui.painter().rect_filled(win, 0.0, BG);
    window_drag_and_resize(ui, win);
    if app.view == View::NowPlaying {
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let rect = ui.max_rect();
            immersive(app, ui, p, rect);
            toast(app, ui, rect);
        });
        window_controls(ui, win, true);
        return;
    }
    sidebar(app, ui);
    // Le panneau de droite n'apparaît que s'il laisse assez de place à la vue principale.
    app.right_fits = ui.available_width() >= 380.0 + 780.0;
    if app.right != RightPane::None && app.right_fits {
        right_pane(app, ui, p);
    }
    egui::CentralPanel::no_frame().show(ui, |ui| {
        let full = ui.max_rect();
        let rect = Rect::from_min_max(full.min + Vec2::new(0.0, 12.0), full.max - Vec2::new(12.0, 12.0));
        ui.painter().rect_filled(rect, 28.0, MAIN);
        let bar = Rect::from_min_max(Pos2::new(rect.left() + 20.0, rect.bottom() - 96.0), Pos2::new(rect.right() - 20.0, rect.bottom() - 16.0));
        let content = Rect::from_min_max(rect.min + Vec2::new(32.0, 24.0), Pos2::new(rect.right() - 32.0, bar.top() - 12.0));
        place(ui, UiBuilder::new().max_rect(content).layout(Layout::top_down(Align::Min)), |ui| {
            ui.set_clip_rect(content.expand(8.0));
            match app.view {
                View::Home => home(app, ui, p),
                View::Search => search_view(app, ui, p),
                View::Liked => collection(app, ui, p, Coll::Liked),
                View::History => collection(app, ui, p, Coll::History),
                View::Playlist(i) => collection(app, ui, p, Coll::Playlist(i)),
                View::Playlists => playlists_view(app, ui),
                View::Settings => settings_view(app, ui, p),
                View::Import => import_view(app, ui),
                View::NowPlaying => {}
            }
        });
        player_bar(app, ui, p, bar);
        toast(app, ui, rect);
    });
    window_controls(ui, win, false);
}

/// Fenêtre sans barre Windows : la bande du haut déplace la fenêtre (double-clic =
/// agrandir/restaurer) et les bords la redimensionnent. Enregistré avant le contenu,
/// donc les boutons posés par-dessus restent prioritaires.
fn window_drag_and_resize(ui: &mut egui::Ui, win: Rect) {
    let band = Rect::from_min_size(win.min, Vec2::new(win.width(), 40.0));
    let resp = ui.interact(band, ui.scope_id().with("window-drag"), Sense::click_and_drag());
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    if resp.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    if maximized {
        return;
    }
    use egui::viewport::ResizeDirection as D;
    let Some(pos) = ui.input(|i| i.pointer.hover_pos()) else { return };
    let m = 6.0;
    let (l, r, t, b) = (pos.x < win.left() + m, pos.x > win.right() - m, pos.y < win.top() + m, pos.y > win.bottom() - m);
    let dir = match (l, r, t, b) {
        (true, _, true, _) => Some(D::NorthWest),
        (_, true, true, _) => Some(D::NorthEast),
        (true, _, _, true) => Some(D::SouthWest),
        (_, true, _, true) => Some(D::SouthEast),
        (true, ..) => Some(D::West),
        (_, true, ..) => Some(D::East),
        (_, _, true, _) => Some(D::North),
        (.., true) => Some(D::South),
        _ => None,
    };
    if let Some(d) = dir {
        let icon = match d {
            D::North | D::South => egui::CursorIcon::ResizeVertical,
            D::East | D::West => egui::CursorIcon::ResizeHorizontal,
            D::NorthWest | D::SouthEast => egui::CursorIcon::ResizeNwSe,
            _ => egui::CursorIcon::ResizeNeSw,
        };
        ui.ctx().set_cursor_icon(icon);
        if ui.input(|i| i.pointer.primary_pressed()) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::BeginResize(d));
        }
    }
}

/// Boutons de fenêtre en haut à gauche (fermer, agrandir, réduire), en formes cookie.
fn window_controls(ui: &mut egui::Ui, win: Rect, on_art: bool) {
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    let buttons: [(char, f32, &str); 3] = [
        ('\u{e5cd}', 9.0, "Fermer"),
        (if maximized { '\u{f1cf}' } else { '\u{f1ce}' }, 6.0, if maximized { "Restaurer" } else { "Agrandir" }),
        ('\u{e15b}', 4.0, "Réduire"),
    ];
    for (k, (glyph, lobes, tip)) in buttons.into_iter().enumerate() {
        let c = Pos2::new(win.left() + 32.0 + k as f32 * 42.0, win.top() + 30.0);
        let r = Rect::from_center_size(c, Vec2::splat(38.0));
        let resp = ui.interact(r, ui.scope_id().with(("window-btn", k)), Sense::click());
        let h = hover_t(ui, resp.id, resp.hovered());
        let base = if on_art { Color32::from_white_alpha(30) } else { SURFACE };
        let hot = if k == 0 { Color32::from_rgb(0x93, 0x00, 0x0a) } else if on_art { Color32::from_white_alpha(70) } else { SELECT };
        ui.painter().add(cookie(c, 18.0 + 1.5 * h, lobes, 0.08, 0.6 * h, lerp(base, hot, h)));
        let icon_c = if k == 0 { lerp(TEXT, Color32::from_rgb(0xff, 0xda, 0xd6), h) } else { TEXT };
        ui.painter().text(c, Align2::CENTER_CENTER, glyph.to_string(), icon_font(17.0), icon_c);
        if resp.on_hover_text(tip).clicked() {
            ui.ctx().send_viewport_cmd(match k {
                0 => egui::ViewportCommand::Close,
                1 => egui::ViewportCommand::Maximized(!maximized),
                _ => egui::ViewportCommand::Minimized(true),
            });
        }
    }
}

pub fn shortcuts(app: &mut App, ui: &egui::Ui, _p: &Props) {
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    let (space, next, prev, find, esc) = ui.input(|i| {
        (
            i.key_pressed(Key::Space),
            i.modifiers.ctrl && i.key_pressed(Key::ArrowRight),
            i.modifiers.ctrl && i.key_pressed(Key::ArrowLeft),
            i.modifiers.ctrl && i.key_pressed(Key::L),
            i.key_pressed(Key::Escape),
        )
    });
    if space && !typing {
        app.actions.push(Action::TogglePause);
    }
    if next {
        app.actions.push(Action::Next);
    }
    if prev {
        app.actions.push(Action::Prev);
    }
    if find {
        app.focus_search = true;
        app.actions.push(Action::Go(View::Search));
    }
    if esc && app.view == View::NowPlaying && !typing {
        app.view = app.back.pop().unwrap_or(View::Home);
    }
}

fn toast(app: &mut App, ui: &mut egui::Ui, rect: Rect) {
    let Some((msg, at)) = &app.error else { return };
    if at.elapsed() > Duration::from_secs(6) {
        app.error = None;
        return;
    }
    ui.ctx().request_repaint_after(Duration::from_millis(500));
    let galley = ui.painter().layout(msg.clone(), FontId::proportional(14.0), ON_PRIMARY, rect.width() - 120.0);
    let size = galley.size() + Vec2::new(36.0, 20.0);
    let r = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 24.0 + size.y / 2.0), size);
    ui.painter().rect_filled(r, size.y / 2.0, ERROR);
    ui.painter().galley(r.min + Vec2::new(18.0, 10.0), galley, ON_PRIMARY);
}

// ---------------------------------------------------------------------------------
// Barre latérale
// ---------------------------------------------------------------------------------

fn nav_row(ui: &mut egui::Ui, selected: bool, glyph: Option<char>, thumb: Option<&[Track]>, label: &str, trailing: Option<char>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 50.0), Sense::click());
    let h = hover_t(ui, resp.id, resp.hovered());
    let sel = ui.ctx().animate_bool_with_time(resp.id.with("sel"), selected, 0.25);
    let a = (sel * 255.0).max(h * 120.0) as u8;
    if a > 0 {
        let grow = 0.94 + 0.06 * sel.max(h);
        ui.painter().rect_filled(Rect::from_center_size(rect.center(), Vec2::new(rect.width() * grow, rect.height())), 25.0, alpha(SELECT, a));
    }
    let ic = Pos2::new(rect.left() + 30.0, rect.center().y);
    if let Some(tracks) = thumb {
        collage(ui, Rect::from_center_size(ic, Vec2::splat(34.0)), tracks, 8, I_NOTE);
    } else if let Some(g) = glyph {
        if selected {
            ui.painter().add(cookie(ic, 17.0, 8.0, 0.07, 0.0, PRIMARY));
            ui.painter().text(ic, Align2::CENTER_CENTER, g.to_string(), icon_font(19.0), ON_PRIMARY);
        } else {
            ui.painter().text(ic, Align2::CENTER_CENTER, g.to_string(), icon_font(22.0), TEXT);
        }
    }
    let text = Rect::from_min_max(Pos2::new(rect.left() + 58.0, rect.top()), Pos2::new(rect.right() - if trailing.is_some() { 34.0 } else { 12.0 }, rect.bottom()));
    text_in(ui, text, RichText::new(label).size(16.0).color(TEXT), None);
    if let Some(tr) = trailing {
        ui.painter().text(Pos2::new(rect.right() - 20.0, rect.center().y), Align2::CENTER_CENTER, tr.to_string(), icon_font(20.0), MUTED);
    }
    resp
}

fn section_label(ui: &mut egui::Ui, text: &str, add_button: bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(RichText::new(text).font(bold(13.5)).color(MUTED));
        let w = ui.available_width() - if add_button { 40.0 } else { 8.0 };
        wave_divider(ui, w, FAINT);
        if add_button && icon_btn(ui, I_ADD, 18.0, PRIMARY, None, "Créer une playlist").clicked() {
            clicked = true;
        }
    });
    clicked
}

fn sidebar(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::left("nav")
        .exact_size(268.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(BG).inner_margin(Margin { left: 14, right: 12, top: 62, bottom: 16 }))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for (glyph, label, view) in [(I_HOME, "Accueil", View::Home), (I_SEARCH, "Rechercher", View::Search)] {
                if nav_row(ui, app.view == view, Some(glyph), None, label, None).clicked() {
                    app.actions.push(Action::Go(view));
                    if view == View::Search {
                        app.focus_search = true;
                    }
                }
            }
            ui.add_space(10.0);
            section_label(ui, "Bibliothèque", false);
            if nav_row(ui, app.view == View::Liked, Some(I_HEART_FILL), None, "Titres likés", None).clicked() {
                app.actions.push(Action::Go(View::Liked));
            }
            if nav_row(ui, app.view == View::History, Some(I_HISTORY), None, "Historique", None).clicked() {
                app.actions.push(Action::Go(View::History));
            }
            let label = if app.import.running { format!("Import… {}/{}", app.import.processed, app.import.total) } else { "Importer".to_string() };
            if nav_row(ui, app.view == View::Import, Some(I_IMPORT), None, &label, None).clicked() {
                app.actions.push(Action::Go(View::Import));
            }
            ui.add_space(10.0);
            if section_label(ui, "Playlists", true) {
                app.actions.push(Action::NewPlaylist(None));
            }
            let bottom_h = 60.0;
            let list_h = (ui.available_height() - bottom_h - 56.0).max(60.0);
            egui::ScrollArea::vertical().id_salt("nav-pl").max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                for (i, pl) in app.lib.playlists.iter().enumerate() {
                    if nav_row(ui, app.view == View::Playlist(i), None, Some(&pl.tracks), &pl.name, None).clicked() {
                        app.actions.push(Action::Go(View::Playlist(i)));
                    }
                }
            });
            if nav_row(ui, app.view == View::Playlists, Some(I_GRID), None, "Toutes les playlists", Some(I_CHEVRON)).clicked() {
                app.actions.push(Action::Go(View::Playlists));
            }
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                if nav_row(ui, app.view == View::Settings, Some(I_SETTINGS), None, "Paramètres", None).clicked() {
                    app.actions.push(Action::Go(View::Settings));
                }
                if nav_row(ui, false, Some(I_TUNE), None, "Égaliseur et son", None).clicked() {
                    app.imm_tab = ImmTab::Eq;
                    app.actions.push(Action::Go(View::NowPlaying));
                }
            });
        });
}

// ---------------------------------------------------------------------------------
// Menus et listes de titres
// ---------------------------------------------------------------------------------

fn track_menu(ui: &mut egui::Ui, t: &Track, lib: &Library, actions: &mut Vec<Action>, in_playlist: Option<(usize, usize)>) {
    ui.set_min_width(230.0);
    if ui.button("Lire ensuite").clicked() {
        actions.push(Action::PlayNext(t.clone()));
        ui.close();
    }
    if ui.button("Ajouter à la file d'attente").clicked() {
        actions.push(Action::Enqueue(t.clone()));
        ui.close();
    }
    ui.separator();
    let like = if lib.is_liked(&t.url) { "Retirer des titres likés" } else { "Ajouter aux titres likés" };
    if ui.button(like).clicked() {
        actions.push(Action::ToggleLike(t.clone()));
        ui.close();
    }
    ui.menu_button("Ajouter à une playlist", |ui| {
        ui.set_min_width(210.0);
        if ui.button("+ Nouvelle playlist").clicked() {
            actions.push(Action::NewPlaylist(Some(t.clone())));
            ui.close();
        }
        if !lib.playlists.is_empty() {
            ui.separator();
        }
        for (i, pl) in lib.playlists.iter().enumerate() {
            if ui.button(&pl.name).clicked() {
                actions.push(Action::AddToPlaylist(i, t.clone()));
                ui.close();
            }
        }
    });
    if let Some((pl, i)) = in_playlist {
        if ui.button("Retirer de cette playlist").clicked() {
            actions.push(Action::RemoveFromPlaylist(pl, i));
            ui.close();
        }
    }
    ui.separator();
    if ui.button("Copier le lien").clicked() {
        actions.push(Action::CopyLink(t.url.clone()));
        ui.close();
    }
}

fn more_button(ui: &mut egui::Ui, rect: Rect, color: Color32, t: &Track, lib: &Library, actions: &mut Vec<Action>, ctx_pl: Option<(usize, usize)>) {
    place(ui, UiBuilder::new().max_rect(rect), |ui| {
        ui.menu_button(ico(I_MORE, 20.0, color), |ui| track_menu(ui, t, lib, actions, ctx_pl));
    });
}

const ROW_H: f32 = 64.0;

fn track_list(ui: &mut egui::Ui, id: &str, tracks: &[Track], lib: &Library, actions: &mut Vec<Action>, current: Option<&str>, playlist: Option<usize>) {
    egui::ScrollArea::vertical().id_salt(id).auto_shrink(false).show_rows(ui, ROW_H, tracks.len(), |ui, range| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for i in range {
            let t = &tracks[i];
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
            let hovered = ui.rect_contains_pointer(rect);
            let h = hover_t(ui, resp.id, hovered);
            if h > 0.0 {
                ui.painter().rect_filled(rect, 18.0, alpha(SURFACE, (255.0 * h) as u8));
            }
            let is_current = current == Some(t.url.as_str());
            let num = Pos2::new(rect.left() + 22.0, rect.center().y);
            if hovered {
                ui.painter().text(num, Align2::CENTER_CENTER, I_PLAY.to_string(), icon_font(22.0), TEXT);
            } else if is_current {
                ui.painter().text(num, Align2::CENTER_CENTER, I_NOTE.to_string(), icon_font(20.0), PRIMARY);
            } else {
                ui.painter().text(num, Align2::CENTER_CENTER, (i + 1).to_string(), FontId::proportional(14.0), MUTED);
            }
            let art = Rect::from_min_size(Pos2::new(rect.left() + 46.0, rect.top() + 8.0), Vec2::splat(48.0));
            put_square_thumb(ui, art, &t.thumb, 12);

            let liked = lib.is_liked(&t.url);
            let more_r = Rect::from_center_size(Pos2::new(rect.right() - 26.0, rect.center().y), Vec2::splat(36.0));
            let heart_r = more_r.translate(Vec2::new(-40.0, 0.0));
            let add_r = heart_r.translate(Vec2::new(-40.0, 0.0));
            let dur_x = add_r.left() - 12.0;
            ui.painter().text(Pos2::new(dur_x, rect.center().y), Align2::RIGHT_CENTER, fmt_time(t.duration), FontId::proportional(14.0), MUTED);
            let text = Rect::from_min_max(Pos2::new(art.right() + 14.0, rect.top()), Pos2::new(dur_x - 60.0, rect.bottom()));
            text_in(ui, text, RichText::new(&t.title).font(bold(15.5)).color(if is_current { PRIMARY } else { TEXT }), Some(RichText::new(&t.artist).color(MUTED).size(13.5)));

            if hovered || liked {
                let b = ui.put(heart_r, egui::Button::new(ico(if liked { I_HEART_FILL } else { I_HEART }, 20.0, if liked { PRIMARY } else { MUTED })).frame(false));
                if b.on_hover_text(if liked { "Retirer des titres likés" } else { "Ajouter aux titres likés" }).clicked() {
                    actions.push(Action::ToggleLike(t.clone()));
                }
            }
            if hovered {
                let b = ui.put(add_r, egui::Button::new(ico(I_ADD, 20.0, MUTED)).frame(false));
                if b.on_hover_text("Ajouter à la file d'attente").clicked() {
                    actions.push(Action::Enqueue(t.clone()));
                }
            }
            let ctx_pl = playlist.map(|pl| (pl, i));
            more_button(ui, more_r, if hovered { TEXT } else { FAINT }, t, lib, actions, ctx_pl);
            resp.context_menu(|ui| track_menu(ui, t, lib, actions, ctx_pl));
            if resp.clicked() {
                actions.push(Action::Play { tracks: tracks.to_vec(), start: i });
            }
        }
        ui.add_space(12.0);
    });
}

// ---------------------------------------------------------------------------------
// Accueil
// ---------------------------------------------------------------------------------

fn back_button(app: &mut App, ui: &mut egui::Ui) {
    let enabled = !app.back.is_empty();
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::click());
    let h = hover_t(ui, resp.id, resp.hovered() && enabled);
    ui.painter().add(cookie(rect.center(), 23.0, 7.0, 0.05, 0.3 + 0.4 * h, lerp(SURFACE, SELECT, h)));
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, I_BACK.to_string(), icon_font(22.0), if enabled { TEXT } else { FAINT });
    if enabled && resp.on_hover_text("Retour").clicked() {
        if let Some(v) = app.back.pop() {
            app.view = v;
        }
    }
}

fn section_header(ui: &mut egui::Ui, glyph: char, text: &str, see_all: bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
        ui.painter().add(cookie(r.center(), 19.0, 8.0, 0.08, 0.0, CONTAINER));
        ui.painter().text(r.center(), Align2::CENTER_CENTER, glyph.to_string(), icon_font(20.0), PRIMARY);
        ui.label(RichText::new(text).font(heavy(28.0)).color(TEXT));
        let w = ui.available_width() - if see_all { 150.0 } else { 0.0 };
        wave_divider(ui, w, FAINT);
        if see_all {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(132.0, 46.0), Sense::click());
            let h = hover_t(ui, resp.id, resp.hovered());
            ui.painter().rect_filled(r, 23.0, lerp(SELECT, CONTAINER, h));
            ui.painter().text(Pos2::new(r.left() + 30.0 + 4.0 * h, r.center().y), Align2::CENTER_CENTER, I_ARROW.to_string(), icon_font(20.0), TEXT);
            ui.painter().text(Pos2::new(r.left() + 50.0, r.center().y), Align2::LEFT_CENTER, "Tout voir", bold(15.0), TEXT);
            clicked = resp.clicked();
        }
    });
    clicked
}

fn greeting() -> (char, &'static str) {
    match crate::local_hour() {
        5..=11 => (I_SUN, "Bonjour"),
        12..=17 => (I_SUN, "Bon après-midi"),
        18..=21 => (I_TWILIGHT, "Bonsoir"),
        _ => (I_MOON, "Encore debout…"),
    }
}

fn home(app: &mut App, ui: &mut egui::Ui, p: &Props) {
    egui::ScrollArea::vertical().id_salt("home").auto_shrink(false).show(ui, |ui| {
        back_button(app, ui);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(egui::include_image!("../icons/logo-256.png")).fit_to_exact_size(Vec2::splat(64.0)));
            ui.label(RichText::new("Mixtape").font(heavy(52.0)).color(PRIMARY));
        });
        let (g, text) = greeting();
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
            ui.painter().add(cookie(r.center(), 16.0, 7.0, 0.07, 0.0, CONTAINER));
            ui.painter().text(r.center(), Align2::CENTER_CENTER, g.to_string(), icon_font(18.0), PRIMARY);
            ui.label(RichText::new(text).size(19.0).color(MUTED));
        });
        ui.add_space(22.0);
        hero_card(app, ui, p);
        ui.add_space(18.0);

        // Raccourcis (côte à côte, ou empilés si la place manque)
        let avail = ui.available_width();
        let two = avail >= 620.0;
        let (row, _) = ui.allocate_exact_size(Vec2::new(avail, if two { 88.0 } else { 188.0 }), Sense::hover());
        let (r1, r2) = if two {
            let w = (avail - 16.0) / 2.0;
            (Rect::from_min_size(row.min, Vec2::new(w, 88.0)), Rect::from_min_size(row.min + Vec2::new(w + 16.0, 0.0), Vec2::new(w, 88.0)))
        } else {
            (Rect::from_min_size(row.min, Vec2::new(avail, 88.0)), Rect::from_min_size(row.min + Vec2::new(0.0, 100.0), Vec2::new(avail, 88.0)))
        };
        let liked = app.lib.liked.len();
        let pls = app.lib.playlists.len();
        if quick_card(ui, r1, I_STAR, "Titres likés", &format!("{liked} favori{}", if liked > 1 { "s" } else { "" })).clicked() {
            app.actions.push(Action::Go(View::Liked));
        }
        let sub = if pls == 0 { "Crée ta première playlist".to_string() } else { format!("{pls} collection{} à explorer", if pls > 1 { "s" } else { "" }) };
        if quick_card(ui, r2, I_PLAYLIST, "Tes playlists", &sub).clicked() {
            app.actions.push(Action::Go(View::Playlists));
        }

        if !app.lib.history.is_empty() {
            ui.add_space(26.0);
            if section_header(ui, I_HISTORY, "Écoutés récemment", true) {
                app.actions.push(Action::Go(View::History));
            }
            ui.add_space(12.0);
            let card = 200.0;
            let n = ((ui.available_width() + 18.0) / (card + 18.0)).floor().max(1.0) as usize;
            let history: Vec<Track> = app.lib.history.clone();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                for (i, t) in history.iter().enumerate().take(n) {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(card, card + 56.0), Sense::click());
                    let art = Rect::from_min_size(rect.min, Vec2::splat(card));
                    put_square_thumb(ui, art, &t.thumb, 18);
                    let h = hover_t(ui, resp.id, resp.hovered());
                    if h > 0.0 {
                        ui.painter().rect_filled(art, 18.0, Color32::from_black_alpha((70.0 * h) as u8));
                        let c = Pos2::new(art.right() - 34.0, art.bottom() - 34.0 + 12.0 * (1.0 - h));
                        ui.painter().add(cookie(c, 24.0 * (0.6 + 0.4 * h), 9.0, 0.06, 0.0, alpha(PRIMARY, (255.0 * h) as u8)));
                        ui.painter().text(c, Align2::CENTER_CENTER, I_PLAY.to_string(), icon_font(26.0 * (0.6 + 0.4 * h)), alpha(ON_PRIMARY, (255.0 * h) as u8));
                    }
                    let (title, artist) = display_names(t);
                    let text = Rect::from_min_max(Pos2::new(rect.left() + 4.0, art.bottom() + 6.0), rect.max);
                    place(ui, UiBuilder::new().max_rect(text).layout(Layout::top_down(Align::Min)), |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.add(egui::Label::new(RichText::new(title).font(bold(15.5))).truncate().selectable(false));
                        ui.add(egui::Label::new(RichText::new(artist).color(MUTED).size(13.5)).truncate().selectable(false));
                    });
                    let lib = &app.lib;
                    let actions = &mut app.actions;
                    resp.context_menu(|ui| track_menu(ui, t, lib, actions, None));
                    if resp.clicked() {
                        app.actions.push(Action::Play { tracks: history.clone(), start: i });
                    }
                }
            });
        }

        let pls: Vec<(usize, String, Vec<Track>)> = app.lib.playlists.iter().enumerate().map(|(i, p)| (i, p.name.clone(), p.tracks.iter().take(4).cloned().collect())).collect();
        if !pls.is_empty() {
            ui.add_space(22.0);
            if section_header(ui, I_PLAYLIST, "Tes playlists", true) {
                app.actions.push(Action::Go(View::Playlists));
            }
            ui.add_space(12.0);
            playlist_cards(app, ui, &pls, 1);
        }
        if app.lib.history.is_empty() && app.lib.liked.is_empty() {
            ui.add_space(24.0);
            ui.label(RichText::new("Lance une recherche pour commencer : tes titres écoutés et likés apparaîtront ici.").color(MUTED).size(16.0));
        }
        ui.add_space(20.0);
    });
}

fn quick_card(ui: &mut egui::Ui, rect: Rect, glyph: char, title: &str, sub: &str) -> egui::Response {
    let resp = ui.interact(rect, ui.scope_id().with(("quick", title)), Sense::click());
    let h = hover_t(ui, resp.id, resp.hovered());
    ui.painter().rect_filled(rect, 26.0, lerp(SURFACE, SURFACE_HI, h));
    let c = Pos2::new(rect.left() + 46.0, rect.center().y);
    ui.painter().add(cookie(c, 26.0, 4.0, 0.12, 0.4, CONTAINER));
    ui.painter().text(c, Align2::CENTER_CENTER, glyph.to_string(), icon_font(26.0), PRIMARY);
    let text = Rect::from_min_max(Pos2::new(rect.left() + 90.0, rect.top()), Pos2::new(rect.right() - 46.0, rect.bottom()));
    text_in(ui, text, RichText::new(title).font(bold(18.0)), Some(RichText::new(sub).color(MUTED).size(14.5)));
    ui.painter().text(Pos2::new(rect.right() - 28.0, rect.center().y), Align2::CENTER_CENTER, I_CHEVRON.to_string(), icon_font(24.0), MUTED);
    resp
}

/// Grande carte « En cours » de l'accueil.
fn hero_card(app: &mut App, ui: &mut egui::Ui, p: &Props) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 250.0), Sense::hover());
    let Some(t) = app.current(p).cloned() else {
        ui.painter().rect_filled(rect, 32.0, SURFACE);
        let c = Pos2::new(rect.left() + 120.0, rect.center().y);
        ui.painter().add(cookie(c, 80.0, 9.0, 0.06, 0.0, CONTAINER));
        ui.painter().text(c, Align2::CENTER_CENTER, I_NOTE.to_string(), icon_font(64.0), PRIMARY);
        let text = Rect::from_min_max(Pos2::new(rect.left() + 240.0, rect.top()), rect.max - Vec2::new(24.0, 0.0));
        text_in(ui, text, RichText::new("Rien en lecture").font(heavy(34.0)).color(TEXT), Some(RichText::new("Cherche un titre, un artiste ou colle une playlist YouTube.").size(17.0).color(MUTED)));
        return;
    };
    // Fond : un petit morceau de la pochette étiré sur toute la carte (rendu flou, sans
    // calcul), teinté, dans un rectangle arrondi ; dérive lentement pendant la lecture.
    ui.painter().rect_filled(rect, 32.0, SURFACE);
    if let Some((u, Loaded::Ready(a))) = &app.art {
        if *u == t.url {
            let tt = ui.input(|i| i.time) as f32 * if p.pause { 0.0 } else { 0.03 };
            let c = Pos2::new(0.5 + 0.15 * tt.sin(), 0.45 + 0.15 * (tt * 0.7).cos());
            let uv = Rect::from_center_size(c, Vec2::new(0.16, 0.08));
            ui.painter().add(egui::epaint::RectShape::filled(rect, 32.0, Color32::from_rgb(120, 112, 150)).with_texture(a.tex.id(), uv));
            ui.painter().rect_filled(rect, 32.0, alpha(MAIN, 110));
        }
    }
    let art = Rect::from_min_size(rect.min + Vec2::new(28.0, 28.0), Vec2::splat(194.0));
    match &app.art {
        Some((u, Loaded::Ready(a))) if *u == t.url => {
            ui.painter().rect_filled(art, 22.0, SURFACE_HI);
            ui.put(art, egui::Image::from_texture(egui::load::SizedTexture::from_handle(&a.tex)).fit_to_exact_size(art.size()).corner_radius(22));
        }
        _ => put_square_thumb(ui, art, &t.thumb, 22),
    }
    let resp = ui.interact(art, ui.scope_id().with("hero-art"), Sense::click());
    if resp.on_hover_text("Ouvrir la vue immersive").clicked() {
        app.actions.push(Action::Go(View::NowPlaying));
    }
    let x = art.right() + 32.0;
    let right = rect.right() - 32.0;
    let (title, artist) = display_names(&t);
    let mut job = egui::text::LayoutJob::default();
    job.append("EN COURS", 0.0, egui::TextFormat { font_id: bold(14.0), color: PRIMARY, extra_letter_spacing: 2.0, ..Default::default() });
    ui.painter().add(Shape::galley(Pos2::new(x, art.top() + 4.0), ui.painter().layout_job(job), PRIMARY));
    text_in(ui, Rect::from_min_max(Pos2::new(x, art.top() + 26.0), Pos2::new(right, art.top() + 76.0)), RichText::new(title).font(heavy(38.0)).color(Color32::from_rgb(0xec, 0xe2, 0xff)), None);
    text_in(ui, Rect::from_min_max(Pos2::new(x, art.top() + 78.0), Pos2::new(right, art.top() + 106.0)), RichText::new(artist).size(21.0).color(MUTED), None);

    let controls = Rect::from_min_max(Pos2::new(x, art.top() + 116.0), Pos2::new(right, art.top() + 186.0));
    place(ui, UiBuilder::new().max_rect(controls).layout(Layout::left_to_right(Align::Center)), |ui| {
        let playing = !p.pause;
        let loading = !p.pause && p.time.is_none();
        let glyph = if loading { I_WAIT } else if playing { I_PAUSE } else { I_PLAY };
        if play_button(ui, 66.0, glyph, PRIMARY, ON_PRIMARY, playing).clicked() {
            app.actions.push(Action::TogglePause);
        }
        ui.add_space(8.0);
        ui.label(RichText::new(if playing { "Pause" } else { "Lecture" }).font(bold(18.0)).color(TEXT));
        ui.add_space(14.0);
        let lib = &app.lib;
        let actions = &mut app.actions;
        ui.menu_button(ico(I_MORE, 22.0, MUTED), |ui| track_menu(ui, &t, lib, actions, None));
    });
    place(ui, UiBuilder::new().max_rect(Rect::from_min_max(Pos2::new(x, art.bottom() - 22.0), Pos2::new(right, art.bottom()))), |ui| {
        let (prev, commit) = wavy_progress(ui, right - x, app.seek_drag.unwrap_or(p.time.unwrap_or(0.0)), p.duration.unwrap_or(0.0), !p.pause, PRIMARY, Color32::from_white_alpha(40));
        seek_feedback(app, prev, commit);
    });
}

fn seek_feedback(app: &mut App, preview: Option<f64>, commit: Option<f64>) {
    if let Some(v) = preview {
        app.seek_drag = Some(v);
    }
    if let Some(v) = commit {
        app.actions.push(Action::Seek(v));
        app.seek_drag = None;
    }
}

fn playlist_cards(app: &mut App, ui: &mut egui::Ui, pls: &[(usize, String, Vec<Track>)], max_rows: usize) {
    let card = 200.0;
    let n = ((ui.available_width() + 18.0) / (card + 18.0)).floor().max(1.0) as usize;
    for row in pls.chunks(n).take(max_rows) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            for (i, name, tracks) in row {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(card, card + 40.0), Sense::click());
                let art = Rect::from_min_size(rect.min, Vec2::splat(card));
                collage(ui, art, tracks, 18, I_NOTE);
                let h = hover_t(ui, resp.id, resp.hovered());
                if h > 0.0 {
                    ui.painter().rect_filled(art, 18.0, Color32::from_black_alpha((60.0 * h) as u8));
                }
                text_in(ui, Rect::from_min_max(Pos2::new(rect.left() + 4.0, art.bottom() + 4.0), rect.max), RichText::new(name.as_str()).font(bold(15.5)), None);
                if resp.clicked() {
                    app.actions.push(Action::Go(View::Playlist(*i)));
                }
            }
        });
        ui.add_space(14.0);
    }
}

fn playlists_view(app: &mut App, ui: &mut egui::Ui) {
    back_button(app, ui);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Tes playlists").font(heavy(44.0)).color(PRIMARY));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(200.0, 48.0), Sense::click());
            ui.painter().rect_filled(r, 24.0, if resp.hovered() { CONTAINER } else { SELECT });
            ui.painter().text(Pos2::new(r.left() + 28.0, r.center().y), Align2::CENTER_CENTER, I_ADD.to_string(), icon_font(22.0), TEXT);
            ui.painter().text(Pos2::new(r.left() + 48.0, r.center().y), Align2::LEFT_CENTER, "Nouvelle playlist", bold(15.0), TEXT);
            if resp.clicked() {
                app.actions.push(Action::NewPlaylist(None));
            }
        });
    });
    ui.add_space(16.0);
    let pls: Vec<(usize, String, Vec<Track>)> = app.lib.playlists.iter().enumerate().map(|(i, p)| (i, p.name.clone(), p.tracks.iter().take(4).cloned().collect())).collect();
    if pls.is_empty() {
        ui.label(RichText::new("Aucune playlist pour l'instant : clic droit sur un titre → « Ajouter à une playlist ».").color(MUTED).size(16.0));
        return;
    }
    egui::ScrollArea::vertical().id_salt("pls").auto_shrink(false).show(ui, |ui| {
        playlist_cards(app, ui, &pls, usize::MAX);
    });
}

// ---------------------------------------------------------------------------------
// Recherche et collections
// ---------------------------------------------------------------------------------

fn search_view(app: &mut App, ui: &mut egui::Ui, p: &Props) {
    back_button(app, ui);
    ui.add_space(4.0);
    ui.label(RichText::new("Rechercher").font(heavy(44.0)).color(PRIMARY));
    ui.add_space(10.0);
    egui::Frame::new().fill(SURFACE).corner_radius(30).inner_margin(Margin::symmetric(22, 16)).show(ui, |ui| {
        ui.set_width(ui.available_width().min(720.0));
        ui.horizontal(|ui| {
            ui.label(ico(I_SEARCH, 24.0, PRIMARY));
            ui.add_space(4.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut app.query)
                    .frame(egui::Frame::NONE)
                    .font(FontId::proportional(18.0))
                    .hint_text("Titre, artiste… ou URL de playlist YouTube")
                    .desired_width(f32::INFINITY),
            );
            if std::mem::take(&mut app.focus_search) {
                resp.request_focus();
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                app.actions.push(Action::Search(app.query.clone()));
            }
        });
    });
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        let title = if app.results_query.is_empty() {
            String::new()
        } else if app.results_query.starts_with("http") {
            "Playlist importée".to_string()
        } else {
            format!("Résultats pour « {} »", app.results_query)
        };
        ui.add(egui::Label::new(RichText::new(title).font(heavy(24.0))).truncate());
        if !app.results.is_empty() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if play_button(ui, 52.0, I_PLAY, PRIMARY, ON_PRIMARY, false).on_hover_text("Tout lire").clicked() {
                    app.actions.push(Action::Play { tracks: app.results.clone(), start: 0 });
                }
            });
        }
    });
    match &app.search_status {
        SearchStatus::Idle => {}
        SearchStatus::Searching => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("Recherche…").color(MUTED));
            });
        }
        SearchStatus::Error(e) => {
            ui.add(egui::Label::new(RichText::new(e).color(ERROR)).wrap());
        }
        SearchStatus::Done => {}
    }
    ui.add_space(6.0);
    let current = app.current(p).map(|t| t.url.clone());
    track_list(ui, "results", &app.results, &app.lib, &mut app.actions, current.as_deref(), None);
}

#[derive(Clone, Copy)]
enum Coll {
    Liked,
    History,
    Playlist(usize),
}

fn collection(app: &mut App, ui: &mut egui::Ui, p: &Props, coll: Coll) {
    if let Coll::Playlist(i) = coll {
        if i >= app.lib.playlists.len() {
            app.view = View::Home;
            return;
        }
    }
    let tracks: Vec<Track> = match coll {
        Coll::Liked => app.lib.liked.clone(),
        Coll::History => app.lib.history.clone(),
        Coll::Playlist(i) => app.lib.playlists[i].tracks.clone(),
    };
    let total: f64 = tracks.iter().filter_map(|t| t.duration).sum();
    back_button(app, ui);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let (tile, _) = ui.allocate_exact_size(Vec2::splat(170.0), Sense::hover());
        match coll {
            Coll::Liked => {
                ui.painter().rect_filled(tile, 32.0, CONTAINER);
                ui.painter().add(cookie(tile.center(), 62.0, 4.0, 0.14, 0.4, PRIMARY));
                ui.painter().text(tile.center(), Align2::CENTER_CENTER, I_HEART_FILL.to_string(), icon_font(58.0), ON_PRIMARY);
            }
            Coll::History => {
                ui.painter().rect_filled(tile, 32.0, CONTAINER);
                ui.painter().add(cookie(tile.center(), 62.0, 9.0, 0.06, 0.0, PRIMARY));
                ui.painter().text(tile.center(), Align2::CENTER_CENTER, I_HISTORY.to_string(), icon_font(58.0), ON_PRIMARY);
            }
            Coll::Playlist(_) => collage(ui, tile, &tracks, 28, I_NOTE),
        }
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.add_space(26.0);
            ui.label(RichText::new(match coll { Coll::History => "Historique", _ => "Playlist" }).color(MUTED).size(14.0));
            match coll {
                Coll::Playlist(i) => {
                    let name = &mut app.lib.playlists[i].name;
                    let r = ui.add(egui::TextEdit::singleline(name).font(heavy(42.0)).text_color(PRIMARY).frame(egui::Frame::NONE).desired_width(ui.available_width() - 20.0));
                    if r.lost_focus() {
                        app.lib.save();
                    }
                    r.on_hover_text("Cliquer pour renommer");
                }
                Coll::Liked => {
                    ui.label(RichText::new("Titres likés").font(heavy(42.0)).color(PRIMARY));
                }
                Coll::History => {
                    ui.label(RichText::new("Écoutés récemment").font(heavy(42.0)).color(PRIMARY));
                }
            }
            ui.label(RichText::new(format!("{} · {}", titres(tracks.len()), fmt_time(Some(total)))).color(MUTED).size(15.0));
        });
    });
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        if play_button(ui, 62.0, I_PLAY, PRIMARY, ON_PRIMARY, false).on_hover_text("Lire").clicked() && !tracks.is_empty() {
            app.actions.push(Action::Play { tracks: tracks.clone(), start: 0 });
        }
        let sh = app.lib.shuffle;
        if icon_btn(ui, I_SHUFFLE, 24.0, if sh { ON_PRIMARY } else { TEXT }, Some(if sh { PRIMARY } else { SURFACE }), "Lecture aléatoire").clicked() {
            app.actions.push(Action::ToggleShuffle);
        }
        if let Coll::Playlist(i) = coll {
            if icon_btn(ui, I_DELETE, 22.0, TEXT, Some(SURFACE), "Supprimer la playlist").clicked() {
                app.actions.push(Action::DeletePlaylist(i));
            }
        }
    });
    ui.add_space(6.0);
    if tracks.is_empty() {
        let hint = match coll {
            Coll::Playlist(_) => "Playlist vide : clic droit sur un titre → « Ajouter à une playlist ».",
            Coll::Liked => "Aucun titre liké : clique sur le cœur d'un titre pour l'ajouter ici.",
            Coll::History => "Rien d'écouté pour l'instant.",
        };
        ui.add_space(12.0);
        ui.label(RichText::new(hint).color(MUTED).size(16.0));
        return;
    }
    let current = app.current(p).map(|t| t.url.clone());
    let (id, pl) = match coll {
        Coll::Liked => ("liked".to_string(), None),
        Coll::History => ("history".to_string(), None),
        Coll::Playlist(i) => (format!("pl-{i}"), Some(i)),
    };
    track_list(ui, &id, &tracks, &app.lib, &mut app.actions, current.as_deref(), pl);
}

// ---------------------------------------------------------------------------------
// Panneau de droite : paroles ou file d'attente
// ---------------------------------------------------------------------------------

fn right_pane(app: &mut App, ui: &mut egui::Ui, p: &Props) {
    egui::Panel::right("right")
        .exact_size(380.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(BG).inner_margin(Margin { left: 0, right: 12, top: 12, bottom: 12 }))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            ui.painter().rect_filled(rect, 28.0, MAIN);
            let inner = rect.shrink2(Vec2::new(26.0, 24.0));
            place(ui, UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)), |ui| match app.right {
                RightPane::Lyrics => {
                    let track = app.current(p).cloned();
                    match track {
                        Some(t) => {
                            let footer = 26.0;
                            let area = Rect::from_min_max(ui.max_rect().min, ui.max_rect().max - Vec2::new(0.0, footer));
                            place(ui, UiBuilder::new().max_rect(area).layout(Layout::top_down(Align::Min)), |ui| {
                                lyrics_view(app, ui, p, &t, LyricsStyle::Pane);
                            });
                            ui.painter().text(Pos2::new(inner.left(), inner.bottom() - 6.0), Align2::LEFT_BOTTOM, "Paroles : LRCLIB", FontId::proportional(13.0), FAINT);
                        }
                        None => {
                            ui.label(RichText::new("Les paroles du titre en cours s'afficheront ici.").color(MUTED).size(16.0));
                        }
                    }
                }
                RightPane::Queue => queue_list(app, ui, p, false),
                RightPane::None => {}
            });
        });
}

#[derive(PartialEq)]
enum LyricsStyle {
    Pane,
    Immersive,
}

fn lyrics_view(app: &mut App, ui: &mut egui::Ui, p: &Props, track: &Track, style: LyricsStyle) {
    let imm = style == LyricsStyle::Immersive;
    let (size, dim, past) = if imm {
        (32.0, Color32::from_white_alpha(95), Color32::from_white_alpha(120))
    } else {
        (25.0, FAINT, alpha(MUTED, 170))
    };
    match app.lyrics.get(&track.url) {
        None | Some(Loaded::Loading) => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("Recherche des paroles…").color(dim).size(16.0));
            });
        }
        Some(Loaded::Missing) => {
            ui.add_space(ui.available_height() * 0.35);
            ui.label(RichText::new("Paroles introuvables pour ce titre.").font(bold(size * 0.8)).color(dim));
        }
        Some(Loaded::Ready(Lyrics::Instrumental)) => {
            ui.add_space(ui.available_height() * 0.35);
            ui.horizontal(|ui| {
                ui.label(ico(I_NOTE, size, dim));
                ui.label(RichText::new("Instrumental").font(heavy(size)).color(dim));
            });
        }
        Some(Loaded::Ready(Lyrics::Plain(text))) => {
            ui.label(RichText::new("Paroles non synchronisées").size(13.0).color(dim));
            egui::ScrollArea::vertical().id_salt(("lyrics-plain", imm)).auto_shrink(false).show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(text).font(bold(size * 0.75)).color(if imm { Color32::from_white_alpha(220) } else { TEXT })).wrap());
            });
        }
        Some(Loaded::Ready(Lyrics::Synced(lines))) => {
            let time = p.time.unwrap_or(0.0);
            let cur = lyrics::current_line(lines, time);
            // Mémorisé séparément pour le panneau et la vue immersive : chacun défile seul.
            let key = ui.scope_id().with(("lyrics-line", imm, &track.url));
            let last: Option<Option<usize>> = ui.data(|d| d.get_temp(key));
            let changed = last != Some(cur);
            ui.data_mut(|d| d.insert_temp(key, cur));
            let glow = match &app.art {
                Some((_, Loaded::Ready(a))) => a.light,
                _ => PRIMARY,
            };
            let mut seek = None;
            if !p.pause {
                ui.ctx().request_repaint_after(Duration::from_millis(150));
            }
            egui::ScrollArea::vertical().id_salt(("lyrics", imm)).auto_shrink(false).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.add_space(ui.available_height() * 0.32);
                ui.spacing_mut().item_spacing.y = if imm { 22.0 } else { 18.0 };
                for (i, (start, text)) in lines.iter().enumerate() {
                    let instrumental = text.trim().is_empty();
                    let is_cur = cur == Some(i);
                    let color = if is_cur {
                        if imm { Color32::WHITE } else { TEXT }
                    } else if cur.is_some_and(|c| i < c) {
                        past
                    } else {
                        dim
                    };
                    // La ligne en cours est un peu plus grande, et entourée d'un halo en vue immersive.
                    let font = heavy(if is_cur { size * 1.06 } else { size });
                    // Pause instrumentale : une note (police d'icônes, la police de texte n'a pas ♪).
                    let rich = if instrumental { ico(I_NOTE, size, color) } else { RichText::new(text).font(font).color(color) };
                    let label = egui::Label::new(rich).wrap().sense(Sense::click());
                    let r = if is_cur && imm {
                        // Halo dessiné *sous* le texte : on réserve la place avant d'ajouter le libellé.
                        let slot = ui.painter().add(Shape::Noop);
                        let resp = ui.add(label);
                        let bg = resp.rect.expand2(Vec2::new(20.0, 12.0));
                        let glow_shapes: Vec<Shape> = (0..6)
                            .rev()
                            .map(|k| Shape::rect_filled(bg.expand(k as f32 * 3.0), 28.0 + k as f32 * 3.0, alpha(glow, (8 + (6 - k) * 5) as u8)))
                            .collect();
                        ui.painter().set(slot, Shape::Vec(glow_shapes));
                        resp
                    } else {
                        ui.add(label)
                    };
                    if r.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if r.clicked() {
                        seek = Some(*start);
                    }
                    if changed && is_cur {
                        r.scroll_to_me(Some(Align::Center));
                    }
                }
                ui.add_space(ui.available_height() * 0.5);
            });
            if let Some(s) = seek {
                app.actions.push(Action::Seek(s));
            }
        }
    }
}

fn queue_list(app: &mut App, ui: &mut egui::Ui, p: &Props, imm: bool) {
    let (title_c, sub_c, hover) = if imm {
        (Color32::WHITE, Color32::from_white_alpha(170), Color32::from_white_alpha(25))
    } else {
        (TEXT, MUTED, SURFACE)
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new("File d'attente").font(heavy(24.0)).color(title_c));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(78.0, 34.0), Sense::click());
            ui.painter().rect_filled(r, 17.0, if resp.hovered() { hover } else { Color32::TRANSPARENT });
            ui.painter().text(r.center(), Align2::CENTER_CENTER, "Vider", bold(14.0), sub_c);
            if resp.clicked() {
                app.actions.push(Action::QueueClear);
            }
        });
    });
    ui.add_space(8.0);
    let start = usize::try_from(p.pos).unwrap_or(0);
    let n = app.queue.len().saturating_sub(start);
    if n == 0 {
        ui.label(RichText::new("La file est vide.").color(sub_c));
        return;
    }
    egui::ScrollArea::vertical().id_salt(("queue", imm)).auto_shrink(false).show_rows(ui, 60.0, n, |ui, range| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for k in range {
            let i = start + k;
            let t = &app.queue[i];
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 60.0), Sense::click());
            let hovered = ui.rect_contains_pointer(rect);
            let h = hover_t(ui, resp.id, hovered);
            if h > 0.0 {
                ui.painter().rect_filled(rect, 16.0, alpha(hover, (hover.a() as f32 * h) as u8));
            }
            let art = Rect::from_min_size(rect.min + Vec2::new(6.0, 8.0), Vec2::splat(44.0));
            put_square_thumb(ui, art, &t.thumb, 12);
            let text = Rect::from_min_max(Pos2::new(art.right() + 12.0, rect.top()), Pos2::new(rect.right() - 40.0, rect.bottom()));
            let (title, artist) = display_names(t);
            let is_cur = i as i64 == p.pos;
            let color = if is_cur { if imm { Color32::WHITE } else { PRIMARY } } else { title_c };
            let text = if is_cur {
                // Titre en cours : petite icône (la police n'a pas ▶).
                ui.painter().text(Pos2::new(text.left() + 9.0, rect.center().y), Align2::CENTER_CENTER, I_GRAPH.to_string(), icon_font(18.0), if imm { Color32::WHITE } else { PRIMARY });
                Rect::from_min_max(Pos2::new(text.left() + 24.0, text.top()), text.max)
            } else {
                text
            };
            text_in(ui, text, RichText::new(title).font(bold(15.0)).color(color), Some(RichText::new(artist).color(sub_c).size(13.0)));
            if hovered && k > 0 {
                let r = Rect::from_center_size(Pos2::new(rect.right() - 22.0, rect.center().y), Vec2::splat(32.0));
                if ui.put(r, egui::Button::new(ico(I_CLOSE, 18.0, sub_c)).frame(false)).on_hover_text("Retirer").clicked() {
                    app.actions.push(Action::QueueRemove(i));
                }
            }
            if resp.clicked() {
                app.actions.push(Action::QueuePlay(i));
            }
        }
    });
}

// ---------------------------------------------------------------------------------
// Barre de lecture flottante
// ---------------------------------------------------------------------------------

fn player_bar(app: &mut App, ui: &mut egui::Ui, p: &Props, rect: Rect) {
    // Ombre douce puis pilule.
    for k in 1..=4 {
        ui.painter().rect_filled(rect.expand(k as f32 * 2.0).translate(Vec2::new(0.0, 3.0)), 40.0 + k as f32 * 2.0, Color32::from_black_alpha(14));
    }
    ui.painter().rect_filled(rect, 40.0, BAR);
    let inner = rect.shrink2(Vec2::new(18.0, 10.0));
    let current = app.current(p).cloned();
    let compact = inner.width() < 820.0;
    place(ui, UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let sh = app.lib.shuffle;
        if !compact && icon_btn(ui, I_SHUFFLE, 22.0, if sh { PRIMARY } else { MUTED }, None, "Lecture aléatoire").clicked() {
            app.actions.push(Action::ToggleShuffle);
        }
        if icon_btn(ui, I_PREV, 26.0, TEXT, None, "Précédent (Ctrl+←)").clicked() {
            app.actions.push(Action::Prev);
        }
        let playing = p.pos >= 0 && !p.pause;
        let loading = playing && p.time.is_none();
        let glyph = if loading { I_WAIT } else if playing { I_PAUSE } else { I_PLAY };
        if play_button(ui, 56.0, glyph, PRIMARY, ON_PRIMARY, playing).on_hover_text("Lecture/Pause (Espace)").clicked() {
            app.actions.push(Action::TogglePause);
        }
        if icon_btn(ui, I_NEXT, 26.0, TEXT, None, "Suivant (Ctrl+→)").clicked() {
            app.actions.push(Action::Next);
        }
        let (glyph, color, tip) = match app.lib.repeat {
            Repeat::Off => (I_REPEAT, MUTED, "Répéter : non"),
            Repeat::All => (I_REPEAT, PRIMARY, "Répéter : tout"),
            Repeat::One => (I_REPEAT_ONE, PRIMARY, "Répéter : ce titre"),
        };
        if !compact && icon_btn(ui, glyph, 22.0, color, None, tip).clicked() {
            app.actions.push(Action::CycleRepeat);
        }
        ui.add_space(if compact { 6.0 } else { 14.0 });

        // Droite d'abord (largeur fixe), puis le titre + la vague prennent le reste.
        let right_w = 220.0;
        let mid_w = (ui.available_width() - right_w - 8.0).max(120.0);
        ui.allocate_ui_with_layout(Vec2::new(mid_w, 60.0), Layout::left_to_right(Align::Center), |ui| {
            if let Some(t) = &current {
                let (art, art_resp) = ui.allocate_exact_size(Vec2::splat(50.0), Sense::click());
                match &app.art {
                    Some((u, Loaded::Ready(a))) if *u == t.url => {
                        ui.put(art, egui::Image::from_texture(egui::load::SizedTexture::from_handle(&a.tex)).fit_to_exact_size(art.size()).corner_radius(12));
                    }
                    _ => put_square_thumb(ui, art, &t.thumb, 12),
                }
                if art_resp.on_hover_text("Vue immersive").clicked() {
                    app.actions.push(Action::Go(View::NowPlaying));
                }
                ui.add_space(8.0);
                let col_w = ui.available_width() - 40.0;
                let (title, artist) = display_names(t);
                ui.allocate_ui_with_layout(Vec2::new(col_w, 60.0), Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add_space(2.0);
                    ui.add(egui::Label::new(RichText::new(title).font(heavy(16.5))).truncate().selectable(false));
                    ui.add(egui::Label::new(RichText::new(artist).color(MUTED).size(13.5)).truncate().selectable(false));
                    let pos = app.seek_drag.unwrap_or(p.time.unwrap_or(0.0));
                    let (prev, commit) = wavy_progress(ui, col_w.min(360.0), pos, p.duration.unwrap_or(0.0), playing, PRIMARY, Color32::from_white_alpha(45));
                    seek_feedback(app, prev, commit);
                });
                let lib = &app.lib;
                let actions = &mut app.actions;
                ui.menu_button(ico(I_MORE, 22.0, MUTED), |ui| track_menu(ui, t, lib, actions, None));
            } else {
                ui.label(RichText::new("Rien en lecture").color(MUTED).size(15.0));
            }
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let muted = app.muted_volume.is_some() || app.volume < 0.5;
            if let Some(v) = thin_slider(ui, 70.0, (app.volume / 100.0) as f32, PRIMARY, Color32::from_white_alpha(45), 5.0) {
                app.muted_volume = None;
                app.actions.push(Action::SetVolume((v * 100.0).round() as f64));
            }
            if icon_btn(ui, if muted { I_MUTE } else { I_VOLUME }, 22.0, MUTED, None, if muted { "Rétablir le son" } else { "Couper le son" }).clicked() {
                toggle_mute(app);
            }
            // Sans place pour le panneau de droite, paroles et file s'ouvrent en vue immersive.
            for (pane, tab, glyph, tip) in [(RightPane::Queue, ImmTab::Queue, I_QUEUE, "File d'attente"), (RightPane::Lyrics, ImmTab::Lyrics, I_LYRICS, "Paroles")] {
                let on = app.right_fits && app.right == pane;
                if icon_btn(ui, glyph, 22.0, if on { PRIMARY } else { MUTED }, None, tip).clicked() {
                    if app.right_fits {
                        app.right = if on { RightPane::None } else { pane };
                    } else {
                        app.right = pane;
                        app.imm_tab = tab;
                        app.actions.push(Action::Go(View::NowPlaying));
                    }
                }
            }
        });
    });
}

fn toggle_mute(app: &mut App) {
    match app.muted_volume.take() {
        Some(v) => app.actions.push(Action::SetVolume(v)),
        None => {
            app.muted_volume = Some(app.volume.max(5.0));
            app.actions.push(Action::SetVolume(0.0));
        }
    }
}

// ---------------------------------------------------------------------------------
// Vue immersive
// ---------------------------------------------------------------------------------

fn liquid_background(ui: &egui::Ui, rect: Rect, colors: [Color32; 4], light: Color32, tex: Option<egui::TextureId>, t: f32) {
    let painter = ui.painter().with_clip_rect(rect);
    let base = lerp(lerp(colors[0], colors[3], 0.5), Color32::from_rgb(0x12, 0x12, 0x1c), 0.35);
    painter.rect_filled(rect, 0.0, base);
    if let Some(tex) = tex {
        // Pochette très agrandie qui dérive : texture « liquide » tirée de l'image.
        let c = Pos2::new(0.5 + 0.2 * (t * 0.045).sin(), 0.5 + 0.2 * (t * 0.031).cos());
        let uv = Rect::from_center_size(c, Vec2::new(0.13, 0.13 * rect.height() / rect.width()));
        painter.add(egui::epaint::RectShape::filled(rect, 0.0, Color32::from_gray(150)).with_texture(tex, uv));
    }
    let (w, h) = (rect.width(), rect.height());
    let big = w.max(h) * 0.62;
    // Taches de couleur qui dérivent lentement (fond « liquide »).
    let specs = [(0.2, 0.25, 0.11, 0.07, colors[0]), (0.8, 0.2, 0.09, 0.12, colors[1]), (0.25, 0.85, 0.13, 0.08, colors[2]), (0.85, 0.8, 0.07, 0.1, colors[3]), (0.5, 0.5, 0.05, 0.06, alpha(light, 90))];
    for (k, (fx, fy, sx, sy, c)) in specs.into_iter().enumerate() {
        let kf = k as f32;
        let x = rect.left() + w * (fx + 0.12 * (t * sx + kf).sin());
        let y = rect.top() + h * (fy + 0.12 * (t * sy + kf * 1.7).cos());
        let a = if k == 4 { c.a() } else if tex.is_some() { 150 } else { 230 };
        painter.add(blob(Pos2::new(x, y), big * (0.8 + 0.15 * (t * 0.2 + kf).sin()), alpha(c, a)));
    }
    // Fleurs décoratives très discrètes (formes festonnées).
    for (k, (fx, fy, r)) in [(0.08, 0.85, 0.22), (0.92, 0.18, 0.18), (0.62, 0.95, 0.16), (0.4, 0.05, 0.12)].into_iter().enumerate() {
        let c = Pos2::new(rect.left() + w * fx, rect.top() + h * fy);
        painter.add(cookie(c, h * r, 8.0, 0.22, t * 0.05 * if k % 2 == 0 { 1.0 } else { -1.0 }, Color32::from_white_alpha(9)));
    }
    painter.rect_filled(rect, 0.0, Color32::from_black_alpha(55));
}

fn immersive(app: &mut App, ui: &mut egui::Ui, p: &Props, rect: Rect) {
    let t = ui.input(|i| i.time) as f32;
    let (colors, light, tex) = match &app.art {
        Some((_, Loaded::Ready(a))) => (a.colors, a.light, Some(a.tex.id())),
        _ => ([Color32::from_rgb(0x3a, 0x2f, 0x6b), Color32::from_rgb(0x2a, 0x24, 0x52), Color32::from_rgb(0x1f, 0x2a, 0x55), Color32::from_rgb(0x30, 0x22, 0x48)], PRIMARY, None),
    };
    let playing = p.pos >= 0 && !p.pause;
    liquid_background(ui, rect, colors, light, tex, t);
    if playing {
        ui.ctx().request_repaint_after(Duration::from_millis(33));
    }

    // Fermer (haut gauche)
    // Fermer la vue immersive : à droite des boutons de fenêtre.
    let close = Rect::from_center_size(rect.left_top() + Vec2::new(178.0, 44.0), Vec2::splat(44.0));
    let close_resp = ui.interact(close, ui.scope_id().with("imm-close"), Sense::click());
    let h = hover_t(ui, close_resp.id, close_resp.hovered());
    ui.painter().circle_filled(close.center(), 22.0, Color32::from_white_alpha((35.0 + 30.0 * h) as u8));
    ui.painter().text(close.center(), Align2::CENTER_CENTER, I_CLOSE.to_string(), icon_font(22.0), Color32::WHITE);
    if close_resp.on_hover_text("Fermer (Échap)").clicked() {
        app.view = app.back.pop().unwrap_or(View::Home);
    }

    // Barre d'outils segmentée (haut droite)
    let tabs = [(ImmTab::Art, I_IMAGE, "Pochette"), (ImmTab::Lyrics, I_LYRICS, "Paroles"), (ImmTab::Queue, I_QUEUE, "File d'attente"), (ImmTab::Sleep, I_BEDTIME, "Minuterie de veille"), (ImmTab::Eq, I_TUNE, "Égaliseur et son")];
    let bar_w = tabs.len() as f32 * 62.0 + 12.0;
    let bar = Rect::from_min_size(Pos2::new(rect.right() - bar_w - 28.0, rect.top() + 22.0), Vec2::new(bar_w, 56.0));
    ui.painter().rect_filled(bar, 28.0, Color32::from_white_alpha(28));
    for (k, (tab, glyph, tip)) in tabs.into_iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(bar.left() + 6.0 + k as f32 * 62.0, bar.top() + 6.0), Vec2::new(62.0, 44.0));
        let resp = ui.interact(r, ui.scope_id().with(("imm-tab", k)), Sense::click());
        let sel = app.imm_tab == tab;
        let st = ui.ctx().animate_bool_with_time(resp.id.with("sel"), sel, 0.25);
        let h = hover_t(ui, resp.id, resp.hovered());
        if st > 0.0 {
            ui.painter().rect_filled(Rect::from_center_size(r.center(), r.size() * (0.7 + 0.3 * st)), 22.0, alpha(Color32::WHITE, (255.0 * st) as u8));
        }
        if h > 0.0 && !sel {
            ui.painter().rect_filled(r, 22.0, Color32::from_white_alpha((30.0 * h) as u8));
        }
        // La couleur suit l'animation de la pilule (sinon blanc sur blanc pendant le fondu).
        let mut color = lerp(Color32::WHITE, Color32::from_rgb(0x20, 0x1c, 0x30), st);
        if tab == ImmTab::Sleep && app.sleep.is_some() && !sel {
            color = light;
        }
        ui.painter().text(r.center(), Align2::CENTER_CENTER, glyph.to_string(), icon_font(24.0), color);
        if resp.on_hover_text(tip).clicked() {
            app.imm_tab = tab;
        }
    }

    let content = Rect::from_min_max(rect.min + Vec2::new(60.0, 100.0), rect.max - Vec2::new(60.0, 40.0));
    let art_only = app.imm_tab == ImmTab::Art;
    let left = if art_only {
        Rect::from_center_size(content.center(), Vec2::new(content.width().min(560.0), content.height()))
    } else {
        Rect::from_min_max(content.min, Pos2::new(content.left() + content.width() * 0.42, content.bottom()))
    };
    player_column(app, ui, p, left, light, art_only, t);
    if !art_only {
        let right = Rect::from_min_max(Pos2::new(left.right() + 60.0, content.top()), content.max);
        place(ui, UiBuilder::new().max_rect(right).layout(Layout::top_down(Align::Min)), |ui| match app.imm_tab {
            ImmTab::Lyrics => match app.current(p).cloned() {
                Some(track) => lyrics_view(app, ui, p, &track, LyricsStyle::Immersive),
                None => {
                    ui.label(RichText::new("Rien en lecture.").color(Color32::from_white_alpha(150)).size(20.0));
                }
            },
            ImmTab::Queue => glass_card(ui, ui.available_height(), |ui| queue_list(app, ui, p, true)),
            ImmTab::Sleep => sleep_panel(app, ui),
            ImmTab::Eq => eq_panel(app, ui),
            ImmTab::Art => {}
        });
    }
}

fn glass_card(ui: &mut egui::Ui, min_h: f32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(Color32::from_white_alpha(22)).corner_radius(30).inner_margin(Margin::same(24)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(min_h - 48.0);
        add(ui);
    });
}

/// Colonne gauche de la vue immersive : pochette festonnée + halo, titre, contrôles.
fn player_column(app: &mut App, ui: &mut egui::Ui, p: &Props, area: Rect, light: Color32, big: bool, t: f32) {
    let Some(track) = app.current(p).cloned() else {
        ui.painter().text(area.center(), Align2::CENTER_CENTER, "Rien en lecture", heavy(34.0), Color32::WHITE);
        return;
    };
    let playing = !p.pause && p.pos >= 0;
    // Hauteur totale = 20 + 2,57 × rayon (pochette + halo) + ~340 (titre, progression,
    // contrôles, volume) : le rayon est borné pour que tout tienne à l'écran.
    let art_r = ((area.height() - 360.0) / 2.57).min(area.width() / 2.0 - 40.0).min(if big { 230.0 } else { 175.0 }).max(60.0);
    let c = Pos2::new(area.center().x, area.top() + 20.0 + art_r * 1.25);

    // Halo de barres radiales (animation décorative, pas une analyse du son).
    let bars = 72;
    for k in 0..bars {
        let a = k as f32 / bars as f32 * TAU;
        let v = if playing { wobble(k as f32, t) } else { 0.25 };
        let r0 = art_r * 1.06;
        let r1 = art_r * (1.12 + 0.16 * v);
        let dir = Vec2::new(a.cos(), a.sin());
        ui.painter().line_segment([c + dir * r0, c + dir * r1], Stroke::new(5.0, alpha(lerp(light, Color32::WHITE, 0.25), 210)));
    }
    let rot = if playing { t * 0.06 } else { 0.0 };
    ui.painter().add(cookie(c, art_r * 1.02, 10.0, 0.045, rot, alpha(lerp(light, Color32::WHITE, 0.4), 230)));
    match &app.art {
        Some((u, Loaded::Ready(a))) if *u == track.url => {
            ui.painter().add(cookie_image(c, art_r * 0.96, 10.0, 0.045, rot, a.tex.id(), alpha(lerp(light, Color32::WHITE, 0.4), 230)));
        }
        _ => {
            ui.painter().add(cookie(c, art_r * 0.96, 10.0, 0.045, rot, Color32::from_white_alpha(40)));
            ui.put(Rect::from_center_size(c, Vec2::splat(36.0)), egui::Spinner::new().color(Color32::WHITE));
        }
    }

    let below = Rect::from_min_max(Pos2::new(area.left(), c.y + art_r * 1.32), area.max);
    place(ui, UiBuilder::new().max_rect(below).layout(Layout::top_down(Align::Center)), |ui| {
        let (title, artist) = display_names(&track);
        ui.add(egui::Label::new(RichText::new(title).font(heavy(36.0)).color(Color32::WHITE)).truncate().selectable(false)).on_hover_text(&track.title);
        ui.horizontal(|ui| {
            let w = ui.painter().layout_no_wrap(artist.clone(), FontId::proportional(21.0), Color32::WHITE).size().x + 46.0;
            ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
            ui.label(RichText::new(artist).size(21.0).color(Color32::from_white_alpha(185)));
            let liked = app.lib.is_liked(&track.url);
            let lib = &app.lib;
            let actions = &mut app.actions;
            ui.menu_button(ico(I_MORE, 22.0, Color32::from_white_alpha(185)), |ui| track_menu(ui, &track, lib, actions, None));
            let _ = liked;
        });
        ui.add_space(14.0);

        // Progression : temps écoulé, vague, temps restant.
        let width = area.width().min(480.0);
        ui.allocate_ui_with_layout(Vec2::new(width, 26.0), Layout::left_to_right(Align::Center), |ui| {
            let dur = p.duration.unwrap_or(0.0);
            let pos = app.seek_drag.unwrap_or(p.time.unwrap_or(0.0));
            ui.label(RichText::new(fmt_time(Some(pos))).size(14.0).color(Color32::from_white_alpha(200)));
            let (prev, commit) = wavy_progress(ui, width - 110.0, pos, dur, playing, Color32::WHITE, Color32::from_white_alpha(60));
            seek_feedback(app, prev, commit);
            ui.label(RichText::new(format!("-{}", fmt_time(Some((dur - pos).max(0.0))))).size(14.0).color(Color32::from_white_alpha(200)));
        });
        ui.add_space(14.0);

        ui.allocate_ui_with_layout(Vec2::new(5.0 * 64.0 + 40.0, 84.0), Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            let sh = app.lib.shuffle;
            if icon_btn(ui, I_SHUFFLE, 28.0, Color32::WHITE, if sh { Some(Color32::from_white_alpha(55)) } else { None }, "Lecture aléatoire").clicked() {
                app.actions.push(Action::ToggleShuffle);
            }
            if icon_btn(ui, I_PREV, 34.0, Color32::WHITE, None, "Précédent").clicked() {
                app.actions.push(Action::Prev);
            }
            let loading = playing && p.time.is_none();
            let glyph = if loading { I_WAIT } else if playing { I_PAUSE } else { I_PLAY };
            if play_button(ui, 80.0, glyph, Color32::WHITE, Color32::from_rgb(0x1c, 0x1a, 0x28), playing).clicked() {
                app.actions.push(Action::TogglePause);
            }
            if icon_btn(ui, I_NEXT, 34.0, Color32::WHITE, None, "Suivant").clicked() {
                app.actions.push(Action::Next);
            }
            let (glyph, bg) = match app.lib.repeat {
                Repeat::Off => (I_REPEAT, None),
                Repeat::All => (I_REPEAT, Some(Color32::from_white_alpha(55))),
                Repeat::One => (I_REPEAT_ONE, Some(Color32::from_white_alpha(55))),
            };
            if icon_btn(ui, glyph, 28.0, Color32::WHITE, bg, "Répéter").clicked() {
                app.actions.push(Action::CycleRepeat);
            }
        });
        ui.add_space(14.0);
        ui.allocate_ui_with_layout(Vec2::new(width, 30.0), Layout::left_to_right(Align::Center), |ui| {
            let muted = app.muted_volume.is_some() || app.volume < 0.5;
            if icon_btn(ui, if muted { I_MUTE } else { I_VOLUME }, 22.0, Color32::WHITE, None, "Couper le son").clicked() {
                toggle_mute(app);
            }
            if let Some(v) = thin_slider(ui, width - 56.0, (app.volume / 100.0) as f32, Color32::WHITE, Color32::from_white_alpha(70), 8.0) {
                app.muted_volume = None;
                app.actions.push(Action::SetVolume((v * 100.0).round() as f64));
            }
        });
    });
}

fn sleep_panel(app: &mut App, ui: &mut egui::Ui) {
    glass_card(ui, 0.0, |ui| {
        ui.label(RichText::new("Minuterie de veille").font(heavy(26.0)).color(Color32::WHITE));
        ui.add_space(4.0);
        let status = match app.sleep {
            None => "Désactivée".to_string(),
            Some(Sleep::EndOfTrack(_)) => "Pause à la fin du titre en cours".to_string(),
            Some(Sleep::At(at, _)) => {
                let left = at.saturating_duration_since(Instant::now()).as_secs();
                format!("Pause dans {}:{:02}", left / 60, left % 60)
            }
        };
        ui.label(RichText::new(status).size(17.0).color(Color32::from_white_alpha(190)));
        ui.add_space(18.0);
        let choices: [(&str, Option<u64>); 6] = [("Désactivée", None), ("15 min", Some(15)), ("30 min", Some(30)), ("45 min", Some(45)), ("1 h", Some(60)), ("Fin du titre", Some(0))];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
            for (label, v) in choices {
                let sel = match (app.sleep, v) {
                    (None, None) => true,
                    (Some(Sleep::EndOfTrack(_)), Some(0)) => true,
                    (Some(Sleep::At(_, m)), Some(x)) => m == x,
                    _ => false,
                };
                let galley_w = ui.painter().layout_no_wrap(label.to_string(), bold(16.0), Color32::WHITE).size().x;
                let (r, resp) = ui.allocate_exact_size(Vec2::new(galley_w + 40.0, 48.0), Sense::click());
                ui.painter().rect_filled(r, 24.0, if sel { Color32::WHITE } else if resp.hovered() { Color32::from_white_alpha(45) } else { Color32::from_white_alpha(25) });
                ui.painter().text(r.center(), Align2::CENTER_CENTER, label, bold(16.0), if sel { Color32::from_rgb(0x20, 0x1c, 0x30) } else { Color32::WHITE });
                if resp.clicked() {
                    app.actions.push(Action::SetSleep(v));
                }
            }
        });
    });
}

fn eq_panel(app: &mut App, ui: &mut egui::Ui) {
    let white = Color32::WHITE;
    let soft = Color32::from_white_alpha(180);
    glass_card(ui, 0.0, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Normalisation du volume").font(heavy(22.0)).color(white));
                ui.label(RichText::new("Lisse les écarts de volume entre les titres").size(14.5).color(soft));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if switch(ui, app.lib.normalize).clicked() {
                    app.actions.push(Action::ToggleNormalize);
                }
            });
        });
    });
    ui.add_space(18.0);
    glass_card(ui, 0.0, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Égaliseur").font(heavy(22.0)).color(white));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if switch(ui, app.lib.eq_on).clicked() {
                    app.actions.push(Action::ToggleEq);
                }
                ui.add_space(10.0);
                let mut chosen = None;
                egui::ComboBox::from_id_salt("eq-preset").selected_text(RichText::new(&app.lib.eq_preset).font(bold(15.0)).color(white)).width(150.0).show_ui(ui, |ui| {
                    for (name, gains) in EQ_PRESETS {
                        if ui.selectable_label(app.lib.eq_preset == name, name).clicked() {
                            chosen = Some((name, gains));
                        }
                    }
                });
                if let Some((name, gains)) = chosen {
                    app.actions.push(Action::SetEqGains(gains, name.to_string()));
                }
            });
        });
        ui.add_space(14.0);
        let on = app.lib.eq_on;
        let gains = app.lib.eq_gains;
        // Courbe reliant les bandes.
        let w = ui.available_width();
        let (curve, _) = ui.allocate_exact_size(Vec2::new(w, 70.0), Sense::hover());
        let xs: Vec<f32> = (0..6).map(|k| curve.left() + 22.0 + k as f32 * (w - 44.0) / 5.0).collect();
        let y_of = |g: f32| egui::remap(g, 12.0..=-12.0, curve.top() + 6.0..=curve.bottom() - 6.0);
        ui.painter().line_segment([Pos2::new(curve.left(), y_of(0.0)), Pos2::new(curve.right(), y_of(0.0))], Stroke::new(1.0, Color32::from_white_alpha(40)));
        let mut pts = vec![];
        for k in 0..5 {
            for s in 0..=12 {
                let f = s as f32 / 12.0;
                let e = (1.0 - (f * std::f32::consts::PI).cos()) / 2.0;
                pts.push(Pos2::new(egui::lerp(xs[k]..=xs[k + 1], f), y_of(egui::lerp(gains[k]..=gains[k + 1], e))));
            }
        }
        let col = if on { Color32::from_white_alpha(230) } else { Color32::from_white_alpha(80) };
        ui.painter().add(Shape::line(pts, Stroke::new(3.0, col)));
        for (k, x) in xs.iter().enumerate() {
            ui.painter().circle_filled(Pos2::new(*x, y_of(gains[k])), 5.0, col);
        }
        ui.add_space(10.0);
        // Curseurs verticaux.
        let mut new_gains = gains;
        let mut changed = false;
        ui.horizontal(|ui| {
            let step = (w - 44.0) / 5.0;
            ui.add_space(0.0);
            for (k, (_, label)) in EQ_BANDS.iter().enumerate() {
                let x = curve.left() + 22.0 + k as f32 * step;
                let col_rect = Rect::from_min_size(Pos2::new(x - 22.0, ui.cursor().top()), Vec2::new(44.0, 230.0));
                place(ui, UiBuilder::new().max_rect(col_rect).layout(Layout::top_down(Align::Center)), |ui| {
                    if let Some(g) = eq_slider(ui, k, gains[k], 190.0) {
                        new_gains[k] = g;
                        changed = true;
                    }
                    ui.label(RichText::new(*label).font(bold(14.0)).color(soft));
                });
            }
            ui.allocate_space(Vec2::new(w, 230.0));
        });
        if changed {
            app.actions.push(Action::SetEqGains(new_gains, "Personnalisé".into()));
        }
        ui.label(RichText::new("Gains de -12 à +12 dB : 60 Hz, 150 Hz, 400 Hz, 1 kHz, 2,4 kHz, 15 kHz.").size(13.0).color(Color32::from_white_alpha(140)));
    });
}

// ---------------------------------------------------------------------------------
// Paramètres
// ---------------------------------------------------------------------------------

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(SURFACE).corner_radius(28).inner_margin(Margin::same(24)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

/// Pastille de choix (sélectionnée = lavande pleine).
fn chip(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let w = ui.painter().layout_no_wrap(label.to_string(), bold(15.0), TEXT).size().x + if selected { 58.0 } else { 36.0 };
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 44.0), Sense::click());
    let h = hover_t(ui, resp.id, resp.hovered());
    let fill = if selected { PRIMARY } else { lerp(SURFACE_HI, SELECT, h) };
    ui.painter().rect_filled(r, 22.0, fill);
    let color = if selected { ON_PRIMARY } else { TEXT };
    if selected {
        ui.painter().text(Pos2::new(r.left() + 24.0, r.center().y), Align2::CENTER_CENTER, I_CHECK.to_string(), icon_font(18.0), color);
        ui.painter().text(Pos2::new(r.left() + 40.0, r.center().y), Align2::LEFT_CENTER, label, bold(15.0), color);
    } else {
        ui.painter().text(r.center(), Align2::CENTER_CENTER, label, bold(15.0), color);
    }
    resp
}

fn settings_view(app: &mut App, ui: &mut egui::Ui, p: &Props) {
    use crate::library::Cookies;
    back_button(app, ui);
    ui.add_space(4.0);
    ui.label(RichText::new("Paramètres").font(heavy(44.0)).color(PRIMARY));
    ui.add_space(16.0);
    egui::ScrollArea::vertical().id_salt("settings").auto_shrink(false).show(ui, |ui| {
        card(ui, |ui| {
            ui.label(RichText::new("Accès à YouTube").font(heavy(24.0)));
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(
                "Si YouTube répond « Sign in to confirm you're not a bot », la lecture échoue. \
                 yt-dlp peut alors utiliser les cookies d'un navigateur où tu es connecté à YouTube.",
            ).size(15.0).color(MUTED)).wrap());
            ui.add_space(14.0);

            let current = app.lib.cookies.clone();
            let mut choice: Option<Cookies> = None;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
                if chip(ui, "Aucun", current == Cookies::None).clicked() {
                    choice = Some(Cookies::None);
                }
                for (label, id) in [("Firefox", "firefox"), ("Edge", "edge"), ("Chrome", "chrome"), ("Brave", "brave")] {
                    if chip(ui, label, current == Cookies::Browser(id.into())).clicked() {
                        choice = Some(Cookies::Browser(id.into()));
                    }
                }
                let is_file = matches!(current, Cookies::File(_));
                if chip(ui, "Fichier cookies.txt", is_file).clicked() && !is_file {
                    choice = Some(Cookies::File(String::new()));
                }
            });
            if let Cookies::File(path) = &current {
                ui.add_space(12.0);
                let mut p = path.clone();
                egui::Frame::new().fill(SURFACE_HI).corner_radius(22).inner_margin(Margin::symmetric(18, 12)).show(ui, |ui| {
                    ui.set_width(ui.available_width().min(640.0));
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut p)
                            .frame(egui::Frame::NONE)
                            .hint_text(r"Chemin du fichier, ex. C:\Users\moi\Documents\cookies.txt")
                            .desired_width(f32::INFINITY),
                    );
                    if r.changed() {
                        choice = Some(Cookies::File(p.clone()));
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new("Exporte-le avec une extension « Get cookies.txt LOCALLY », sur youtube.com, connecté.").size(13.5).color(MUTED));
            }
            if let Some(c) = choice {
                app.actions.push(Action::SetCookies(c));
            }

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(150.0, 46.0), Sense::click());
                ui.painter().rect_filled(r, 23.0, if resp.hovered() { CONTAINER } else { SELECT });
                ui.painter().text(r.center(), Align2::CENTER_CENTER, "Tester", bold(16.0), TEXT);
                let busy = matches!(app.cookie_test, Some(Loaded::Loading));
                if resp.clicked() && !busy {
                    app.actions.push(Action::TestCookies);
                }
                ui.add_space(10.0);
                match &app.cookie_test {
                    None => {
                        ui.label(RichText::new("Vérifie que YouTube accepte la lecture.").color(MUTED));
                    }
                    Some(Loaded::Loading) => {
                        ui.spinner();
                        ui.label(RichText::new("Test en cours…").color(MUTED));
                    }
                    Some(Loaded::Ready(Ok(()))) => {
                        ui.label(ico(I_CHECK, 20.0, PRIMARY));
                        ui.label(RichText::new("YouTube accepte la lecture.").font(bold(15.0)).color(PRIMARY));
                    }
                    Some(Loaded::Ready(Err(e))) => {
                        ui.add(egui::Label::new(RichText::new(e).color(ERROR).size(14.0)).wrap());
                    }
                    Some(Loaded::Missing) => {}
                }
            });
            ui.add_space(14.0);
            ui.add(egui::Label::new(RichText::new(
                "À savoir : Chrome et Edge chiffrent leurs cookies d'une façon que yt-dlp ne sait souvent pas lire, \
                 et il faut parfois fermer le navigateur pendant la lecture des cookies. Firefox ou un fichier cookies.txt \
                 marchent le plus souvent. Les cookies donnent accès à ta session Google : un compte secondaire est plus prudent.",
            ).size(13.5).color(FAINT)).wrap());
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.label(RichText::new("Sortie audio").font(heavy(24.0)));
            ui.label(RichText::new("Périphérique sur lequel Mixtape joue le son. « Automatique » suit le périphérique par défaut de Windows.").size(15.0).color(MUTED));
            ui.add_space(12.0);
            let current = app.lib.audio_device.clone();
            let mut chosen = None;
            // mpv liste « auto » puis chaque sortie WASAPI ; on n'affiche que celles-ci.
            let mut devices: Vec<(String, String)> = vec![("auto".into(), "Automatique (périphérique par défaut)".into())];
            devices.extend(p.devices.iter().filter(|(n, _)| n.starts_with("wasapi/")).cloned());
            if devices.len() == 1 && p.devices.is_empty() {
                ui.label(RichText::new("Liste en cours de chargement…").color(MUTED));
            }
            for (name, desc) in &devices {
                let sel = *name == current;
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 50.0), Sense::click());
                let h = hover_t(ui, resp.id, resp.hovered());
                let fill = if sel { CONTAINER } else { alpha(SURFACE_HI, (255.0 * h) as u8) };
                ui.painter().rect_filled(r, 25.0, fill);
                let c = Pos2::new(r.left() + 28.0, r.center().y);
                if sel {
                    ui.painter().add(cookie(c, 14.0, 7.0, 0.07, 0.0, PRIMARY));
                    ui.painter().text(c, Align2::CENTER_CENTER, I_CHECK.to_string(), icon_font(16.0), ON_PRIMARY);
                } else {
                    ui.painter().text(c, Align2::CENTER_CENTER, I_VOLUME.to_string(), icon_font(20.0), MUTED);
                }
                let label = if desc.is_empty() { name.as_str() } else { desc.as_str() };
                text_in(ui, Rect::from_min_max(Pos2::new(r.left() + 54.0, r.top()), r.max - Vec2::new(16.0, 0.0)), RichText::new(label).font(bold(15.5)).color(if sel { TEXT } else { MUTED }), None);
                if resp.clicked() && !sel {
                    chosen = Some(name.clone());
                }
            }
            if !devices.iter().any(|(n, _)| *n == current) && !p.devices.is_empty() {
                ui.label(RichText::new("La sortie choisie n'est plus branchée : mpv utilise le périphérique par défaut.").size(13.5).color(ERROR));
            }
            if let Some(d) = chosen {
                app.actions.push(Action::SetAudioDevice(d));
            }
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Égaliseur et son").font(heavy(24.0)));
                    ui.label(RichText::new("Égaliseur 6 bandes, préréglages, normalisation du volume.").size(15.0).color(MUTED));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_btn(ui, I_CHEVRON, 24.0, TEXT, Some(SURFACE_HI), "Ouvrir").clicked() {
                        app.imm_tab = ImmTab::Eq;
                        app.actions.push(Action::Go(View::NowPlaying));
                    }
                });
            });
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.label(RichText::new("À propos").font(heavy(24.0)));
            ui.add(egui::Label::new(RichText::new(format!(
                "Mixtape {} · lecture mpv, recherche et flux yt-dlp, paroles LRCLIB. \
                 Polices Google Sans Flex (OFL) et Material Icons (Apache 2.0). Design inspiré de Swiftify.",
                env!("CARGO_PKG_VERSION")
            )).size(14.5).color(MUTED)).wrap());
        });
        ui.add_space(20.0);
    });
}

// ---------------------------------------------------------------------------------
// Importer (Spotify, Deezer)
// ---------------------------------------------------------------------------------

fn pill_button(ui: &mut egui::Ui, label: &str, glyph: Option<char>, primary: bool, enabled: bool) -> egui::Response {
    let w = ui.painter().layout_no_wrap(label.to_string(), bold(15.0), TEXT).size().x + if glyph.is_some() { 64.0 } else { 40.0 };
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 46.0), if enabled { Sense::click() } else { Sense::hover() });
    let h = hover_t(ui, resp.id, resp.hovered() && enabled);
    let (fill, fg) = if primary { (lerp(PRIMARY, Color32::WHITE, 0.12 * h), ON_PRIMARY) } else { (lerp(SELECT, CONTAINER, h), TEXT) };
    let (fill, fg) = if enabled { (fill, fg) } else { (SURFACE_HI, FAINT) };
    ui.painter().rect_filled(r, 23.0, fill);
    let mut x = r.left() + 20.0;
    if let Some(g) = glyph {
        ui.painter().text(Pos2::new(x + 10.0, r.center().y), Align2::CENTER_CENTER, g.to_string(), icon_font(20.0), fg);
        x += 26.0;
    }
    ui.painter().text(Pos2::new(x, r.center().y), Align2::LEFT_CENTER, label, bold(15.0), fg);
    resp
}

fn import_view(app: &mut App, ui: &mut egui::Ui) {
    // Fichier glissé sur la fenêtre.
    let dropped: Vec<std::path::PathBuf> = ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
    for path in dropped {
        app.actions.push(Action::ImportFile(path));
    }
    back_button(app, ui);
    ui.add_space(4.0);
    ui.label(RichText::new("Importer").font(heavy(44.0)).color(PRIMARY));
    ui.label(RichText::new("Récupère tes titres likés et tes playlists Spotify ou Deezer : chaque titre est retrouvé sur YouTube.").size(16.0).color(MUTED));
    ui.add_space(14.0);
    egui::ScrollArea::vertical().id_salt("import").auto_shrink(false).show(ui, |ui| {
        let busy = app.import.running || app.import.analyzing;
        if app.import.running || app.import.done {
            import_progress(app, ui);
            ui.add_space(16.0);
        }
        if !app.import.running {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(ico(I_LINK, 24.0, PRIMARY));
                    ui.label(RichText::new("Depuis un lien").font(heavy(22.0)));
                });
                ui.label(RichText::new("Playlist ou album Spotify public · playlist ou album Deezer · profil Deezer public (coups de cœur + playlists).").size(14.5).color(MUTED));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let mut submit = false;
                    egui::Frame::new().fill(SURFACE_HI).corner_radius(23).inner_margin(Margin::symmetric(18, 13)).show(ui, |ui| {
                        ui.set_width((ui.available_width() - 170.0).max(200.0));
                        let r = ui.add(egui::TextEdit::singleline(&mut app.import.link).frame(egui::Frame::NONE).hint_text("https://open.spotify.com/playlist/…  ou  https://www.deezer.com/…").desired_width(f32::INFINITY));
                        submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    });
                    let can = !busy && !app.import.link.trim().is_empty();
                    if (pill_button(ui, "Analyser", None, true, can).clicked() || submit) && can {
                        app.actions.push(Action::ImportLink(app.import.link.trim().to_string()));
                    }
                });
            });
            ui.add_space(14.0);
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(ico(I_FILE, 24.0, PRIMARY));
                    ui.label(RichText::new("Depuis un fichier").font(heavy(22.0)));
                });
                ui.add(egui::Label::new(RichText::new(
                    "Pour tes titres likés Spotify (privés, donc inaccessibles par lien) : sur tunemymusic.com, choisis Spotify comme source, \
                     sélectionne « Titres likés » et tes playlists, puis « Exporter vers un fichier ». Glisse le fichier ici ou choisis-le. \
                     Marche aussi avec Exportify (CSV) et l'export de données Spotify (YourLibrary.json, Playlist1.json).",
                ).size(14.5).color(MUTED)).wrap());
                ui.add_space(10.0);
                if pill_button(ui, "Choisir un fichier…", Some(I_FILE), false, !busy).clicked() && !busy {
                    app.actions.push(Action::ImportPickFile);
                }
            });
        }
        if app.import.analyzing {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("Analyse en cours…").color(MUTED));
            });
        }
        if let Some(e) = &app.import.error {
            ui.add_space(10.0);
            ui.add(egui::Label::new(RichText::new(e).color(ERROR).size(14.5)).wrap());
        }
        if !app.import.cols.is_empty() && !app.import.running && !app.import.done {
            ui.add_space(16.0);
            import_selection(app, ui);
        }
        ui.add_space(20.0);
    });
}

fn import_selection(app: &mut App, ui: &mut egui::Ui) {
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("À importer").font(heavy(22.0)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("Tout effacer").color(MUTED).size(14.0)).frame(false)).clicked() {
                    app.actions.push(Action::ImportReset);
                }
            });
        });
        ui.add_space(8.0);
        for c in app.import.cols.iter_mut() {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::click());
            let h = hover_t(ui, resp.id, resp.hovered());
            ui.painter().rect_filled(r, 20.0, alpha(SURFACE_HI, (90.0 + 120.0 * h) as u8));
            if resp.clicked() {
                c.selected = !c.selected;
            }
            // Case à cocher en cookie.
            let cc = Pos2::new(r.left() + 30.0, r.center().y);
            if c.selected {
                ui.painter().add(cookie(cc, 14.0, 7.0, 0.07, 0.0, PRIMARY));
                ui.painter().text(cc, Align2::CENTER_CENTER, I_CHECK.to_string(), icon_font(16.0), ON_PRIMARY);
            } else {
                ui.painter().circle_stroke(cc, 12.0, Stroke::new(2.0, MUTED));
            }
            let dest = match c.dest {
                crate::import::Dest::Liked => "→ Titres likés".to_string(),
                crate::import::Dest::Playlist => "→ nouvelle playlist".to_string(),
            };
            let mut sub = format!("{} · {} · {}", c.source, titres(c.tracks.len()), dest);
            if c.truncated {
                sub.push_str(" · limité aux 100 premiers (passe par un fichier pour le reste)");
            }
            text_in(ui, Rect::from_min_max(Pos2::new(r.left() + 56.0, r.top()), r.max - Vec2::new(16.0, 0.0)), RichText::new(&c.name).font(bold(16.0)), Some(RichText::new(sub).size(13.5).color(if c.truncated { ERROR } else { MUTED })));
            ui.add_space(6.0);
        }
        let n: usize = app.import.cols.iter().filter(|c| c.selected).map(|c| c.tracks.len()).sum();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let label = format!("Importer {}", titres(n));
            if pill_button(ui, &label, Some(I_IMPORT), true, n > 0).clicked() && n > 0 {
                app.actions.push(Action::ImportStart);
            }
            ui.add_space(8.0);
            let mins = (n as f32 * 0.8 / 60.0).ceil() as usize;
            ui.label(RichText::new(format!("≈ {} min · tu peux continuer à écouter pendant l'import", mins.max(1))).color(MUTED).size(14.0));
        });
    });
}

fn import_progress(app: &mut App, ui: &mut egui::Ui) {
    let st = &app.import;
    let (running, total, processed, found, missing, current) = (st.running, st.total, st.processed, st.found, st.missing.clone(), st.current.clone());
    card(ui, |ui| {
        ui.label(RichText::new(if running { "Import en cours" } else { "Import terminé" }).font(heavy(22.0)));
        ui.add_space(8.0);
        let frac = if total > 0 { processed as f64 / total as f64 } else { 0.0 };
        let w = ui.available_width();
        // Vague de progression (non interactive).
        let (r, _) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::hover());
        let (x0, x1) = (r.left() + 3.0, r.right() - 3.0);
        let xf = x0 + (x1 - x0) * frac as f32;
        let t = ui.input(|i| i.time) as f32;
        if xf > x0 + 1.0 {
            ui.painter().add(Shape::line(wave_points(x0, xf, r.center().y, 3.0, 22.0, if running { t * 4.0 } else { 0.0 }), Stroke::new(4.0, PRIMARY)));
        }
        if x1 > xf + 6.0 {
            ui.painter().line_segment([Pos2::new(xf + 6.0, r.center().y), Pos2::new(x1, r.center().y)], Stroke::new(4.0, Color32::from_white_alpha(40)));
        }
        if running {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
        ui.add_space(6.0);
        ui.label(RichText::new(format!("{processed} / {total} · {found} trouvés · {} introuvables", missing.len())).font(bold(15.0)));
        if running && !current.is_empty() {
            ui.add(egui::Label::new(RichText::new(format!("Recherche : {current}…")).color(MUTED).size(14.0)).truncate());
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if running {
                if pill_button(ui, "Arrêter", Some(I_CLOSE), false, true).clicked() {
                    app.actions.push(Action::ImportCancel);
                }
            } else {
                if pill_button(ui, "Voir les titres likés", Some(I_HEART_FILL), true, true).clicked() {
                    app.actions.push(Action::Go(View::Liked));
                }
                if pill_button(ui, "Nouvel import", Some(I_IMPORT), false, true).clicked() {
                    app.actions.push(Action::ImportReset);
                }
            }
        });
        if !missing.is_empty() && !running {
            ui.add_space(12.0);
            egui::CollapsingHeader::new(RichText::new(format!("Titres introuvables ({})", missing.len())).color(MUTED)).id_salt("missing").show(ui, |ui| {
                if ui.button("Copier la liste").clicked() {
                    ui.ctx().copy_text(missing.join("\n"));
                }
                for m in missing.iter().take(300) {
                    ui.label(RichText::new(m).size(13.5).color(MUTED));
                }
            });
        }
    });
}
