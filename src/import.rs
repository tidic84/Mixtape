//! Import de bibliothèques Spotify et Deezer.
//!
//! 1. On récupère des listes de titres (titre, artiste, durée) :
//!    - lien de playlist/album Spotify public (page « embed », 100 premiers titres max) ;
//!    - lien de playlist/album Deezer, ou profil Deezer public (coups de cœur + playlists),
//!      via l'API publique de Deezer ;
//!    - fichier d'export : CSV (Tune My Music, Exportify…), JSON des données Spotify,
//!      ou texte « Artiste - Titre ».
//! 2. Chaque titre est retrouvé sur YouTube (recherches yt-dlp groupées par lots) et le
//!    meilleur résultat est choisi selon la durée, la chaîne et le titre.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use serde_json::Value;

use crate::search::Track;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Mixtape/0.2";

#[derive(Clone, Debug)]
pub struct SrcTrack {
    pub title: String,
    pub artist: String,
    pub duration: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dest {
    Liked,
    Playlist,
}

#[derive(Clone, Debug)]
pub struct Collection {
    pub name: String,
    pub source: String,
    pub dest: Dest,
    pub tracks: Vec<SrcTrack>,
    /// Liste incomplète (Spotify sans compte : 100 premiers titres seulement).
    pub truncated: bool,
    pub selected: bool,
}

// ---------------------------------------------------------------------------------
// Récupération des listes
// ---------------------------------------------------------------------------------

fn get(url: &str) -> Result<ehttp::Response, String> {
    let mut req = ehttp::Request::get(url);
    req.headers.insert("User-Agent", UA);
    req.headers.insert("Accept-Language", "fr-FR,fr;q=0.9,en;q=0.8");
    let resp = ehttp::fetch_blocking(&req)?;
    if !resp.ok {
        return Err(format!("{} ({url})", resp.status));
    }
    Ok(resp)
}

fn get_json(url: &str) -> Result<Value, String> {
    let resp = get(url)?;
    let v: Value = serde_json::from_slice(&resp.bytes).map_err(|e| e.to_string())?;
    if let Some(err) = v.get("error") {
        return Err(err["message"].as_str().unwrap_or("erreur Deezer").to_string());
    }
    Ok(v)
}

/// Extrait (type, identifiant) après un segment de chemin connu, ex. `/playlist/123`.
fn path_id(url: &str, kinds: &[&str]) -> Option<(String, String)> {
    let path = url.split(['?', '#']).next()?;
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    for w in parts.windows(2) {
        if kinds.contains(&w[0]) {
            return Some((w[0].to_string(), w[1].to_string()));
        }
    }
    None
}

/// Analyse un lien Spotify ou Deezer. Bloquant (réseau) : à appeler hors de l'interface.
pub fn fetch_link(url: &str) -> Result<Vec<Collection>, String> {
    let url = url.trim();
    if url.contains("spotify.com") || url.starts_with("spotify:") {
        return spotify_link(url);
    }
    if url.contains("deezer.com") || url.contains("deezer.page.link") || url.contains("link.deezer") {
        // Les liens courts (partage depuis l'app) redirigent vers la vraie adresse.
        let real = if url.contains("deezer.com/") && !url.contains("link.deezer") { url.to_string() } else { get(url)?.url };
        return deezer_link(&real);
    }
    Err("Lien non reconnu : colle un lien de playlist ou d'album Spotify, ou un lien Deezer (playlist, album, profil).".into())
}

fn spotify_link(url: &str) -> Result<Vec<Collection>, String> {
    let url = url.replace("spotify:", "https://open.spotify.com/").replace(':', "/").replacen("https///", "https://", 1);
    let Some((kind, id)) = path_id(&url, &["playlist", "album"]) else {
        return Err("Lien Spotify non pris en charge : il faut une playlist ou un album.".into());
    };
    let resp = get(&format!("https://open.spotify.com/embed/{kind}/{id}"))?;
    let html = String::from_utf8_lossy(&resp.bytes);
    let start = html.find("<script id=\"__NEXT_DATA__\" type=\"application/json\">").ok_or("page Spotify inattendue")?;
    let rest = &html[start..];
    let json_start = rest.find('>').ok_or("page Spotify inattendue")? + 1;
    let json_end = rest.find("</script>").ok_or("page Spotify inattendue")?;
    let v: Value = serde_json::from_str(&rest[json_start..json_end]).map_err(|e| e.to_string())?;
    let entity = &v["props"]["pageProps"]["state"]["data"]["entity"];
    let list = entity["trackList"].as_array().ok_or("playlist privée ou introuvable")?;
    let tracks: Vec<SrcTrack> = list
        .iter()
        .filter_map(|t| {
            Some(SrcTrack {
                title: t["title"].as_str()?.to_string(),
                artist: t["subtitle"].as_str().unwrap_or("").replace('\u{a0}', " "),
                duration: t["duration"].as_f64().map(|ms| ms / 1000.0),
            })
        })
        .collect();
    let name = entity["name"].as_str().or(entity["title"].as_str()).unwrap_or("Playlist Spotify").to_string();
    Ok(vec![Collection {
        truncated: tracks.len() >= 100,
        name,
        source: "Spotify".into(),
        dest: Dest::Playlist,
        tracks,
        selected: true,
    }])
}

fn deezer_tracks(first_url: &str) -> Result<Vec<SrcTrack>, String> {
    let mut out = vec![];
    let mut url = Some(first_url.to_string());
    let mut guard = 0;
    while let Some(u) = url.take() {
        let v = get_json(&u)?;
        for t in v["data"].as_array().into_iter().flatten() {
            if let Some(title) = t["title"].as_str() {
                out.push(SrcTrack {
                    title: title.to_string(),
                    artist: t["artist"]["name"].as_str().unwrap_or("").to_string(),
                    duration: t["duration"].as_f64(),
                });
            }
        }
        url = v["next"].as_str().map(str::to_string);
        guard += 1;
        if guard > 200 {
            break;
        }
    }
    Ok(out)
}

fn deezer_link(url: &str) -> Result<Vec<Collection>, String> {
    let Some((kind, id)) = path_id(url, &["playlist", "album", "profile", "user"]) else {
        return Err("Lien Deezer non pris en charge : playlist, album ou profil.".into());
    };
    let api = "https://api.deezer.com";
    match kind.as_str() {
        "playlist" | "album" => {
            let info = get_json(&format!("{api}/{kind}/{id}"))?;
            let tracks = deezer_tracks(&format!("{api}/{kind}/{id}/tracks?limit=100"))?;
            let loved = info["is_loved_track"].as_bool() == Some(true);
            Ok(vec![Collection {
                name: info["title"].as_str().unwrap_or("Playlist Deezer").to_string(),
                source: "Deezer".into(),
                dest: if loved { Dest::Liked } else { Dest::Playlist },
                tracks,
                truncated: false,
                selected: true,
            }])
        }
        _ => {
            // Profil public : coups de cœur + playlists.
            let user = get_json(&format!("{api}/user/{id}"))?;
            let who = user["name"].as_str().unwrap_or("Deezer").to_string();
            let mut cols = vec![];
            if let Ok(fav) = deezer_tracks(&format!("{api}/user/{id}/tracks?limit=100")) {
                if !fav.is_empty() {
                    cols.push(Collection { name: format!("Coups de cœur de {who}"), source: "Deezer".into(), dest: Dest::Liked, tracks: fav, truncated: false, selected: true });
                }
            }
            let pls = get_json(&format!("{api}/user/{id}/playlists?limit=100"))?;
            for pl in pls["data"].as_array().into_iter().flatten() {
                if pl["is_loved_track"].as_bool() == Some(true) {
                    continue; // déjà couverts par les coups de cœur
                }
                let Some(pid) = pl["id"].as_u64() else { continue };
                let tracks = deezer_tracks(&format!("{api}/playlist/{pid}/tracks?limit=100")).unwrap_or_default();
                if tracks.is_empty() {
                    continue;
                }
                cols.push(Collection {
                    name: pl["title"].as_str().unwrap_or("Playlist").to_string(),
                    source: "Deezer".into(),
                    dest: Dest::Playlist,
                    tracks,
                    truncated: false,
                    selected: true,
                });
            }
            if cols.is_empty() {
                return Err("Rien de public sur ce profil Deezer (profil privé ?).".into());
            }
            Ok(cols)
        }
    }
}

// ---------------------------------------------------------------------------------
// Fichiers d'export
// ---------------------------------------------------------------------------------

/// Découpe une ligne CSV (guillemets doublés gérés). Séparateur `,` ou `;`.
fn csv_line(line: &str, sep: char) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

fn is_liked_name(name: &str) -> bool {
    let n = name.to_lowercase();
    ["liked", "titres likés", "favori", "coups de cœur", "coups de coeur", "loved", "favorite"].iter().any(|k| n.contains(k))
}

pub fn parse_file(path: &Path) -> Result<Vec<Collection>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("lecture impossible : {e}"))?;
    let text = String::from_utf8_lossy(&bytes).trim_start_matches('\u{feff}').to_string();
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Import").to_string();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    if ext == "json" {
        return parse_spotify_json(&text, &stem);
    }
    let first = text.lines().next().unwrap_or("");
    if ext == "csv" || first.contains(',') && first.to_lowercase().contains("artist") {
        return parse_csv(&text, &stem);
    }
    // Texte libre : « Artiste - Titre » par ligne.
    let tracks: Vec<SrcTrack> = text
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let (a, t) = l.split_once(" - ").or_else(|| l.split_once(" – "))?;
            Some(SrcTrack { artist: a.trim().into(), title: t.trim().into(), duration: None })
        })
        .collect();
    if tracks.is_empty() {
        return Err("Format de fichier non reconnu (CSV, JSON Spotify, ou lignes « Artiste - Titre »).".into());
    }
    let dest = if is_liked_name(&stem) { Dest::Liked } else { Dest::Playlist };
    Ok(vec![Collection { name: stem, source: "Fichier".into(), dest, tracks, truncated: false, selected: true }])
}

