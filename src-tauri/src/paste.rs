//! Puts the transcript where the user is typing: through the clipboard and a
//! simulated Cmd+V / Ctrl+V, restoring what the clipboard held before.
//!
//! The layout-aware Cmd+V lookup is adapted from Handy
//! (https://github.com/cjpais/Handy, MIT, Copyright (c) 2025 CJ Pais).

use enigo::{Direction, Enigo, Key, Keyboard};
use serde::Serialize;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// One clipboard for the app's lifetime: on Linux the clipboard's content
/// lives only as long as the process owning it keeps serving it.
pub struct ClipboardState(Mutex<Option<arboard::Clipboard>>);

impl ClipboardState {
    pub fn new() -> Self {
        let clipboard = arboard::Clipboard::new()
            .inspect_err(|e| log::warn!("Clipboard unavailable: {e}"))
            .ok();
        Self(Mutex::new(clipboard))
    }

    fn with<T>(
        &self,
        f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
    ) -> Result<T, PasteError> {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let clipboard = guard.as_mut().ok_or(PasteError::NoClipboard)?;
        f(clipboard).map_err(|e| PasteError::Failed {
            detail: e.to_string(),
        })
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PasteError {
    NoClipboard,
    /// macOS: Parler may not send keystrokes until the user allows it in
    /// Privacy & Security > Accessibility. The transcript is left on the
    /// clipboard.
    NeedsAccessibility,
    Failed {
        detail: String,
    },
}

pub fn copy(app: &AppHandle, text: &str) -> Result<(), PasteError> {
    let text = text.to_string();
    app.state::<ClipboardState>()
        .with(move |clipboard| clipboard.set_text(text))
}

/// Blocks for a few hundred milliseconds; never call it on the main thread,
/// which it needs free to send the keystrokes.
pub fn paste(app: &AppHandle, text: &str) -> Result<(), PasteError> {
    let clipboard = app.state::<ClipboardState>();
    let saved = clipboard.with(|c| c.get_text()).ok();
    copy(app, text)?;
    // Give the clipboard a moment to settle before the target reads it.
    thread::sleep(Duration::from_millis(60));

    // On failure the transcript stays on the clipboard so the user can paste
    // it by hand; that beats restoring and losing it.
    send_paste_keys(app)?;

    // Let the target app read the clipboard before restoring it.
    thread::sleep(Duration::from_millis(350));
    let _ = match saved {
        Some(previous) => clipboard.with(move |c| c.set_text(previous)),
        None => clipboard.with(|c| c.clear()),
    };
    Ok(())
}

fn send_paste_keys(app: &AppHandle) -> Result<(), PasteError> {
    #[cfg(target_os = "linux")]
    if linux::send_with_native_tool() {
        return Ok(());
    }

    // Enigo and the macOS keyboard-layout APIs want the main thread.
    let (tx, rx) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = tx.send(press_paste_chord());
    })
    .map_err(|e| PasteError::Failed {
        detail: e.to_string(),
    })?;
    rx.recv().map_err(|_| PasteError::Failed {
        detail: "the main thread dropped the paste".to_string(),
    })?
}

fn press_paste_chord() -> Result<(), PasteError> {
    let mut enigo = Enigo::new(&enigo::Settings::default()).map_err(|e| {
        if cfg!(target_os = "macos") {
            log::warn!("Cannot send keystrokes: {e}");
            PasteError::NeedsAccessibility
        } else {
            PasteError::Failed {
                detail: e.to_string(),
            }
        }
    })?;

    #[cfg(target_os = "macos")]
    let (modifier, v) = (Key::Meta, macos::command_v_key());
    #[cfg(target_os = "windows")]
    let (modifier, v) = (Key::Control, Key::Other(0x56)); // VK_V
    #[cfg(target_os = "linux")]
    let (modifier, v) = (Key::Control, Key::Unicode('v'));

    let failed = |e: enigo::InputError| PasteError::Failed {
        detail: e.to_string(),
    };
    enigo.key(modifier, Direction::Press).map_err(failed)?;
    let clicked = enigo.key(v, Direction::Click).map_err(failed);
    // Some apps read the modifier from global state when handling the V,
    // so it stays down a moment; it is released even if the click failed.
    thread::sleep(Duration::from_millis(60));
    enigo.key(modifier, Direction::Release).map_err(failed)?;
    clicked
}

