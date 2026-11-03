//! Mode de test sans souris : `mixtape.exe --autotest "<recherche>" <dossier>`.
//!
//! Volume à 0, bibliothèque isolée dans <dossier>. Enchaîne : recherche → lecture →
//! like → « Lecture en cours » (pochette + paroles) → suivant → avance à 2:00 → pause →
//! titres likés → nouvelle playlist → accueil + aléatoire. Captures et `autotest.json`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use serde_json::json;

use crate::lyrics::Lyrics;
use crate::{mpv::Props, Action, App, Loaded, View};

pub struct AutoTest {
    pub query: String,
    out: PathBuf,
    step: u8,
    started: Instant,
    step_at: Instant,
    log: Vec<serde_json::Value>,
    lyrics: String,
    /// Session attendue (mode @restore), lue avant que l'app ne la réécrive.
    expected: Option<(i64, f64)>,
    /// Position du premier titre réellement joué (des vidéos peuvent être sautées).
    first: i64,
}

impl AutoTest {
    pub fn from_args() -> Option<Self> {
        let args: Vec<String> = std::env::args().collect();
        let i = args.iter().position(|a| a == "--autotest")?;
        let query = args.get(i + 1)?.clone();
        let out = PathBuf::from(args.get(i + 2).cloned().unwrap_or_else(|| ".".into()));
        let _ = std::fs::create_dir_all(&out);
        let mut expected = None;
        if query == "@restore" || query == "@settings" {
            let lib = crate::library::Library::load(out.join("library.json"));
            expected = Some((lib.session.pos, lib.session.time));
        } else {
            let _ = std::fs::remove_file(out.join("library.json"));
        }
        let now = Instant::now();
        Some(Self { query, out, step: 0, started: now, step_at: now, log: vec![], lyrics: String::new(), expected, first: -1 })
    }

    pub fn library_path(&self) -> PathBuf {
        self.out.join("library.json")
    }

    fn mark(&mut self, what: &str, p: &Props, app: &App) {
        self.log.push(json!({
            "t": (self.started.elapsed().as_secs_f64() * 10.0).round() / 10.0,
            "step": what,
            "pos": p.pos, "time": p.time, "pause": p.pause,
            "queue": app.queue.len(),
            "liked": app.lib.liked.len(),
            "history": app.lib.history.len(),
            "playlists": app.lib.playlists.len(),
            "shuffle": app.lib.shuffle,
            "view": format!("{:?}", app.view),
        }));
        self.step += 1;
        self.step_at = Instant::now();
    }

    fn screenshot(&self, ctx: &egui::Context, name: &str) {
        let path = self.out.join(name);
        ctx.request_screenshot(move |img: Arc<egui::ColorImage>| {
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, raw) {
                let _ = buf.save(&path);
            }
        });
    }

    fn write_report(&self, ok: bool, reason: &str) {
        let report = json!({ "ok": ok, "reason": reason, "lyrics": self.lyrics, "steps": self.log });
        let _ = std::fs::write(self.out.join("autotest.json"), serde_json::to_string_pretty(&report).unwrap());
    }
}

pub fn step(app: &mut App, p: &Props) {
    let Some(mut t) = app.autotest.take() else { return };
    run(&mut t, app, p);
    app.autotest = Some(t);
}

