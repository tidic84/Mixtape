#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod art;
mod autotest;
mod fonts;
mod hotkeys;
mod import;
mod library;
mod lyrics;
mod mpv;
mod search;
mod smtc;
mod views;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui;
use serde_json::json;

use library::{Library, Playlist, Repeat, Session};
use lyrics::Lyrics;
use mpv::Mpv;
use search::Track;

/// Dossier `bin` (mpv.exe + yt-dlp.exe) : à côté de l'exécutable ou dans un parent.
fn bin_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .skip(1)
        .map(|d| d.join("bin"))
        .find(|b| b.join("mpv.exe").exists() && b.join("yt-dlp.exe").exists())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    Home,
    Search,
    Liked,
    History,
    Playlists,
    Playlist(usize),
    Import,
    /// Vue immersive plein écran (pochette, paroles, file, minuterie, égaliseur).
    NowPlaying,
    Settings,
}

/// Panneau à droite de la vue principale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RightPane {
    None,
    Lyrics,
    Queue,
}

/// Onglets de la barre d'outils de la vue immersive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImmTab {
    Art,
    Lyrics,
    Queue,
    Sleep,
    Eq,
}

/// Minuterie de veille : met en pause à une heure donnée ou à la fin du titre.
#[derive(Clone, Copy, Debug)]
pub enum Sleep {
    At(Instant, u64),
    EndOfTrack(i64),
}

pub enum SearchStatus {
    Idle,
    Searching,
    Done,
    Error(String),
}

/// Demandes émises par l'interface pendant le rendu, appliquées en fin d'image.
pub enum Action {
    /// Lit `tracks[start]`, puis la suite de la liste.
    Play { tracks: Vec<Track>, start: usize },
    Enqueue(Track),
    PlayNext(Track),
    ToggleLike(Track),
    AddToPlaylist(usize, Track),
    NewPlaylist(Option<Track>),
    RemoveFromPlaylist(usize, usize),
    DeletePlaylist(usize),
    QueuePlay(usize),
    QueueRemove(usize),
    QueueClear,
    TogglePause,
    Next,
    Prev,
    Seek(f64),
    SetVolume(f64),
    ToggleShuffle,
    CycleRepeat,
    Go(View),
    Search(String),
    CopyLink(String),
    SetEqGains([f32; 6], String),
    ToggleEq,
    ToggleNormalize,
    /// Minutes (0 = fin du titre), `None` = désactiver.
    SetSleep(Option<u64>),
    SetCookies(library::Cookies),
    SetAudioDevice(String),
    ImportLink(String),
    ImportPickFile,
    ImportFile(PathBuf),
    ImportStart,
    ImportCancel,
    ImportReset,
    TestCookies,
}

/// État de l'écran « Importer ».
#[derive(Default)]
pub struct ImportState {
    pub link: String,
    pub cols: Vec<import::Collection>,
    pub analyzing: bool,
    pub error: Option<String>,
    pub running: bool,
    pub done: bool,
    pub total: usize,
    pub processed: usize,
    pub found: usize,
    pub missing: Vec<String>,
    pub current: String,
    analyze_rx: Option<mpsc::Receiver<Result<Vec<import::Collection>, String>>>,
    progress_rx: Option<mpsc::Receiver<import::Progress>>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Pour chaque collection : index de la playlist créée (None = titres likés).
    targets: Vec<Option<usize>>,
    since_save: usize,
}

/// Pochette prête à l'affichage : texture, 4 teintes de fond, teinte claire.
pub struct ArtReady {
    pub file: Option<PathBuf>,
    pub tex: egui::TextureHandle,
    pub colors: [egui::Color32; 4],
    pub light: egui::Color32,
}

/// Données chargées en arrière-plan pour le titre en cours (paroles, pochette).
pub enum Loaded<T> {
    Loading,
    Ready(T),
    Missing,
}