fn parse_csv(text: &str, stem: &str) -> Result<Vec<Collection>, String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header_line = lines.next().ok_or("fichier vide")?;
    let sep = if header_line.matches(';').count() > header_line.matches(',').count() { ';' } else { ',' };
    let header: Vec<String> = csv_line(header_line, sep).into_iter().map(|h| h.to_lowercase()).collect();
    // Par ordre de préférence : nom exact, puis préfixe (« artist name(s) »), en évitant
    // les colonnes d'identifiants (« track uri », « artist uri(s) »…).
    let col = |names: &[&str]| {
        let usable = |h: &String| !h.contains("uri") && !h.contains(" id") && !h.ends_with("id");
        names
            .iter()
            .find_map(|n| header.iter().position(|h| h == n))
            .or_else(|| names.iter().find_map(|n| header.iter().position(|h| h.starts_with(n) && usable(h))))
    };
    let title_i = col(&["track name", "track", "title", "titre", "name", "song"]).ok_or("colonne du titre introuvable")?;
    let artist_i = col(&["artist name", "artist", "artiste", "artists"]).ok_or("colonne de l'artiste introuvable")?;
    let dur_i = col(&["duration (ms)", "duration_ms", "duration", "durée"]);
    let pl_i = col(&["playlist name", "playlist"]);
    let type_i = col(&["type"]);

    let mut cols: Vec<Collection> = vec![];
    for line in lines {
        let f = csv_line(line, sep);
        let Some(title) = f.get(title_i).filter(|s| !s.is_empty()) else { continue };
        // Exportify : « Artist Name(s) » séparés par des virgules → premier artiste suffit.
        let artist = f.get(artist_i).cloned().unwrap_or_default();
        let duration = dur_i.and_then(|i| f.get(i)).and_then(|d| d.parse::<f64>().ok()).map(|d| if d > 10_000.0 { d / 1000.0 } else { d });
        let playlist = pl_i.and_then(|i| f.get(i)).filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| stem.to_string());
        let liked = is_liked_name(&playlist) || type_i.and_then(|i| f.get(i)).is_some_and(|t| is_liked_name(t));
        let name = if liked && pl_i.is_none() { stem.to_string() } else { playlist };
        let track = SrcTrack { title: title.clone(), artist, duration };
        match cols.iter_mut().find(|c| c.name == name) {
            Some(c) => c.tracks.push(track),
            None => cols.push(Collection {
                dest: if liked || is_liked_name(&name) { Dest::Liked } else { Dest::Playlist },
                name,
                source: "Fichier".into(),
                tracks: vec![track],
                truncated: false,
                selected: true,
            }),
        }
    }
    if cols.is_empty() {
        return Err("Aucun titre trouvé dans le fichier.".into());
    }
    Ok(cols)
}