fn run(t: &mut AutoTest, app: &mut App, p: &Props) {
    let ctx = app.ctx.clone();
    ctx.request_repaint_after(Duration::from_millis(200));
    let waited = t.step_at.elapsed();
    let time = p.time.unwrap_or(0.0);

    if t.query == "@restore" {
        restore(t, app, p, &ctx);
        return;
    }
    if t.query == "@settings" {
        settings(t, app, p, &ctx);
        return;
    }
    if t.query.starts_with("@import ") {
        import_test(t, app, p, &ctx);
        return;
    }
    if t.query.starts_with("@mesure ") {
        measure(t, app, p, &ctx);
        return;
    }
    if t.query.starts_with("@tour ") {
        tour(t, app, p, &ctx);
        return;
    }
    if t.step < 12 && waited > Duration::from_secs(60) {
        let s = t.step;
        t.write_report(false, &format!("délai dépassé à l'étape {s}"));
        t.step = 99;
        return;
    }
    match t.step {
        0 if !app.results.is_empty() => {
            t.mark("résultats", p, app);
            app.actions.push(Action::Play { tracks: app.results.clone(), start: 1 });
        }
        1 if p.pos >= 0 && time > 4.0 => {
            t.first = p.pos;
            t.mark("lecture", p, app);
            if let Some(cur) = app.current(p).cloned() {
                app.actions.push(Action::ToggleLike(cur));
            }
            app.actions.push(Action::Go(View::NowPlaying));
        }
        2 => {
            let url = app.current(p).map(|c| c.url.clone()).unwrap_or_default();
            let lyrics_done = !matches!(app.lyrics.get(&url), None | Some(Loaded::Loading));
            let art_done = !matches!(&app.art, Some((_, Loaded::Loading)) | None);
            if (lyrics_done && art_done && waited > Duration::from_secs(2)) || waited > Duration::from_secs(20) {
                t.lyrics = match app.lyrics.get(&url) {
                    Some(Loaded::Ready(Lyrics::Synced(l))) => format!("synchronisées ({} lignes)", l.len()),
                    Some(Loaded::Ready(Lyrics::Plain(_))) => "texte seul".into(),
                    Some(Loaded::Ready(Lyrics::Instrumental)) => "instrumental".into(),
                    Some(Loaded::Missing) => "introuvables".into(),
                    _ => "en cours".into(),
                };
                t.mark("lecture en cours", p, app);
                t.screenshot(&ctx, "1-lecture-en-cours.png");
            }
        }
        3 if waited > Duration::from_secs(1) => {
            t.mark("suivant", p, app);
            app.actions.push(Action::Next);
        }
        4 if p.pos > t.first && time > 2.0 => {
            t.mark("lecture titre 2", p, app);
            app.actions.push(Action::Seek(120.0));
        }
        5 if time >= 120.0 => {
            t.mark("avance", p, app);
            app.actions.push(Action::TogglePause);
            app.actions.push(Action::Go(View::Liked));
        }
        6 if p.pause && waited > Duration::from_secs(1) => {
            t.mark("pause + titres likés", p, app);
            t.screenshot(&ctx, "2-titres-likes.png");
        }
        7 if waited > Duration::from_secs(1) => {
            t.mark("nouvelle playlist", p, app);
            if let Some(cur) = app.current(p).cloned() {
                app.actions.push(Action::NewPlaylist(Some(cur)));
            }
        }
        8 if waited > Duration::from_secs(1) => {
            t.mark("accueil + aléatoire", p, app);
            app.actions.push(Action::Go(View::Home));
            app.actions.push(Action::ToggleShuffle);
            app.actions.push(Action::TogglePause);
            app.right = crate::RightPane::Lyrics;
            // .139 est en 150 % : zoom réduit pour que le panneau de paroles ait la place.
            ctx.set_zoom_factor(0.8);
        }
        9 if waited > Duration::from_secs(4) => {
            t.mark("accueil", p, app);
            t.screenshot(&ctx, "3-accueil.png");
        }
        10 if waited > Duration::from_secs(1) => {
            t.mark("égaliseur", p, app);
            ctx.set_zoom_factor(1.0);
            app.imm_tab = crate::ImmTab::Eq;
            app.actions.push(Action::SetEqGains([4.0, 2.5, 0.0, -1.0, 2.0, 3.5], "Personnalisé".into()));
            app.actions.push(Action::Go(View::NowPlaying));
        }
        11 if waited > Duration::from_secs(2) => {
            t.mark("fin", p, app);
            t.screenshot(&ctx, "4-egaliseur.png");
        }
        12 if waited > Duration::from_secs(1) => {
            t.step = 13;
            t.write_report(true, "ok");
            // Fermeture normale : vérifie que la session est enregistrée en quittant.
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        _ => {}
    }
}

/// `--autotest @restore <dossier>` : relit la bibliothèque du dossier et vérifie que la
/// session revient en pause, sur le bon titre, à la bonne position.
fn restore(t: &mut AutoTest, app: &mut App, p: &Props, ctx: &egui::Context) {
    let waited = t.step_at.elapsed();
    match t.step {
        0 if (p.pos >= 0 && p.time.is_some() && waited > Duration::from_secs(2)) || waited > Duration::from_secs(40) => {
            t.mark("session restaurée", p, app);
            t.screenshot(ctx, "5-session-restauree.png");
        }
        1 if waited > Duration::from_secs(1) => {
            t.step = 2;
            let (pos, time) = t.expected.unwrap_or((-1, 0.0));
            let ok = p.pause && p.pos == pos && (p.time.unwrap_or(0.0) - time).abs() < 3.0;
            let why = format!("attendu pos {pos} t {time:.1} en pause ; obtenu pos {} t {:?} pause {}", p.pos, p.time, p.pause);
            t.write_report(ok, &why);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        _ => {}
    }
}

/// `--autotest @settings <dossier>` : écran Paramètres, cookies Firefox, bouton Tester.
fn settings(t: &mut AutoTest, app: &mut App, p: &Props, ctx: &egui::Context) {
    use crate::library::Cookies;
    let waited = t.step_at.elapsed();
    match t.step {
        0 if waited > Duration::from_secs(1) => {
            t.mark("paramètres", p, app);
            app.actions.push(Action::Go(View::Settings));
            app.actions.push(Action::SetCookies(Cookies::Browser("firefox".into())));
        }
        1 if waited > Duration::from_secs(1) => {
            t.mark("test cookies", p, app);
            app.actions.push(Action::TestCookies);
        }
        2 if matches!(app.cookie_test, Some(Loaded::Ready(_))) || waited > Duration::from_secs(45) => {
            t.lyrics = match &app.cookie_test {
                Some(Loaded::Ready(Ok(()))) => "test : OK".into(),
                Some(Loaded::Ready(Err(e))) => format!("test : {e}"),
                _ => "test : sans réponse".into(),
            };
            t.mark("résultat", p, app);
            t.screenshot(ctx, "6-parametres.png");
        }
        3 if waited > Duration::from_secs(1) => {
            t.step = 4;
            let mpv_err = app.error.as_ref().map(|(e, _)| e.clone()).unwrap_or_default();
            let ok = matches!(app.cookie_test, Some(Loaded::Ready(_))) && mpv_err.is_empty();
            t.write_report(ok, &format!("erreur mpv : « {mpv_err} »"));
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        _ => {}
    }
}

/// `--autotest "@tour <recherche>" <dossier>` : capture chaque écran, puis enchaîne des
/// phases de 20 s (immersif, accueil, pause, réduit) pour mesurer la consommation.
fn tour(t: &mut AutoTest, app: &mut App, p: &Props, ctx: &egui::Context) {
    use crate::{ImmTab, RightPane};
    let waited = t.step_at.elapsed();
    let time = p.time.unwrap_or(0.0);
    let after = |s: u64| waited > Duration::from_secs(s);
    if t.step < 40 && waited > Duration::from_secs(60) {
        let s = t.step;
        t.write_report(false, &format!("délai dépassé à l'étape {s}"));
        t.step = 99;
        return;
    }
    let shot = |t: &mut AutoTest, app: &App, name: &str| {
        t.mark(name, p, app);
        t.screenshot(ctx, &format!("{name}.png"));
    };
    match t.step {
        0 if !app.results.is_empty() && after(2) => {
            shot(t, app, "01-recherche-etroit");
            app.actions.push(Action::Play { tracks: app.results.clone(), start: 1 });
        }
        1 if p.pos == 0 && time > 3.0 => {
            t.mark("lecture", p, app);
            if let Some(cur) = app.current(p).cloned() {
                app.actions.push(Action::ToggleLike(cur.clone()));
                app.actions.push(Action::NewPlaylist(Some(cur)));
            }
            for k in [2, 3, 4] {
                if let Some(tr) = app.results.get(k).cloned() {
                    app.actions.push(Action::AddToPlaylist(0, tr.clone()));
                    if k == 2 {
                        app.actions.push(Action::ToggleLike(tr));
                    }
                }
            }
            app.actions.push(Action::Seek(55.0));
            app.actions.push(Action::Go(View::Home));
        }
        2 if after(4) => shot(t, app, "02-accueil-etroit"),
        3 if after(1) => {
            t.mark("zoom 0.8", p, app);
            ctx.set_zoom_factor(0.8);
        }
        4 if after(3) => {
            shot(t, app, "03-accueil-large");
            app.actions.push(Action::Go(View::Search));
        }
        5 if after(2) => {
            shot(t, app, "04-recherche-large");
            app.actions.push(Action::Go(View::Liked));
        }
        6 if after(2) => {
            shot(t, app, "05-titres-likes");
            app.actions.push(Action::Go(View::History));
        }
        7 if after(2) => {
            shot(t, app, "06-historique");
            app.actions.push(Action::Go(View::Playlists));
        }
        8 if after(2) => {
            shot(t, app, "07-playlists");
            app.actions.push(Action::Go(View::Playlist(0)));
        }
        9 if after(2) => {
            shot(t, app, "08-playlist");
            app.right = RightPane::Queue;
        }
        10 if after(2) => {
            shot(t, app, "09-file-droite");
            app.right = RightPane::Lyrics;
            app.actions.push(Action::Go(View::Settings));
        }
        11 if after(2) => {
            shot(t, app, "10-parametres");
            ctx.set_zoom_factor(1.0);
            app.imm_tab = ImmTab::Art;
            app.actions.push(Action::Go(View::NowPlaying));
        }
        12 if after(3) => {
            shot(t, app, "11-immersif-pochette");
            app.imm_tab = ImmTab::Lyrics;
        }
        13 if after(3) => {
            shot(t, app, "12-immersif-paroles");
            app.imm_tab = ImmTab::Queue;
        }
        14 if after(2) => {
            shot(t, app, "13-immersif-file");
            app.imm_tab = ImmTab::Sleep;
            app.actions.push(Action::SetSleep(Some(30)));
        }
        15 if after(2) => {
            shot(t, app, "14-immersif-minuterie");
            app.actions.push(Action::SetSleep(None));
            app.imm_tab = ImmTab::Eq;
        }
        16 if after(2) => {
            shot(t, app, "15-immersif-egaliseur");
            app.imm_tab = ImmTab::Lyrics;
        }
        // Phases de mesure (20 s chacune), horodatées dans le rapport.
        17 if after(2) => t.mark("MESURE immersif lecture", p, app),
        18 if after(20) => {
            t.mark("MESURE accueil lecture", p, app);
            app.actions.push(Action::Go(View::Home));
        }
        19 if after(20) => {
            t.mark("MESURE accueil pause", p, app);
            app.actions.push(Action::TogglePause);
        }
        20 if after(20) => {
            t.mark("MESURE réduit lecture", p, app);
            app.actions.push(Action::TogglePause);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        21 if after(20) => {
            t.mark("fin", p, app);
            t.write_report(true, "ok");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            t.step = 22;
        }
        _ => {}
    }
}

/// `--autotest "@mesure <recherche>" <dossier>` : usage normal sans captures ni zoom,
/// phases de 30 s horodatées (accueil en lecture, immersif, accueil en pause).
fn measure(t: &mut AutoTest, app: &mut App, p: &Props, ctx: &egui::Context) {
    let waited = t.step_at.elapsed();
    let after = |s: u64| waited > Duration::from_secs(s);
    match t.step {
        0 if !app.results.is_empty() => {
            t.mark("résultats", p, app);
            app.actions.push(Action::Play { tracks: app.results.clone(), start: 1 });
        }
        1 if p.pos == 0 && p.time.unwrap_or(0.0) > 3.0 => {
            t.mark("MESURE accueil lecture", p, app);
            app.actions.push(Action::Go(View::Home));
        }
        2 if after(30) => {
            t.mark("MESURE immersif lecture", p, app);
            app.actions.push(Action::Go(View::NowPlaying));
        }
        3 if after(30) => {
            t.mark("MESURE accueil pause", p, app);
            app.actions.push(Action::Go(View::Home));
            app.actions.push(Action::TogglePause);
        }
        4 if after(30) => {
            t.mark("fin", p, app);
            t.write_report(true, "ok");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            t.step = 5;
        }
        _ => {}
    }
}

/// `--autotest "@import <lien>|<lien>|<fichier>…" <dossier>` : analyse chaque source,
/// importe tout, puis rapporte ce qui a été trouvé.
fn import_test(t: &mut AutoTest, app: &mut App, p: &Props, ctx: &egui::Context) {
    let waited = t.step_at.elapsed();
    let sources: Vec<String> = t.query.trim_start_matches("@import ").split('|').map(str::to_string).collect();
    let k = t.step as usize;
    if k < sources.len() {
        if k == 0 && waited < Duration::from_millis(500) {
            app.actions.push(Action::Go(View::Import));
            return;
        }
        if !app.import.analyzing && app.import.cols.len() >= k && (k == 0 || waited > Duration::from_secs(1)) {
            let src = &sources[k];
            t.mark(&format!("analyse {src}"), p, app);
            if src.starts_with("http") {
                app.actions.push(Action::ImportLink(src.clone()));
            } else {
                app.actions.push(Action::ImportFile(src.into()));
            }
        }
        return;
    }
    match k - sources.len() {
        0 if !app.import.analyzing && waited > Duration::from_secs(2) => {
            t.mark("sélection", p, app);
            t.screenshot(ctx, "1-import-selection.png");
        }
        1 if waited > Duration::from_secs(1) => {
            t.lyrics = app.import.cols.iter().map(|c| format!("{} [{}] {} titres", c.name, c.source, c.tracks.len())).collect::<Vec<_>>().join(" ; ");
            if let Some(e) = &app.import.error {
                t.lyrics.push_str(&format!(" | ERREUR {e}"));
            }
            t.mark("démarrage", p, app);
            app.actions.push(Action::ImportStart);
        }
        2 if app.import.processed > app.import.total / 3 && waited > Duration::from_secs(3) => {
            t.mark("en cours", p, app);
            t.screenshot(ctx, "2-import-en-cours.png");
        }
        3 if app.import.done => {
            t.mark("terminé", p, app);
            t.screenshot(ctx, "3-import-fin.png");
        }
        4 if waited > Duration::from_secs(1) => {
            let liked: Vec<String> = app.lib.liked.iter().take(8).map(|x| x.title.clone()).collect();
            let pls: Vec<String> = app.lib.playlists.iter().map(|pl| format!("{} ({}) : {}", pl.name, pl.tracks.len(), pl.tracks.iter().take(4).map(|x| x.title.as_str()).collect::<Vec<_>>().join(" | "))).collect();
            let report = format!(
                "{} | trouvés {} / {} | introuvables : {} | likés : {} | playlists : {}",
                t.lyrics, app.import.found, app.import.total, app.import.missing.join(" ; "), liked.join(" | "), pls.join(" || ")
            );
            t.step = 99;
            t.write_report(true, &report);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        _ => {}
    }
}