pub struct App {
    pub ctx: egui::Context,
    bin: Option<PathBuf>,
    mpv: Option<Mpv>,
    pub lib: Library,
    pub view: View,
    pub back: Vec<View>,
    pub query: String,
    pub results: Vec<Track>,
    pub results_query: String,
    pub search_status: SearchStatus,
    search_rx: Option<mpsc::Receiver<Result<Vec<Track>, String>>>,
    /// Miroir de la playlist de mpv (même ordre, mêmes index).
    pub queue: Vec<Track>,
    pub right: RightPane,
    /// Le panneau de droite tient-il dans la fenêtre (calculé à chaque image).
    pub right_fits: bool,
    pub imm_tab: ImmTab,
    pub sleep: Option<Sleep>,
    pub import: ImportState,
    /// Résultat du dernier test d'accès YouTube (`None` = jamais lancé).
    pub cookie_test: Option<Loaded<Result<(), String>>>,
    cookie_rx: Option<mpsc::Receiver<Result<(), String>>>,
    pub seek_drag: Option<f64>,
    pub volume: f64,
    pub muted_volume: Option<f64>,
    pub focus_search: bool,
    pub actions: Vec<Action>,
    pub error: Option<(String, Instant)>,

    pub lyrics: HashMap<String, Loaded<Lyrics>>,
    lyrics_rx: mpsc::Receiver<(String, Option<Lyrics>)>,
    lyrics_tx: mpsc::Sender<(String, Option<Lyrics>)>,
    /// Pochette du titre en cours : (url, état).
    pub art: Option<(String, Loaded<ArtReady>)>,
    art_rx: mpsc::Receiver<(String, Option<art::Art>)>,
    art_tx: mpsc::Sender<(String, Option<art::Art>)>,

    keys_rx: mpsc::Receiver<hotkeys::MediaKey>,
    keys_tx: mpsc::Sender<hotkeys::MediaKey>,
    smtc: Option<smtc::Smtc>,
    /// (url du titre publié, pochette finale envoyée, dernière position envoyée, à quand)
    smtc_state: (String, bool, f64, Instant),
    last_pos: i64,
    last_saved: Instant,
    pub autotest: Option<autotest::AutoTest>,
    title: String,
    corners_done: bool,
}

fn window_hwnd(frame: &eframe::Frame) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match frame.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

