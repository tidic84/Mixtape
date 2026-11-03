//! Paroles via LRCLIB (https://lrclib.net) : gratuit, sans clé, paroles synchronisées
//! au format LRC quand elles existent.

use serde_json::Value;

use crate::search::Track;

#[derive(Clone, Debug)]
pub enum Lyrics {
    /// Lignes horodatées (secondes, texte), triées.
    Synced(Vec<(f64, String)>),
    Plain(String),
    Instrumental,
}

const UA: &str = "Mixtape/0.2 (lecteur musique open source)";

/// Devine (artiste, titre) depuis un titre YouTube du genre
/// « Daft Punk - One More Time (Official Video) [HD] ».
pub fn guess_artist_title(track: &Track) -> (String, String) {
    let mut title = String::new();
    let mut depth = 0i32;
    for c in track.title.chars() {
        match c {
            '(' | '[' | '【' => depth += 1,
            ')' | ']' | '】' => depth = (depth - 1).max(0),
            _ if depth == 0 => title.push(c),
            _ => {}
        }
    }
    for sep in [" - ", " – ", " — ", " | "] {
        if let Some((a, t)) = title.split_once(sep) {
            return (clean(a), strip_feat(&clean(t)));
        }
    }
    let artist = track
        .artist
        .trim_end_matches("VEVO")
        .trim_end_matches("Vevo")
        .trim_end_matches(" - Topic")
        .trim_end_matches("Official")
        .to_string();
    (clean(&artist), strip_feat(&clean(&title)))
}

/// Pour l'affichage : comme `guess_artist_title`, mais ne retire que les mentions
/// parasites entre parenthèses (Official Video, Lyrics, HD…) et garde les versions
/// (12 Mix, Live, Radio Edit…).
pub fn display_artist_title(track: &Track) -> (String, String) {
    const NOISE: [&str; 15] = ["official", "video", "audio", "lyric", "lyrics", "paroles", "clip", "hd", "hq", "4k", "visualizer", "visualiser", "mv", "music video", "vidéo"];
    let mut title = String::new();
    let mut seg = String::new();
    let mut depth = 0i32;
    for c in track.title.chars() {
        match c {
            '(' | '[' | '【' => {
                depth += 1;
                if depth == 1 {
                    seg.clear();
                }
                seg.push(c);
            }
            ')' | ']' | '】' if depth > 0 => {
                seg.push(c);
                depth -= 1;
                if depth == 0 {
                    let lower = seg.to_lowercase();
                    let noisy = NOISE.iter().any(|n| lower.split(|ch: char| !ch.is_alphanumeric()).any(|w| w == *n) || (n.contains(' ') && lower.contains(n)));
                    if !noisy {
                        title.push_str(&seg);
                    }
                }
            }
            _ if depth > 0 => seg.push(c),
            _ => title.push(c),
        }
    }
    for sep in [" - ", " – ", " — ", " | "] {
        if let Some((a, t)) = title.split_once(sep) {
            return (clean(a), clean(t));
        }
    }
    let artist = track.artist.trim_end_matches("VEVO").trim_end_matches(" - Topic").to_string();
    (clean(&artist), clean(&title))
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|c: char| c == '"' || c == '\'').to_string()
}

fn strip_feat(s: &str) -> String {
    let lower = s.to_lowercase();
    for key in [" ft.", " ft ", " feat.", " feat ", " featuring "] {
        if let Some(i) = lower.find(key) {
            return s[..i].trim().to_string();
        }
    }
    s.to_string()
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn get(url: &str) -> Option<Value> {
    let mut req = ehttp::Request::get(url);
    req.headers.insert("User-Agent", UA);
    let resp = ehttp::fetch_blocking(&req).ok()?;
    if !resp.ok {
        return None;
    }
    serde_json::from_slice(&resp.bytes).ok()
}

/// Bloquant : à appeler depuis un fil secondaire.
pub fn fetch(track: &Track) -> Option<Lyrics> {
    let (artist, title) = guess_artist_title(track);
    if title.is_empty() {
        return None;
    }
    let mut candidates = vec![];
    if !artist.is_empty() {
        let url = format!(
            "https://lrclib.net/api/search?track_name={}&artist_name={}",
            encode(&title),
            encode(&artist)
        );
        if let Some(Value::Array(a)) = get(&url) {
            candidates = a;
        }
    }
    if candidates.is_empty() {
        let url = format!("https://lrclib.net/api/search?q={}", encode(&format!("{artist} {title}")));
        if let Some(Value::Array(a)) = get(&url) {
            candidates = a;
        }
    }
    pick(&candidates, track.duration)
}

/// Préfère des paroles synchronisées d'une durée proche (versions live/radio edit…).
fn pick(candidates: &[Value], duration: Option<f64>) -> Option<Lyrics> {
    let close = |c: &Value| match (duration, c["duration"].as_f64()) {
        (Some(d), Some(cd)) => (d - cd).abs() <= 8.0,
        _ => true,
    };
    fn field<'a>(c: &'a Value, key: &str) -> Option<&'a str> {
        c[key].as_str().filter(|s| !s.trim().is_empty())
    }
    let synced = |c: &'_ Value| -> Option<String> { field(c, "syncedLyrics").map(str::to_string) };
    let plain = |c: &'_ Value| -> Option<String> { field(c, "plainLyrics").map(str::to_string) };

    if let Some(s) = candidates.iter().filter(|c| close(c)).find_map(synced) {
        return Some(Lyrics::Synced(parse_lrc(&s)));
    }
    if let Some(p) = candidates.iter().filter(|c| close(c)).find_map(plain) {
        return Some(Lyrics::Plain(p));
    }
    if candidates.iter().filter(|c| close(c)).any(|c| c["instrumental"].as_bool() == Some(true)) {
        return Some(Lyrics::Instrumental);
    }
    // Rien à la bonne durée : on accepte la première version synchronisée trouvée.
    candidates.iter().find_map(synced).map(|s| Lyrics::Synced(parse_lrc(&s)))
}

/// « [01:23.45] texte » (éventuellement plusieurs horodatages par ligne).
pub fn parse_lrc(lrc: &str) -> Vec<(f64, String)> {
    let mut out = vec![];
    for line in lrc.lines() {
        let mut rest = line.trim();
        let mut times = vec![];
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let tag = &stripped[..end];
            if let Some((m, s)) = tag.split_once(':') {
                if let (Ok(m), Ok(s)) = (m.parse::<f64>(), s.parse::<f64>()) {
                    times.push(m * 60.0 + s);
                }
            }
            rest = stripped[end + 1..].trim_start();
        }
        for t in times {
            out.push((t, rest.to_string()));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// Index de la ligne en cours pour une position donnée.
pub fn current_line(lines: &[(f64, String)], time: f64) -> Option<usize> {
    // Petite avance : la ligne s'allume un peu avant d'être chantée.
    let t = time + 0.25;
    match lines.iter().rposition(|(start, _)| *start <= t) {
        Some(i) => Some(i),
        None => None,
    }
}