#[cfg(target_os = "linux")]
mod linux {
    use std::process::{Command, Stdio};

    fn run(program: &str, args: &[&str]) -> bool {
        Command::new(program)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// Wayland ignores simulated X11 input for native windows; use a
    /// compositor-level tool when one is installed.
    pub fn send_with_native_tool() -> bool {
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return false;
        }
        run("wtype", &["-M", "ctrl", "v", "-m", "ctrl"])
            || run("ydotool", &["key", "29:1", "47:1", "47:0", "29:0"])
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::Key;
    use std::ffi::c_void;

    type TisInputSourceRef = *const c_void;
    type CfDataRef = *const c_void;
    type CfStringRef = *const c_void;

    /// kVK_ANSI_V, the fallback when the active layout can't be read.
    const ANSI_V_KEYCODE: u16 = 9;
    const KEYCODE_COUNT: u16 = 128;
    const UC_KEY_ACTION_DISPLAY: u16 = 3;
    const UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK: u32 = 1;
    // Carbon's cmdKey (bit 8), shifted right by 8 as UCKeyTranslate expects.
    const COMMAND_MODIFIER_STATE: u32 = 1;

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn TISCopyCurrentKeyboardLayoutInputSource() -> TisInputSourceRef;
        fn TISGetInputSourceProperty(
            input_source: TisInputSourceRef,
            property_key: CfStringRef,
        ) -> CfDataRef;
        static kTISPropertyUnicodeKeyLayoutData: CfStringRef;
        fn UCKeyTranslate(
            key_layout: *const u8,
            virtual_key_code: u16,
            key_action: u16,
            modifier_key_state: u32,
            keyboard_type: u32,
            key_translate_options: u32,
            dead_key_state: *mut u32,
            max_string_length: usize,
            actual_string_length: *mut usize,
            unicode_string: *mut u16,
        ) -> i32;
        fn LMGetKbdType() -> u8;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDataGetBytePtr(data: CfDataRef) -> *const u8;
        fn CFRelease(value: *const c_void);
    }

    struct InputSource(TisInputSourceRef);

    impl Drop for InputSource {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: balances the retain from
                // TISCopyCurrentKeyboardLayoutInputSource.
                unsafe { CFRelease(self.0) };
            }
        }
    }

    /// The physical key macOS reads as `v` while Command is held. Asking with
    /// Command matters: non-Latin layouts map Cmd shortcuts to their ANSI
    /// keys, while Dvorak does not. Must run on the main thread.
    fn command_v_keycode() -> Option<u16> {
        // SAFETY: called on the main thread; released by InputSource::drop.
        let source = InputSource(unsafe { TISCopyCurrentKeyboardLayoutInputSource() });
        if source.0.is_null() {
            return None;
        }
        // SAFETY: the source stays retained for the whole scan.
        let data = unsafe { TISGetInputSourceProperty(source.0, kTISPropertyUnicodeKeyLayoutData) };
        if data.is_null() {
            return None;
        }
        // SAFETY: data is owned by the retained source.
        let layout = unsafe { CFDataGetBytePtr(data) };
        if layout.is_null() {
            return None;
        }
        // SAFETY: no arguments; returns the current keyboard type.
        let keyboard_type = unsafe { LMGetKbdType() } as u32;

        (0..KEYCODE_COUNT).find(|&keycode| {
            let mut dead_key_state = 0;
            let mut chars = [0_u16; 4];
            let mut length = 0_usize;
            // SAFETY: layout is valid while source is retained; every output
            // pointer is local storage of the declared size.
            let status = unsafe {
                UCKeyTranslate(
                    layout,
                    keycode,
                    UC_KEY_ACTION_DISPLAY,
                    COMMAND_MODIFIER_STATE,
                    keyboard_type,
                    UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK,
                    &mut dead_key_state,
                    chars.len(),
                    &mut length,
                    chars.as_mut_ptr(),
                )
            };
            status == 0 && length == 1 && chars[0] == u16::from(b'v')
        })
    }

    pub(super) fn command_v_key() -> Key {
        let keycode = command_v_keycode().unwrap_or_else(|| {
            log::warn!("Could not read the keyboard layout; using the ANSI V key");
            ANSI_V_KEYCODE
        });
        Key::Other(u32::from(keycode))
    }
}