/// Fenêtre sans bordure : on demande à Windows 11 des coins arrondis (sans effet ailleurs).
fn round_corners(frame: &eframe::Frame) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
    let Ok(handle) = frame.window_handle() else { return };
    if let RawWindowHandle::Win32(h) = handle.as_raw() {
        let pref = DWMWCP_ROUND;
        unsafe {
            DwmSetWindowAttribute(h.hwnd.get() as _, DWMWA_WINDOW_CORNER_PREFERENCE as u32, &pref as *const _ as *const _, std::mem::size_of_val(&pref) as u32);
        }
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        egui_extras::install_image_loaders(&ctx);
        views::setup_style(&ctx);

        let autotest = autotest::AutoTest::from_args();
        let lib_path = autotest.as_ref().map_or_else(Library::default_path, |t| t.library_path());
        let mut lib = Library::load(lib_path);
        if autotest.is_some() {
            // Bibliothèque de test vierge, mais mêmes cookies YouTube que l'utilisateur.
            lib.cookies = Library::load(Library::default_path()).cookies;
        }
        {
            // Polices des écritures non latines présentes dans la bibliothèque.
            let all: String = lib.liked.iter().chain(&lib.history).chain(lib.playlists.iter().flat_map(|p| &p.tracks)).chain(&lib.session.queue)
                .flat_map(|t| [t.title.as_str(), t.artist.as_str()]).collect::<Vec<_>>().join(" ");
            fonts::ensure(&ctx, &all);
        }

        let bin = bin_dir();
        let mut error = None;
        let mpv = match &bin {
            Some(bin) => {
                let c = ctx.clone();
                match Mpv::start(bin, move || c.request_repaint()) {
                    Ok(m) => Some(m),
                    Err(e) => {
                        error = Some((e, Instant::now()));
                        None
                    }
                }
            }
            None => {
                error = Some(("dossier bin introuvable (mpv.exe, yt-dlp.exe)".into(), Instant::now()));
                None
            }
        };

        // Boutons multimédia : par la session Windows (SMTC, créée à la première image,
        // quand la fenêtre existe) ; raccourcis globaux seulement si le SMTC échoue.
        let (keys_tx, keys_rx) = mpsc::channel();
        let (lyrics_tx, lyrics_rx) = mpsc::channel();
        let (art_tx, art_rx) = mpsc::channel();

        let mut app = Self {
            ctx,
            bin,
            mpv,
            volume: lib.volume,
            lib,
            view: View::Home,
            back: vec![],
            query: String::new(),
            results: vec![],
            results_query: String::new(),
            search_status: SearchStatus::Idle,
            search_rx: None,
            queue: vec![],
            right: RightPane::Lyrics,
            right_fits: true,
            imm_tab: ImmTab::Lyrics,
            sleep: None,
            import: ImportState::default(),
            cookie_test: None,
            cookie_rx: None,
            seek_drag: None,
            muted_volume: None,
            focus_search: false,
            actions: vec![],
            error,
            lyrics: HashMap::new(),
            lyrics_rx,
            lyrics_tx,
            art: None,
            art_rx,
            art_tx,
            keys_rx,
            keys_tx,
            smtc: None,
            smtc_state: (String::new(), false, -1.0, Instant::now()),
            last_pos: -1,
            last_saved: Instant::now(),
            autotest,
            title: String::new(),
            corners_done: false,
        };
        app.apply_settings();
        if let Some(t) = &app.autotest {
            let q = t.query.clone();
            app.mpv_cmd(json!(["set_property", "volume", 0]));
            if q == "@restore" {
                app.restore_session();
            } else if q == "@settings" {
            } else {
                app.actions.push(Action::Search(q.trim_start_matches("@tour ").trim_start_matches("@mesure ").to_string()));
            }
        } else {
            app.restore_session();
        }
        app
    }

    pub fn mpv_cmd(&self, args: serde_json::Value) {
        if let Some(m) = &self.mpv {
            m.cmd(args);
        }
    }

    pub fn props(&self) -> mpv::Props {
        self.mpv.as_ref().map(Mpv::props).unwrap_or_default()
    }

    pub fn current(&self, p: &mpv::Props) -> Option<&Track> {
        usize::try_from(p.pos).ok().and_then(|i| self.queue.get(i))
    }

    pub fn show_error(&mut self, e: impl Into<String>) {
        self.error = Some((e.into(), Instant::now()));
    }

    fn apply_settings(&self) {
        self.mpv_cmd(json!(["set_property", "volume", self.lib.volume.round()]));
        self.apply_repeat();
        self.apply_filters();
        self.apply_cookies();
        self.mpv_cmd(json!(["set_property", "audio-device", self.lib.audio_device]));
    }

    fn apply_cookies(&self) {
        self.mpv_cmd(json!(["set_property", "ytdl-raw-options", self.lib.cookies.mpv_raw_options()]));
    }

    fn apply_filters(&self) {
        self.mpv_cmd(json!(["af", "set", self.lib.audio_filters()]));
    }

    /// Minuterie de veille : pause quand l'heure est passée ou que le titre a changé.
    fn check_sleep(&mut self, p: &mpv::Props) {
        let fire = match self.sleep {
            Some(Sleep::At(t, _)) => Instant::now() >= t,
            Some(Sleep::EndOfTrack(pos)) => p.pos != pos,
            None => false,
        };
        if fire {
            self.sleep = None;
            self.mpv_cmd(json!(["set_property", "pause", true]));
        } else if matches!(self.sleep, Some(Sleep::At(..))) {
            self.ctx.request_repaint_after(Duration::from_secs(1));
        }
    }

    fn apply_repeat(&self) {
        let (file, list) = match self.lib.repeat {
            Repeat::Off => ("no", "no"),
            Repeat::All => ("no", "inf"),
            Repeat::One => ("inf", "no"),
        };
        self.mpv_cmd(json!(["set_property", "loop-file", file]));
        self.mpv_cmd(json!(["set_property", "loop-playlist", list]));
    }

    /// Recharge la dernière file d'attente, en pause, à la position où on l'avait laissée.
    /// Comme Switify : ouvrir l'app ne relance jamais le son toute seule.
    fn restore_session(&mut self) {
        let s = self.lib.session.clone();
        let Ok(pos) = usize::try_from(s.pos) else { return };
        if pos >= s.queue.len() {
            return;
        }
        self.mpv_cmd(json!(["set_property", "pause", true]));
        for (i, t) in s.queue.iter().enumerate() {
            if i == pos && s.time > 1.0 {
                self.mpv_cmd(json!(["loadfile", t.url, "append", -1, format!("start={:.1}", s.time)]));
            } else {
                self.mpv_cmd(json!(["loadfile", t.url, "append"]));
            }
        }
        self.mpv_cmd(json!(["playlist-play-index", pos]));
        self.queue = s.queue;
    }

    fn save_session(&mut self, p: &mpv::Props) {
        let session = Session { queue: self.queue.clone(), pos: p.pos, time: p.time.unwrap_or(0.0) };
        if session != self.lib.session {
            self.lib.session = session;
            self.lib.save();
        }
        self.last_saved = Instant::now();
    }

    /// Session média Windows : titre/pochette au changement, état, progression.
    fn update_smtc(&mut self, p: &mpv::Props) {
        let Some(smtc) = self.smtc.as_mut() else { return };
        let Some(t) = usize::try_from(p.pos).ok().and_then(|i| self.queue.get(i)).cloned() else {
            if !self.smtc_state.0.is_empty() {
                self.smtc_state.0.clear();
                smtc.clear();
            }
            return;
        };
        let (artist, title) = lyrics::display_artist_title(&t);
        let cover = match &self.art {
            Some((u, Loaded::Ready(a))) if *u == t.url => a.file.clone(),
            _ => None,
        };
        if self.smtc_state.0 != t.url || (!self.smtc_state.1 && cover.is_some()) {
            smtc.set_track(&title, &artist, cover.as_deref(), &t.thumb);
            self.smtc_state = (t.url.clone(), cover.is_some(), -1.0, Instant::now());
        }
        smtc.set_status(Some(!p.pause));
        // Progression : toutes les 5 s, ou tout de suite après un saut dans le titre.
        if let (Some(pos), Some(dur)) = (p.time, p.duration) {
            let expected = self.smtc_state.2 + if p.pause { 0.0 } else { self.smtc_state.3.elapsed().as_secs_f64() };
            if self.smtc_state.2 < 0.0 || self.smtc_state.3.elapsed() > Duration::from_secs(5) || (pos - expected).abs() > 2.0 {
                smtc.set_timeline(pos, dur);
                self.smtc_state.2 = pos;
                self.smtc_state.3 = Instant::now();
            }
        }
    }

    fn start_search(&mut self, query: String) {
        let query = query.trim().to_string();
        let Some(bin) = self.bin.clone() else { return };
        if query.is_empty() {
            return;
        }
        fonts::ensure(&self.ctx, &query);
        self.query = query.clone();
        self.results_query = query.clone();
        self.search_status = SearchStatus::Searching;
        self.go(View::Search);
        let (tx, rx) = mpsc::channel();
        self.search_rx = Some(rx);
        let extra = self.lib.cookies.ytdlp_args();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(search::search(&bin, &query, &extra));
            ctx.request_repaint();
        });
    }

    fn poll_background(&mut self) {
        if let Some(rx) = &self.search_rx {
            if let Ok(res) = rx.try_recv() {
                self.search_rx = None;
                // Libère les miniatures de l'ancienne recherche qui ne servent plus ailleurs.
                for old in &self.results {
                    let used = self.queue.iter().chain(&self.lib.liked).chain(&self.lib.history).any(|q| q.thumb == old.thumb);
                    if !used {
                        self.ctx.forget_image(&old.thumb);
                    }
                }
                match res {
                    Ok(tracks) => {
                        let all: String = tracks.iter().flat_map(|t| [t.title.as_str(), t.artist.as_str()]).collect::<Vec<_>>().join(" ");
                        fonts::ensure(&self.ctx, &all);
                        self.search_status = if tracks.is_empty() { SearchStatus::Error("Aucun résultat.".into()) } else { SearchStatus::Done };
                        self.results = tracks;
                    }
                    Err(e) => {
                        self.results.clear();
                        self.search_status = SearchStatus::Error(e);
                    }
                }
            }
        }
        if let Some(rx) = &self.cookie_rx {
            if let Ok(res) = rx.try_recv() {
                self.cookie_rx = None;
                self.cookie_test = Some(Loaded::Ready(res));
            }
        }
        self.poll_import();
        while let Ok((url, lyr)) = self.lyrics_rx.try_recv() {
            match &lyr {
                Some(Lyrics::Synced(lines)) => fonts::ensure(&self.ctx, &lines.iter().map(|(_, l)| l.as_str()).collect::<String>()),
                Some(Lyrics::Plain(text)) => fonts::ensure(&self.ctx, text),
                _ => {}
            }
            self.lyrics.insert(url, lyr.map_or(Loaded::Missing, Loaded::Ready));
        }
        while let Ok((url, art)) = self.art_rx.try_recv() {
            if self.art.as_ref().is_some_and(|(u, _)| *u == url) {
                let state = match art {
                    Some(a) => Loaded::Ready(ArtReady {
                        file: a.file.clone(),
                        tex: self.ctx.load_texture("art", a.image, egui::TextureOptions::LINEAR),
                        colors: a.colors,
                        light: a.light,
                    }),
                    None => Loaded::Missing,
                };
                self.art = Some((url, state));
            }
        }
        while let Ok(key) = self.keys_rx.try_recv() {
            self.actions.push(match key {
                hotkeys::MediaKey::PlayPause => Action::TogglePause,
                hotkeys::MediaKey::Next => Action::Next,
                hotkeys::MediaKey::Prev => Action::Prev,
                hotkeys::MediaKey::Stop => Action::QueueClear,
            });
        }
    }

    /// Changement de titre : historique, paroles et pochette en arrière-plan.
    fn on_track_change(&mut self, p: &mpv::Props) {
        if p.pos == self.last_pos {
            return;
        }
        self.last_pos = p.pos;
        let Some(t) = self.current(p).cloned() else { return };
        self.lib.push_history(&t);
        // Historique seulement : la session (avec la position) est enregistrée toutes les
        // 15 s pendant la lecture et à la fermeture, quand le temps est connu.
        self.lib.save();

        if !self.lyrics.contains_key(&t.url) {
            if self.lyrics.len() > 30 {
                self.lyrics.clear();
            }
            self.lyrics.insert(t.url.clone(), Loaded::Loading);
            let (tx, ctx, track) = (self.lyrics_tx.clone(), self.ctx.clone(), t.clone());
            std::thread::spawn(move || {
                let _ = tx.send((track.url.clone(), lyrics::fetch(&track)));
                ctx.request_repaint();
            });
        }
        if self.art.as_ref().is_none_or(|(u, _)| *u != t.url) {
            self.art = Some((t.url.clone(), Loaded::Loading));
            let (tx, ctx) = (self.art_tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send((t.url.clone(), art::fetch(&t.thumb)));
                ctx.request_repaint();
            });
        }
    }

    fn poll_import(&mut self) {
        if let Some(rx) = &self.import.analyze_rx {
            if let Ok(res) = rx.try_recv() {
                self.import.analyze_rx = None;
                self.import.analyzing = false;
                match res {
                    Ok(cols) => {
                        let all: String = cols.iter().flat_map(|c| c.tracks.iter().flat_map(|t| [t.title.as_str(), t.artist.as_str()]).chain([c.name.as_str()])).collect::<Vec<_>>().join(" ");
                        fonts::ensure(&self.ctx, &all);
                        // Ajoute aux collections déjà analysées (plusieurs liens/fichiers possibles).
                        self.import.cols.extend(cols);
                        self.import.error = None;
                        self.import.done = false;
                    }
                    Err(e) => self.import.error = Some(e),
                }
            }
        }
        let mut finished = false;
        let msgs: Vec<import::Progress> = self.import.progress_rx.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
        {
            for msg in msgs {
                let st = &mut self.import;
                match msg {
                    import::Progress::Found(ci, _ti, track) => {
                        st.processed += 1;
                        st.found += 1;
                        st.since_save += 1;
                        match st.targets.get(ci).copied().flatten() {
                            Some(pl) => {
                                if let Some(p) = self.lib.playlists.get_mut(pl) {
                                    if !p.tracks.iter().any(|t| t.url == track.url) {
                                        p.tracks.push(track);
                                    }
                                }
                            }
                            None => {
                                if !self.lib.is_liked(&track.url) {
                                    self.lib.liked.push(track);
                                }
                            }
                        }
                    }
                    import::Progress::Missing(ci, ti) => {
                        st.processed += 1;
                        if let Some(t) = st.cols.get(ci).and_then(|c| c.tracks.get(ti)) {
                            st.missing.push(format!("{} – {}", t.artist, t.title));
                        }
                    }
                    import::Progress::Current(s) => st.current = s,
                    import::Progress::Error(e) => st.error = Some(e),
                    import::Progress::Done => finished = true,
                }
            }
            if self.import.since_save >= 20 {
                self.import.since_save = 0;
                self.lib.save();
            }
        }
        if finished {
            self.import.progress_rx = None;
            self.import.running = false;
            self.import.done = true;
            self.import.current.clear();
            // Playlists créées pour l'import mais restées vides (rien trouvé) : on les retire.
            let mut empty: Vec<usize> = self.import.targets.iter().flatten().copied().filter(|&i| self.lib.playlists.get(i).is_some_and(|p| p.tracks.is_empty())).collect();
            empty.sort_unstable_by(|a, b| b.cmp(a));
            for i in empty {
                self.lib.playlists.remove(i);
            }
            self.lib.save();
        }
    }

    fn import_start(&mut self) {
        let Some(bin) = self.bin.clone() else { return };
        let st = &mut self.import;
        if st.running || !st.cols.iter().any(|c| c.selected) {
            return;
        }
        st.targets.clear();
        for c in &st.cols {
            st.targets.push(match (c.selected, c.dest) {
                (true, import::Dest::Playlist) => {
                    let mut name = c.name.clone();
                    if self.lib.playlists.iter().any(|p| p.name == name) {
                        name = format!("{name} ({})", c.source);
                    }
                    self.lib.playlists.push(Playlist { name, tracks: vec![] });
                    Some(self.lib.playlists.len() - 1)
                }
                _ => None,
            });
        }
        self.lib.save();
        st.total = st.cols.iter().filter(|c| c.selected).map(|c| c.tracks.len()).sum();
        st.processed = 0;
        st.found = 0;
        st.missing.clear();
        st.error = None;
        st.done = false;
        st.running = true;
        st.since_save = 0;
        st.cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        st.progress_rx = Some(rx);
        let ctx = self.ctx.clone();
        import::start_matching(bin, self.lib.cookies.ytdlp_args(), st.cols.clone(), tx, st.cancel.clone(), move || ctx.request_repaint());
    }

    fn import_analyze(&mut self, job: impl FnOnce() -> Result<Vec<import::Collection>, String> + Send + 'static) {
        if self.import.analyzing {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.import.analyze_rx = Some(rx);
        self.import.analyzing = true;
        self.import.error = None;
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job());
            ctx.request_repaint();
        });
    }

    pub fn go(&mut self, view: View) {
        if self.view != view {
            self.back.push(self.view);
            self.back.truncate(20);
            self.view = view;
        }
    }

    fn play(&mut self, tracks: Vec<Track>, start: usize) {
        let Some(first) = tracks.get(start).cloned() else { return };
        let mut rest: Vec<Track> = tracks.into_iter().skip(start + 1).collect();
        if self.lib.shuffle {
            shuffle(&mut rest);
        }
        // `stop` vide la playlist de mpv ; on la reconstruit dans l'ordre.
        self.mpv_cmd(json!(["stop"]));
        self.mpv_cmd(json!(["set_property", "pause", false]));
        self.mpv_cmd(json!(["loadfile", first.url, "append-play"]));
        for t in &rest {
            self.mpv_cmd(json!(["loadfile", t.url, "append"]));
        }
        self.queue = std::iter::once(first).chain(rest).collect();
        self.last_pos = -2; // force la mise à jour même si la position reste 0
    }

    /// Mélange ce qui reste après le titre en cours (mpv garde le titre en cours).
    fn reshuffle_rest(&mut self, p: &mpv::Props) {
        let Ok(pos) = usize::try_from(p.pos) else { return };
        if pos + 1 >= self.queue.len() {
            return;
        }
        let current = self.queue[pos].clone();
        let mut rest = self.queue.split_off(pos + 1);
        shuffle(&mut rest);
        self.mpv_cmd(json!(["playlist-clear"]));
        for t in &rest {
            self.mpv_cmd(json!(["loadfile", t.url, "append"]));
        }
        self.queue = std::iter::once(current).chain(rest).collect();
        self.last_pos = 0;
    }

    fn apply_actions(&mut self, p: &mpv::Props) {
        for action in std::mem::take(&mut self.actions) {
            match action {
                Action::Play { tracks, start } => self.play(tracks, start),
                Action::Enqueue(t) => {
                    self.mpv_cmd(json!(["loadfile", t.url, "append-play"]));
                    self.queue.push(t);
                }
                Action::PlayNext(t) => {
                    if p.pos < 0 {
                        self.play(vec![t], 0);
                    } else {
                        let at = (p.pos + 1) as usize;
                        self.mpv_cmd(json!(["loadfile", t.url, "insert-at", at]));
                        self.queue.insert(at.min(self.queue.len()), t);
                    }
                }
                Action::ToggleLike(t) => {
                    self.lib.toggle_like(&t);
                    self.lib.save();
                }
                Action::AddToPlaylist(i, t) => {
                    if let Some(pl) = self.lib.playlists.get_mut(i) {
                        if !pl.tracks.iter().any(|x| x.url == t.url) {
                            pl.tracks.push(t);
                        }
                        self.lib.save();
                    }
                }
                Action::NewPlaylist(t) => {
                    let n = self.lib.playlists.len() + 1;
                    self.lib.playlists.push(Playlist { name: format!("Ma playlist n°{n}"), tracks: t.into_iter().collect() });
                    self.lib.save();
                    self.go(View::Playlist(n - 1));
                }
                Action::RemoveFromPlaylist(pl, i) => {
                    if let Some(pl) = self.lib.playlists.get_mut(pl) {
                        if i < pl.tracks.len() {
                            pl.tracks.remove(i);
                        }
                    }
                    self.lib.save();
                }
                Action::DeletePlaylist(i) => {
                    if i < self.lib.playlists.len() {
                        self.lib.playlists.remove(i);
                        self.lib.save();
                        self.back.retain(|v| !matches!(v, View::Playlist(_)));
                        self.view = View::Home;
                    }
                }
                Action::QueuePlay(i) => {
                    self.mpv_cmd(json!(["set_property", "pause", false]));
                    self.mpv_cmd(json!(["playlist-play-index", i]));
                }
                Action::QueueRemove(i) => {
                    if i < self.queue.len() {
                        self.mpv_cmd(json!(["playlist-remove", i]));
                        self.queue.remove(i);
                    }
                }
                Action::QueueClear => {
                    self.mpv_cmd(json!(["stop"]));
                    self.queue.clear();
                }
                Action::TogglePause => {
                    if p.pos >= 0 {
                        self.mpv_cmd(json!(["cycle", "pause"]));
                    }
                }
                Action::Next => self.mpv_cmd(json!(["playlist-next"])),
                Action::Prev => {
                    // Comme sur Spotify : au-delà de 3 s, « précédent » revient au début.
                    if p.time.unwrap_or(0.0) > 3.0 {
                        self.mpv_cmd(json!(["seek", 0, "absolute"]));
                    } else {
                        self.mpv_cmd(json!(["playlist-prev"]));
                    }
                }
                Action::Seek(t) => self.mpv_cmd(json!(["seek", t, "absolute"])),
                Action::SetVolume(v) => {
                    self.volume = v;
                    if v > 0.5 {
                        self.lib.volume = v; // couper le son ne doit pas être mémorisé
                    }
                    self.mpv_cmd(json!(["set_property", "volume", v.round()]));
                }
                Action::ToggleShuffle => {
                    self.lib.shuffle = !self.lib.shuffle;
                    if self.lib.shuffle {
                        self.reshuffle_rest(p);
                    }
                    self.lib.save();
                }
                Action::CycleRepeat => {
                    self.lib.repeat = self.lib.repeat.next();
                    self.apply_repeat();
                    self.lib.save();
                }
                Action::Go(v) => self.go(v),
                Action::Search(q) => self.start_search(q),
                Action::CopyLink(url) => self.ctx.copy_text(url),
                Action::SetCookies(c) => {
                    self.lib.cookies = c;
                    self.lib.save();
                    self.apply_cookies();
                    self.cookie_test = None;
                }
                Action::ImportLink(url) => self.import_analyze(move || import::fetch_link(&url)),
                Action::ImportPickFile => self.import_analyze(|| {
                    let path = rfd::FileDialog::new()
                        .set_title("Choisir un export Spotify ou Deezer")
                        .add_filter("Exports (CSV, JSON, TXT)", &["csv", "json", "txt"])
                        .pick_file()
                        .ok_or_else(|| "Aucun fichier choisi.".to_string())?;
                    import::parse_file(&path)
                }),
                Action::ImportFile(path) => self.import_analyze(move || import::parse_file(&path)),
                Action::ImportStart => self.import_start(),
                Action::ImportCancel => self.import.cancel.store(true, std::sync::atomic::Ordering::Relaxed),
                Action::ImportReset => {
                    if !self.import.running {
                        self.import = ImportState::default();
                    }
                }
                Action::SetAudioDevice(d) => {
                    self.mpv_cmd(json!(["set_property", "audio-device", d]));
                    self.lib.audio_device = d;
                    self.lib.save();
                }
                Action::TestCookies => {
                    if let Some(bin) = self.bin.clone() {
                        let extra = self.lib.cookies.ytdlp_args();
                        let (tx, rx) = mpsc::channel();
                        self.cookie_rx = Some(rx);
                        self.cookie_test = Some(Loaded::Loading);
                        let ctx = self.ctx.clone();
                        std::thread::spawn(move || {
                            let _ = tx.send(search::test_playback(&bin, &extra));
                            ctx.request_repaint();
                        });
                    }
                }
                Action::SetEqGains(gains, preset) => {
                    self.lib.eq_gains = gains;
                    self.lib.eq_preset = preset;
                    self.lib.eq_on = true;
                    self.apply_filters();
                    self.lib.save();
                }
                Action::ToggleEq => {
                    self.lib.eq_on = !self.lib.eq_on;
                    self.apply_filters();
                    self.lib.save();
                }
                Action::ToggleNormalize => {
                    self.lib.normalize = !self.lib.normalize;
                    self.apply_filters();
                    self.lib.save();
                }
                Action::SetSleep(m) => {
                    self.sleep = match m {
                        None => None,
                        Some(0) => Some(Sleep::EndOfTrack(p.pos)),
                        Some(min) => Some(Sleep::At(Instant::now() + Duration::from_secs(min * 60), min)),
                    };
                }
            }
        }
    }
}