/// Export des données Spotify : YourLibrary.json (titres likés) et Playlist*.json.
fn parse_spotify_json(text: &str, stem: &str) -> Result<Vec<Collection>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("JSON invalide : {e}"))?;
    let mut cols = vec![];
    if let Some(tracks) = v["tracks"].as_array() {
        let list: Vec<SrcTrack> = tracks
            .iter()
            .filter_map(|t| Some(SrcTrack { title: t["track"].as_str()?.into(), artist: t["artist"].as_str().unwrap_or("").into(), duration: None }))
            .collect();
        if !list.is_empty() {
            cols.push(Collection { name: "Titres likés Spotify".into(), source: "Spotify".into(), dest: Dest::Liked, tracks: list, truncated: false, selected: true });
        }
    }
    for pl in v["playlists"].as_array().into_iter().flatten() {
        let list: Vec<SrcTrack> = pl["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|it| {
                let t = &it["track"];
                Some(SrcTrack { title: t["trackName"].as_str()?.into(), artist: t["artistName"].as_str().unwrap_or("").into(), duration: None })
            })
            .collect();
        if !list.is_empty() {
            let name = pl["name"].as_str().unwrap_or(stem).to_string();
            cols.push(Collection { dest: if is_liked_name(&name) { Dest::Liked } else { Dest::Playlist }, name, source: "Spotify".into(), tracks: list, truncated: false, selected: true });
        }
    }
    if cols.is_empty() {
        return Err("Ce JSON ne ressemble pas à un export de données Spotify (YourLibrary.json ou Playlist1.json).".into());
    }
    Ok(cols)
}

