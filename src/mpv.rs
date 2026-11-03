//! Pilotage de mpv par son IPC JSON (tube nommé Windows).
//!
//! mpv tourne en arrière-plan, sans fenêtre, en mode `--idle`. Il résout lui-même les URL
//! YouTube via son script intégré `ytdl_hook`, qui appelle notre yt-dlp.exe.
//! Un fil dédié (runtime tokio mono-thread) lit les événements et écrit les commandes :
//! l'interface n'attend jamais mpv.

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::sync::mpsc;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const OBSERVED: &[&str] = &[
    "pause",
    "time-pos",
    "duration",
    "playlist-pos",
    "volume",
    "paused-for-cache",
    "path",
    "audio-device-list",
];

/// État de lecture tel que rapporté par mpv.
#[derive(Clone, Debug)]
pub struct Props {
    pub pause: bool,
    pub time: Option<f64>,
    pub duration: Option<f64>,
    pub pos: i64,
    pub volume: f64,
    pub buffering: bool,
    pub path: Option<String>,
    /// Sorties audio disponibles : (identifiant mpv, nom lisible).
    pub devices: Vec<(String, String)>,
    /// Dernière erreur (lecture impossible, mpv arrêté…), à afficher puis effacer.
    pub error: Option<String>,
}

impl Default for Props {
    fn default() -> Self {
        Self { pause: false, time: None, duration: None, pos: -1, volume: 70.0, buffering: false, path: None, devices: vec![], error: None }
    }
}

pub struct Mpv {
    tx: mpsc::UnboundedSender<Value>,
    pub props: Arc<Mutex<Props>>,
    child: Child,
}

/// Place mpv dans un Job Windows « kill on close » : si Mixtape est tué ou plante,
/// Windows ferme le handle du Job et mpv meurt avec lui (pas de processus orphelin).
fn tie_to_this_process(child: &Child) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::*;
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        AssignProcessToJobObject(job, child.as_raw_handle() as _);
        // Handle du Job volontairement jamais fermé : il l'est à la fin de notre processus.
    }
}

impl Mpv {
    pub fn start(bin: &Path, repaint: impl Fn() + Send + 'static) -> Result<Self, String> {
        let pipe = format!(r"\\.\pipe\mixtape-mpv-{}", std::process::id());
        let child = Command::new(bin.join("mpv.exe"))
            .args([
                "--no-config",
                "--idle=yes",
                "--no-video",
                "--audio-display=no",
                "--force-window=no",
                "--no-terminal",
                // Session média Windows publiée par Mixtape (avec pochette), pas par mpv.
                "--media-controls=no",
                "--volume=70",
                "--prefetch-playlist=yes",
                "--ytdl-format=bestaudio/best",
                // Cache réduit : 150 Mio par défaut, inutile pour de l'audio.
                "--demuxer-max-bytes=16MiB",
                "--demuxer-max-back-bytes=4MiB",
            ])
            .arg(format!("--input-ipc-server={pipe}"))
            .arg(format!("--script-opts=ytdl_hook-ytdl_path={}", bin.join("yt-dlp.exe").display()))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("lancement de mpv impossible : {e}"))?;
        tie_to_this_process(&child);

        let (tx, rx) = mpsc::unbounded_channel::<Value>();
        let props = Arc::new(Mutex::new(Props::default()));
        let shared = props.clone();
        std::thread::Builder::new()
            .name("mpv-ipc".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build();
                let Ok(rt) = rt else { return };
                if let Err(e) = rt.block_on(ipc_loop(pipe, rx, shared.clone(), &repaint)) {
                    shared.lock().unwrap().error = Some(e);
                }
                repaint();
            })
            .map_err(|e| e.to_string())?;

        Ok(Self { tx, props, child })
    }

    /// Envoie une commande mpv (mise en file ; partira dès que l'IPC est connecté).
    pub fn cmd(&self, args: Value) {
        let _ = self.tx.send(args);
    }

    pub fn props(&self) -> Props {
        self.props.lock().unwrap().clone()
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

async fn ipc_loop(
    pipe: String,
    mut rx: mpsc::UnboundedReceiver<Value>,
    props: Arc<Mutex<Props>>,
    repaint: &(impl Fn() + Send),
) -> Result<(), String> {
    // Le tube n'existe qu'une fois mpv initialisé : on réessaie pendant ~5 s.
    let mut client = None;
    for _ in 0..50 {
        match ClientOptions::new().open(&pipe) {
            Ok(c) => {
                client = Some(c);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    let client = client.ok_or("connexion à mpv impossible (tube IPC absent)")?;
    let (reader, mut writer) = tokio::io::split(client);
    let mut lines = BufReader::new(reader).lines();

    for (i, name) in OBSERVED.iter().enumerate() {
        write(&mut writer, json!(["observe_property", i + 1, name])).await?;
    }

    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    if handle_message(&line, &props) {
                        repaint();
                    }
                }
                _ => return Err("mpv s'est arrêté".into()),
            },
            cmd = rx.recv() => match cmd {
                Some(cmd) => write(&mut writer, cmd).await?,
                None => return Ok(()),
            },
        }
    }
}

async fn write(w: &mut (impl AsyncWriteExt + Unpin), command: Value) -> Result<(), String> {
    let mut line = json!({ "command": command }).to_string();
    line.push('\n');
    w.write_all(line.as_bytes()).await.map_err(|e| e.to_string())
}

/// Met à jour l'état ; renvoie `true` si l'interface doit être redessinée.
fn handle_message(line: &str, props: &Mutex<Props>) -> bool {
    let Ok(msg) = serde_json::from_str::<Value>(line) else { return false };
    let mut p = props.lock().unwrap();
    match msg["event"].as_str() {
        Some("property-change") => {
            let data = &msg["data"];
            match msg["name"].as_str().unwrap_or_default() {
                "pause" => p.pause = data.as_bool().unwrap_or(false),
                "duration" => p.duration = data.as_f64(),
                "paused-for-cache" => p.buffering = data.as_bool().unwrap_or(false),
                "volume" => p.volume = data.as_f64().unwrap_or(p.volume),
                "playlist-pos" => p.pos = data.as_i64().unwrap_or(-1),
                // URL du fichier réellement chargé (comparée à la file côté interface).
                "path" => p.path = data.as_str().map(str::to_string),
                "audio-device-list" => {
                    p.devices = data
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|d| Some((d["name"].as_str()?.to_string(), d["description"].as_str().unwrap_or("").to_string())))
                                .collect()
                        })
                        .unwrap_or_default();
                }
                "time-pos" => {
                    let new = data.as_f64();
                    // Redessiner seulement quand la seconde affichée change.
                    let changed = new.map(f64::floor) != p.time.map(f64::floor);
                    p.time = new;
                    return changed;
                }
                _ => return false,
            }
            true
        }
        Some("end-file") if msg["reason"] == "error" => {
            let why = msg["file_error"].as_str().unwrap_or("erreur inconnue");
            p.error = Some(format!(
                "Lecture impossible ({why}). Si YouTube bloque (« not a bot »), active les cookies dans Paramètres."
            ));
            true
        }
        _ => false,
    }
}
