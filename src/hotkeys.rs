//! Touches multimédia du clavier (lecture/pause, suivant, précédent), actives même
//! quand Mixtape n'a pas le focus. Si une autre application les a déjà réservées,
//! l'enregistrement échoue sans conséquence.

use std::sync::mpsc::Sender;

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, MOD_NOREPEAT, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK, VK_MEDIA_STOP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

#[derive(Clone, Copy, Debug)]
pub enum MediaKey {
    PlayPause,
    Next,
    Prev,
    Stop,
}

const KEYS: [(i32, u16, MediaKey); 4] = [
    (1, VK_MEDIA_PLAY_PAUSE, MediaKey::PlayPause),
    (2, VK_MEDIA_NEXT_TRACK, MediaKey::Next),
    (3, VK_MEDIA_PREV_TRACK, MediaKey::Prev),
    (4, VK_MEDIA_STOP, MediaKey::Stop),
];

pub fn spawn(tx: Sender<MediaKey>, wake: impl Fn() + Send + 'static) {
    let _ = std::thread::Builder::new().name("media-keys".into()).spawn(move || unsafe {
        // Les raccourcis sont liés à ce fil : il doit aussi pomper les messages.
        let mut registered = 0;
        for (id, vk, _) in KEYS {
            if RegisterHotKey(std::ptr::null_mut(), id, MOD_NOREPEAT, vk as u32) != 0 {
                registered += 1;
            }
        }
        if registered == 0 {
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                if let Some((_, _, key)) = KEYS.iter().find(|(id, _, _)| *id as usize == msg.wParam) {
                    if tx.send(*key).is_err() {
                        return;
                    }
                    wake();
                }
            }
        }
    });
}
