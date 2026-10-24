//! The global dictation shortcut, plus Escape to cancel while dictating.
//!
//! A shortcut is an accelerator the global-shortcut plugin understands
//! ("Alt+Space"), or a `Key` used alone (right ⌘, right ⌥, right ⌃, fn),
//! which only an event tap can follow (macOS only, see `modifier_key`).

use crate::dictation::{Dictation, Event, Problem, StatusState};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const CANCEL: &str = "Escape";

/// A key used alone as the shortcut, stored under a name the accelerator
/// syntax never produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Fn,
    RightCommand,
    RightOption,
    RightControl,
}

impl Key {
    const ALL: [Key; 4] = [
        Key::Fn,
        Key::RightCommand,
        Key::RightOption,
        Key::RightControl,
    ];

    /// The key a shortcut setting names, if it names one.
    pub fn from_setting(setting: &str) -> Option<Key> {
        Self::ALL.into_iter().find(|key| key.setting() == setting)
    }

    pub fn setting(self) -> &'static str {
        match self {
            Key::Fn => "Fn",
            Key::RightCommand => "RightCommand",
            Key::RightOption => "RightOption",
            Key::RightControl => "RightControl",
        }
    }
}

/// Registers `accelerator` (e.g. "Alt+Space", or a `Key`'s name) as the
/// dictation shortcut.
pub fn register(app: &AppHandle, accelerator: &str) -> Result<(), String> {
    if let Some(key) = Key::from_setting(accelerator) {
        return register_key(app, key);
    }
    app.global_shortcut()
        .on_shortcut(accelerator, |app, _, event| {
            let dictation = app.state::<Dictation>();
            dictation.send(match event.state {
                ShortcutState::Pressed => Event::Press,
                ShortcutState::Released => Event::Release,
            });
        })
        .map_err(|e| e.to_string())
}

/// A key alone can't fail to register: without the Accessibility permission
/// the window says so, and the key works as soon as it is granted.
#[cfg(target_os = "macos")]
fn register_key(app: &AppHandle, key: Key) -> Result<(), String> {
    crate::modifier_key::start(app, key);
    Ok(())
}

/// Elsewhere a modifier can't be followed alone, and fn never reaches the
/// system.
#[cfg(not(target_os = "macos"))]
fn register_key(_app: &AppHandle, key: Key) -> Result<(), String> {
    Err(format!(
        "{} alone can only be the shortcut on macOS",
        key.setting()
    ))
}

pub fn unregister(app: &AppHandle, accelerator: &str) {
    if Key::from_setting(accelerator).is_some() {
        #[cfg(target_os = "macos")]
        crate::modifier_key::stop();
        return;
    }
    if let Err(e) = app.global_shortcut().unregister(accelerator) {
        log::debug!("Could not unregister {accelerator}: {e}");
    }
}

pub fn is_registered(app: &AppHandle, accelerator: &str) -> bool {
    match Key::from_setting(accelerator) {
        Some(key) => key_running(key),
        None => app.global_shortcut().is_registered(accelerator),
    }
}

#[cfg(target_os = "macos")]
fn key_running(key: Key) -> bool {
    crate::modifier_key::is_running(key)
}

#[cfg(not(target_os = "macos"))]
fn key_running(_key: Key) -> bool {
    false
}

/// Swaps the dictation shortcut. If the new one can't be registered (taken by
/// another app, or invalid), the old one is put back.
pub fn replace(app: &AppHandle, old: &str, new: &str) -> Result<(), String> {
    if old == new {
        return Ok(());
    }
    if new.eq_ignore_ascii_case(CANCEL) {
        return Err("Escape is reserved for cancelling".to_string());
    }
    unregister(app, old);
    register(app, new).inspect_err(|_| {
        let _ = register(app, old);
    })
}

/// Shows that the dictation shortcut doesn't work, until it does. Only the
/// macOS single-key listener needs it so far.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn report_problem(app: &AppHandle, shortcut: &str, detail: String) {
    let state = app.state::<StatusState>();
    let mut status = state.0.lock().unwrap_or_else(|e| e.into_inner());
    status.problem = Some(Problem::Shortcut {
        shortcut: shortcut.to_string(),
        detail,
    });
    let _ = app.emit_to("main", "parler://status", status.clone());
}

/// The shortcut works now: its problem is out of date.
pub fn clear_problem(app: &AppHandle) {
    let state = app.state::<StatusState>();
    let mut status = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if matches!(status.problem, Some(Problem::Shortcut { .. })) {
        status.problem = None;
        let _ = app.emit_to("main", "parler://status", status.clone());
    }
}

/// Escape cancels only while a dictation is under way; the rest of the time
/// it belongs to the other apps.
pub fn set_cancel_active(app: &AppHandle, active: bool) {
    let shortcuts = app.global_shortcut();
    if active {
        if shortcuts.is_registered(CANCEL) {
            return;
        }
        let registered = shortcuts.on_shortcut(CANCEL, |app, _, event| {
            if event.state == ShortcutState::Pressed {
                app.state::<Dictation>().send(Event::Cancel);
            }
        });
        if let Err(e) = registered {
            log::warn!("Escape could not be registered to cancel: {e}");
        }
    } else {
        unregister(app, CANCEL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_alone_have_reserved_names() {
        for key in Key::ALL {
            assert_eq!(Key::from_setting(key.setting()), Some(key));
        }
        assert_eq!(Key::from_setting("Fn"), Some(Key::Fn));
        assert_eq!(Key::from_setting("Alt+Space"), None);
        assert_eq!(Key::from_setting("fn"), None);
    }
}
