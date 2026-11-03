//! Contrôles multimédia de Windows (SMTC) : Mixtape publie le titre, l'artiste, la
//! pochette, l'état et la progression, visibles par le panneau média de Windows, l'écran
//! de verrouillage et les applications qui lisent les sessions (GSMTC). Les boutons
//! (clavier multimédia, casque, autres applications) reviennent ici.
//!
//! mpv publie sa propre session sans pochette : elle est désactivée (`--media-controls=no`).

use std::path::Path;
use std::sync::mpsc::Sender;

use windows::core::{Interface, HSTRING};
use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls, SystemMediaTransportControlsButton,
    SystemMediaTransportControlsButtonPressedEventArgs, SystemMediaTransportControlsTimelineProperties,
};
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::Storage::StorageFile;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;

use crate::hotkeys::MediaKey;

pub struct Smtc {
    controls: SystemMediaTransportControls,
    status: Option<MediaPlaybackStatus>,
}

fn ticks(seconds: f64) -> TimeSpan {
    TimeSpan { Duration: (seconds.max(0.0) * 10_000_000.0) as i64 }
}

impl Smtc {
    /// Rattache une session SMTC à la fenêtre `hwnd`.
    pub fn new(hwnd: isize, tx: Sender<MediaKey>, wake: impl Fn() + Send + Sync + 'static) -> windows::core::Result<Self> {
        let interop = windows::core::factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
        let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(HWND(hwnd as *mut _))? };
        controls.SetIsEnabled(true)?;
        controls.SetIsPlayEnabled(true)?;
        controls.SetIsPauseEnabled(true)?;
        controls.SetIsNextEnabled(true)?;
        controls.SetIsPreviousEnabled(true)?;
        controls.SetIsStopEnabled(true)?;
        controls.SetPlaybackStatus(MediaPlaybackStatus::Closed)?;
        controls.ButtonPressed(&TypedEventHandler::<SystemMediaTransportControls, SystemMediaTransportControlsButtonPressedEventArgs>::new(
            move |_, args| {
                if let Some(args) = args.as_ref() {
                    let key = match args.Button()? {
                        SystemMediaTransportControlsButton::Play | SystemMediaTransportControlsButton::Pause => Some(MediaKey::PlayPause),
                        SystemMediaTransportControlsButton::Next => Some(MediaKey::Next),
                        SystemMediaTransportControlsButton::Previous => Some(MediaKey::Prev),
                        SystemMediaTransportControlsButton::Stop => Some(MediaKey::Stop),
                        _ => None,
                    };
                    if let Some(k) = key {
                        let _ = tx.send(k);
                        wake();
                    }
                }
                Ok(())
            },
        ))?;
        Ok(Self { controls, status: None })
    }

    /// Titre, artiste et pochette (fichier local carré si disponible, sinon URL).
    pub fn set_track(&self, title: &str, artist: &str, cover_file: Option<&Path>, cover_url: &str) {
        let _ = (|| -> windows::core::Result<()> {
            let up = self.controls.DisplayUpdater()?;
            up.ClearAll()?;
            up.SetType(MediaPlaybackType::Music)?;
            let music = up.MusicProperties()?;
            music.SetTitle(&HSTRING::from(title))?;
            music.SetArtist(&HSTRING::from(artist))?;
            let thumb = match cover_file {
                Some(p) => {
                    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(p.as_os_str()))?.join()?;
                    RandomAccessStreamReference::CreateFromFile(&file)?
                }
                None => RandomAccessStreamReference::CreateFromUri(&Uri::CreateUri(&HSTRING::from(cover_url))?)?,
            };
            up.SetThumbnail(&thumb)?;
            up.Update()
        })();
    }

    pub fn clear(&mut self) {
        let _ = self.controls.DisplayUpdater().and_then(|u| u.ClearAll().and_then(|_| u.Update()));
        self.set_status(None);
    }

    /// `Some(true)` = lecture, `Some(false)` = pause, `None` = rien en lecture.
    pub fn set_status(&mut self, playing: Option<bool>) {
        let st = match playing {
            Some(true) => MediaPlaybackStatus::Playing,
            Some(false) => MediaPlaybackStatus::Paused,
            None => MediaPlaybackStatus::Closed,
        };
        if self.status != Some(st) {
            self.status = Some(st);
            let _ = self.controls.SetPlaybackStatus(st);
        }
    }

    pub fn set_timeline(&self, position: f64, duration: f64) {
        let _ = (|| -> windows::core::Result<()> {
            let tl = SystemMediaTransportControlsTimelineProperties::new()?;
            tl.SetStartTime(ticks(0.0))?;
            tl.SetMinSeekTime(ticks(0.0))?;
            tl.SetEndTime(ticks(duration))?;
            tl.SetMaxSeekTime(ticks(duration))?;
            tl.SetPosition(ticks(position.min(duration)))?;
            self.controls.UpdateTimelineProperties(&tl)
        })();
    }
}

/// Vérifie à la compilation que l'interface est bien utilisable (référence à `Interface`).
#[allow(dead_code)]
fn _assert_interface() -> windows::core::GUID {
    SystemMediaTransportControls::IID
}

/// Diagnostic : `mixtape.exe --gsmtc <fichier>` liste les sessions média de Windows
/// telles que les voient les autres applications (titre, artiste, taille de la pochette).
pub fn dump_sessions(out: &Path) {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as Mgr;
    use windows::Storage::Streams::{Buffer, InputStreamOptions};
    let mut lines = vec![];
    let res = (|| -> windows::core::Result<()> {
        let mgr = Mgr::RequestAsync()?.join()?;
        for s in mgr.GetSessions()? {
            let app = s.SourceAppUserModelId()?;
            let props = s.TryGetMediaPropertiesAsync()?.join()?;
            let status = s.GetPlaybackInfo()?.PlaybackStatus()?;
            let thumb = match props.Thumbnail() {
                Ok(t) => match t.OpenReadAsync().and_then(|op| op.join()) {
                    Ok(st) => {
                        let size = st.Size().unwrap_or(0);
                        let kind = st.ContentType().map(|c| c.to_string()).unwrap_or_default();
                        let buf = Buffer::Create(size.min(16) as u32)?;
                        let _ = st.ReadAsync(&buf, buf.Capacity()?, InputStreamOptions::None).and_then(|op| op.join());
                        format!("{size} octets {kind}")
                    }
                    Err(e) => format!("illisible ({e})"),
                },
                Err(_) => "aucune".into(),
            };
            lines.push(format!("{app} | {} | {} | {:?} | pochette : {thumb}", props.Title()?, props.Artist()?, status.0));
        }
        Ok(())
    })();
    if let Err(e) = res {
        lines.push(format!("erreur : {e}"));
    }
    let _ = std::fs::write(out, lines.join("\n"));
}
