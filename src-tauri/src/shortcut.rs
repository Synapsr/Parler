//! The global dictation shortcut, plus Escape to cancel while dictating.

use crate::dictation::{Dictation, Event};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const CANCEL: &str = "Escape";

/// Registers `accelerator` (e.g. "Alt+Space") as the dictation shortcut.
pub fn register(app: &AppHandle, accelerator: &str) -> Result<(), String> {
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

pub fn unregister(app: &AppHandle, accelerator: &str) {
    if let Err(e) = app.global_shortcut().unregister(accelerator) {
        log::debug!("Could not unregister {accelerator}: {e}");
    }
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