// ---------------------------------------------------------------------------------
// Correspondance YouTube
// ---------------------------------------------------------------------------------

pub enum Progress {
    /// (collection, index du titre, résultat)
    Found(usize, usize, Track),
    Missing(usize, usize),
    Current(String),
    Error(String),
    Done,
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_feat(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let lower = out.to_lowercase();
    for key in [" feat.", " ft.", " feat ", " featuring "] {
        if let Some(i) = lower.find(key) {
            return out[..i].trim().to_string();
        }
    }
    out.trim().to_string()
}

fn first_artist(a: &str) -> String {
    a.split([',', '&', ';']).next().unwrap_or(a).trim().to_string()
}

/// Score d'un résultat YouTube pour un titre source (plus haut = meilleur).
fn score(src: &SrcTrack, e: &Value, rank: usize) -> i32 {
    let title = norm(e["title"].as_str().unwrap_or(""));
    let channel = e["channel"].as_str().or(e["uploader"].as_str()).unwrap_or("");
    let chan = norm(channel);
    let want_title = norm(&strip_feat(&src.title));
    let want_artist = norm(&first_artist(&src.artist));
    let src_full = norm(&format!("{} {}", src.title, src.artist));
    let mut s = 12 - 3 * rank as i32;

    if !want_title.is_empty() && title.contains(&want_title) {
        s += 25;
    } else {
        let words: Vec<&str> = want_title.split(' ').filter(|w| w.len() > 1).collect();
        let hit = words.iter().filter(|w| title.contains(**w)).count();
        s += if words.is_empty() { 0 } else { (hit * 20 / words.len()) as i32 } - 15;
    }
    if !want_artist.is_empty() && chan.contains(&want_artist) {
        s += 25; // publié par l'artiste lui-même
    } else if !want_artist.is_empty() && title.contains(&want_artist) {
        s += 10;
    }
    // Mots du titre YouTube absents de la source (autres artistes, émission, année…).
    const NEUTRAL: [&str; 16] = ["official", "video", "audio", "music", "lyrics", "lyric", "clip", "remastered", "remaster", "version", "feat", "ft", "hd", "hq", "officiel", "vidéo"];
    let extra = title.split(' ').filter(|w| w.len() > 2 && !NEUTRAL.contains(w) && !src_full.contains(*w)).count();
    s -= (extra as i32 * 4).min(28);
    if channel.ends_with(" - Topic") {
        s += 20; // « Topic » = audio officiel publié par le label
    }
    if channel.to_lowercase().contains("vevo") {
        s += 8;
    }
    if let (Some(want), Some(got)) = (src.duration, e["duration"].as_f64()) {
        let d = (want - got).abs();
        s += if d <= 3.0 { 25 } else if d <= 8.0 { 18 } else if d <= 20.0 { 5 } else if d <= 45.0 { -8 } else { -30 };
    }
    // Préférer l'audio/clip officiel aux vidéos de paroles publiées par des tiers.
    if title.contains("official audio") || title.contains("audio officiel") {
        s += 10;
    } else if title.contains("official") {
        s += 4;
    }
    for meh in ["lyrics", "lyric", "paroles", "tiktok", "letra"] {
        if title.contains(meh) && !src_full.contains(meh) {
            s -= if meh == "tiktok" { 15 } else { 8 };
            break;
        }
    }
    // Versions à éviter, sauf si la source les demande explicitement.
    for bad in ["live", "cover", "remix", "karaoke", "instrumental", "nightcore", "sped up", "slowed", "reverb", "8d", "reaction", "tutorial", "lesson", "1 hour", "10 hours"] {
        if title.contains(bad) && !src_full.contains(bad) {
            s -= 30;
        }
    }
    s
}

/// Lance la recherche YouTube de tous les titres sélectionnés (fil d'arrière-plan).
pub fn start_matching(bin: PathBuf, cookies: Vec<String>, cols: Vec<Collection>, tx: mpsc::Sender<Progress>, cancel: Arc<AtomicBool>, wake: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        let jobs: Vec<(usize, usize, SrcTrack)> = cols
            .iter()
            .enumerate()
            .filter(|(_, c)| c.selected)
            .flat_map(|(ci, c)| c.tracks.iter().cloned().enumerate().map(move |(ti, t)| (ci, ti, t)))
            .collect();
        for batch in jobs.chunks(20) {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let _ = tx.send(Progress::Current(format!("{} – {}", batch[0].2.artist, batch[0].2.title)));
            wake();
            let queries: Vec<String> = batch
                .iter()
                .map(|(_, _, t)| format!("ytsearch5:{} {}", first_artist(&t.artist), strip_feat(&t.title)))
                .collect();
            let out = Command::new(bin.join("yt-dlp.exe"))
                .args(["--no-warnings", "--ignore-errors", "-J", "--flat-playlist"])
                .args(&cookies)
                .args(&queries)
                .creation_flags(CREATE_NO_WINDOW)
                .output();
            let out = match out {
                Ok(o) => o,
                Err(e) => {
                    let _ = tx.send(Progress::Error(format!("yt-dlp : {e}")));
                    break;
                }
            };
            // Une ligne JSON par recherche, dans l'ordre (une ligne peut manquer en cas d'erreur).
            let results: Vec<Value> = String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
            for (k, (ci, ti, src)) in batch.iter().enumerate() {
                let want = format!("ytsearch5:{} {}", first_artist(&src.artist), strip_feat(&src.title));
                let res = results.iter().find(|r| r["webpage_url"].as_str() == Some(want.as_str()) || r["original_url"].as_str() == Some(want.as_str())).or(results.get(k));
                let best = res.and_then(|r| r["entries"].as_array()).and_then(|entries| {
                    entries
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e["id"].as_str().is_some_and(|id| id.len() == 11))
                        .map(|(i, e)| (score(src, e, i), e))
                        .max_by_key(|(s, _)| *s)
                        .filter(|(s, _)| *s >= 25)
                        .map(|(_, e)| e.clone())
                });
                let msg = match best {
                    Some(e) => {
                        let id = e["id"].as_str().unwrap_or_default();
                        Progress::Found(*ci, *ti, Track {
                            title: e["title"].as_str().unwrap_or(&src.title).to_string(),
                            artist: e["channel"].as_str().unwrap_or(&src.artist).trim_end_matches(" - Topic").to_string(),
                            duration: e["duration"].as_f64().or(src.duration),
                            url: format!("https://www.youtube.com/watch?v={id}"),
                            thumb: format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"),
                        })
                    }
                    None => Progress::Missing(*ci, *ti),
                };
                let _ = tx.send(msg);
            }
            wake();
        }
        let _ = tx.send(Progress::Done);
        wake();
    });
}
