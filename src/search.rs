//! Recherche YouTube (ou lecture d'une URL de playlist) via yt-dlp.

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub duration: Option<f64>,
    pub url: String,
    pub thumb: String,
}

fn to_track(e: &Value) -> Option<Track> {
    let id = e["id"].as_str()?;
    // Seulement des vidéos (id de 11 caractères) : on écarte chaînes et playlists.
    if id.len() != 11 {
        return None;
    }
    let artist = ["channel", "uploader", "artist"]
        .iter()
        .find_map(|k| e[*k].as_str())
        .unwrap_or("")
        .trim_end_matches(" - Topic");
    Some(Track {
        title: e["title"].as_str().unwrap_or("Sans titre").to_string(),
        artist: artist.to_string(),
        duration: e["duration"].as_f64(),
        url: format!("https://www.youtube.com/watch?v={id}"),
        thumb: format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"),
    })
}

/// Bloquant (~3 s) : à appeler depuis un fil secondaire.
pub fn search(bin: &Path, query: &str, extra: &[String]) -> Result<Vec<Track>, String> {
    let q = query.trim();
    let target = if q.starts_with("http") { q.to_string() } else { format!("ytsearch20:{q}") };
    let out = Command::new(bin.join("yt-dlp.exe"))
        .args(["-J", "--flat-playlist", "--no-warnings", "--playlist-end", "300"])
        .args(extra)
        .arg(&target)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("yt-dlp : {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().last().unwrap_or("échec de yt-dlp").to_string());
    }
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    Ok(match v["entries"].as_array() {
        Some(entries) => entries.iter().filter_map(to_track).collect(),
        None => to_track(&v).into_iter().collect(),
    })
}

pub fn fmt_time(sec: Option<f64>) -> String {
    let Some(sec) = sec.filter(|s| s.is_finite() && *s >= 0.0) else { return "0:00".into() };
    let s = sec as u64;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Essaie d'obtenir le flux audio d'une vidéo connue : vérifie que YouTube accepte
/// la lecture avec les cookies choisis. Renvoie le message d'erreur de yt-dlp sinon.
pub fn test_playback(bin: &Path, extra: &[String]) -> Result<(), String> {
    let out = Command::new(bin.join("yt-dlp.exe"))
        .args(["--no-warnings", "-f", "bestaudio", "-g"])
        .args(extra)
        .arg("https://www.youtube.com/watch?v=A2VpR8HahKc")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("yt-dlp : {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err.lines().filter(|l| l.contains("ERROR")).last().or(err.lines().last()).unwrap_or("échec de yt-dlp").trim().to_string())
    }
}