/// Mélange de Fisher-Yates (xorshift initialisé sur l'horloge : pas besoin de mieux).
fn shuffle<T>(v: &mut [T]) {
    let mut x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0x9e37_79b9, |d| d.as_nanos() as u64) | 1;
    for i in (1..v.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        v.swap(i, (x % (i as u64 + 1)) as usize);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if !self.corners_done {
            self.corners_done = true;
            round_corners(frame);
            if self.autotest.is_none() {
                self.smtc = window_hwnd(frame).and_then(|h| {
                    let c = self.ctx.clone();
                    smtc::Smtc::new(h, self.keys_tx.clone(), move || c.request_repaint()).ok()
                });
                if self.smtc.is_none() {
                    let c = self.ctx.clone();
                    hotkeys::spawn(self.keys_tx.clone(), move || c.request_repaint());
                }
            }
        }
        self.poll_background();
        let mut p = self.props();
        // mpv annonce la nouvelle position avant d'avoir chargé le fichier : tant que le
        // fichier chargé n'est pas celui de la file à cette position, le temps est périmé.
        let loaded = self.current(&p).is_some_and(|t| p.path.as_deref() == Some(t.url.as_str()));
        if !loaded {
            p.time = None;
            p.duration = None;
        }
        if let Some(e) = self.mpv.as_ref().and_then(|m| m.props.lock().unwrap().error.take()) {
            self.show_error(e);
            p.error = None;
        }
        if (p.volume - self.volume).abs() > 0.5 && !ui.ctx().egui_is_using_pointer() && self.muted_volume.is_none() {
            self.volume = p.volume;
        }
        self.on_track_change(&p);
        self.check_sleep(&p);
        self.update_smtc(&p);
        if !p.pause && p.pos >= 0 && self.last_saved.elapsed() > Duration::from_secs(15) {
            self.save_session(&p);
        }

        views::shortcuts(self, ui, &p);
        views::render(self, ui, &p);

        let title = self.current(&p).map_or("Mixtape".to_string(), |t| format!("{} · Mixtape", t.title));
        if title != self.title {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        if self.autotest.is_some() {
            autotest::step(self, &p);
        }
        self.apply_actions(&p);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let p = self.props();
        self.save_session(&p);
        self.lib.save();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(views::BG).to_array()
    }
}

/// Heure locale (0-23), pour le message d'accueil.
pub fn local_hour() -> u16 {
    unsafe {
        let mut t: windows_sys::Win32::Foundation::SYSTEMTIME = std::mem::zeroed();
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t);
        t.wHour
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--gsmtc") {
        smtc::dump_sessions(std::path::Path::new(args.get(i + 1).map_or("gsmtc.txt", |s| s.as_str())));
        return Ok(());
    }
    let icon = image::load_from_memory(include_bytes!("../icons/icon.png")).ok().map(|img| {
        let img = img.into_rgba8();
        egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() }
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Mixtape")
        .with_inner_size([1440.0, 880.0])
        .with_min_inner_size([900.0, 600.0])
        .with_decorations(false)
        .with_resizable(true);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, multisampling: 0, depth_buffer: 0, stencil_buffer: 0, ..Default::default() };
    eframe::run_native("Mixtape", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
