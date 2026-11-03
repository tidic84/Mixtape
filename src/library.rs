//! Bibliothèque locale : titres likés, historique, playlists, préférences et dernière
//! session. Tout tient dans un JSON sous %APPDATA%\Mixtape.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::search::Track;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    pub fn next(self) -> Self {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// Cookies YouTube transmis à yt-dlp (contourne le « confirm you're not a bot »).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Cookies {
    #[default]
    None,
    /// Navigateur dont yt-dlp lit les cookies : firefox, edge, chrome, brave…
    Browser(String),
    /// Fichier cookies.txt (format Netscape).
    File(String),
}

impl Cookies {
    /// Arguments de ligne de commande pour yt-dlp.
    pub fn ytdlp_args(&self) -> Vec<String> {
        match self {
            Cookies::None => vec![],
            Cookies::Browser(b) => vec!["--cookies-from-browser".into(), b.clone()],
            Cookies::File(f) if !f.trim().is_empty() => vec!["--cookies".into(), f.trim().into()],
            Cookies::File(_) => vec![],
        }
    }

    /// Valeur de l'option mpv `ytdl-raw-options` (transmise au yt-dlp lancé par mpv).
    pub fn mpv_raw_options(&self) -> String {
        match self {
            Cookies::None => String::new(),
            Cookies::Browser(b) => format!("cookies-from-browser={b}"),
            Cookies::File(f) if !f.trim().is_empty() => format!("cookies={}", f.trim()),
            Cookies::File(_) => String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub tracks: Vec<Track>,
}

/// File d'attente et position au moment de la fermeture (restaurées en pause).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub queue: Vec<Track>,
    pub pos: i64,
    pub time: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub liked: Vec<Track>,
    pub history: Vec<Track>,
    pub playlists: Vec<Playlist>,
    pub volume: f64,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub session: Session,
    /// Égaliseur 6 bandes (60 Hz → 15 kHz), gains en dB.
    pub eq_on: bool,
    pub eq_preset: String,
    pub eq_gains: [f32; 6],
    /// Normalisation dynamique du volume (filtre dynaudnorm de mpv).
    pub normalize: bool,
    pub cookies: Cookies,
    /// Sortie audio (identifiant mpv, ex. « wasapi/{…} ») ; « auto » = périphérique par défaut.
    pub audio_device: String,
    #[serde(skip)]
    path: PathBuf,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            liked: vec![],
            history: vec![],
            playlists: vec![],
            volume: 70.0,
            shuffle: false,
            repeat: Repeat::Off,
            session: Session::default(),
            eq_on: false,
            eq_preset: "Plat".into(),
            eq_gains: [0.0; 6],
            normalize: false,
            cookies: Cookies::None,
            audio_device: "auto".into(),
            path: PathBuf::new(),
        }
    }
}

const HISTORY_MAX: usize = 50;

impl Library {
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join("Mixtape").join("library.json")
    }

    pub fn load(path: PathBuf) -> Self {
        let mut lib: Library = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        lib.path = path;
        lib
    }

    /// Écriture atomique (fichier temporaire puis renommage).
    pub fn save(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("json.tmp");
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }

    pub fn is_liked(&self, url: &str) -> bool {
        self.liked.iter().any(|t| t.url == url)
    }

    pub fn toggle_like(&mut self, track: &Track) {
        if let Some(i) = self.liked.iter().position(|t| t.url == track.url) {
            self.liked.remove(i);
        } else {
            self.liked.insert(0, track.clone());
        }
    }

    pub fn push_history(&mut self, track: &Track) {
        self.history.retain(|t| t.url != track.url);
        self.history.insert(0, track.clone());
        self.history.truncate(HISTORY_MAX);
    }
}

pub const EQ_BANDS: [(u32, &str); 6] = [(60, "60"), (150, "150"), (400, "400"), (1000, "1k"), (2400, "2k"), (15000, "15k")];

pub const EQ_PRESETS: [(&str, [f32; 6]); 7] = [
    ("Plat", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("Acoustique", [3.0, 2.0, 0.0, 1.0, 2.5, 3.0]),
    ("Basses +", [6.0, 4.0, 1.0, 0.0, 0.0, 0.0]),
    ("Aigus +", [0.0, 0.0, 0.0, 1.5, 4.0, 6.0]),
    ("Voix", [-2.0, -1.0, 2.0, 4.0, 3.0, 0.0]),
    ("Électro", [5.0, 3.0, -1.0, 0.0, 2.0, 4.0]),
    ("Rock", [4.0, 2.0, -2.0, 0.0, 3.0, 4.0]),
];

impl Library {
    /// Chaîne de filtres audio mpv (vide = aucun filtre).
    pub fn audio_filters(&self) -> String {
        let mut chain: Vec<String> = vec![];
        if self.eq_on {
            for ((f, _), g) in EQ_BANDS.iter().zip(self.eq_gains) {
                if g.abs() >= 0.1 {
                    chain.push(format!("equalizer=f={f}:t=o:w=1.3:g={g:.1}"));
                }
            }
        }
        if self.normalize {
            chain.push("dynaudnorm=f=300:g=15".into());
        }
        if chain.is_empty() { String::new() } else { format!("lavfi=[{}]", chain.join(",")) }
    }
}
