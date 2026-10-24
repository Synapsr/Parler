//! The global dictation shortcut, plus Escape to cancel while dictating.
//!
//! A shortcut is an accelerator the global-shortcut plugin understands
//! ("Alt+Space"), or `FN`: the fn (🌐) key alone, which only an event tap
//! can follow (macOS only, see `fn_key`).

use crate::dictation::{Dictation, Event, Problem, StatusState};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const CANCEL: &str = "Escape";
/// The fn (🌐) key alone.
pub const FN: &str = "Fn";

/// Registers `accelerator` (e.g. "Alt+Space", or `FN`) as the dictation
/// shortcut.
pub fn register(app: &AppHandle, accelerator: &str) -> Result<(), String> {
    if accelerator == FN {
        return register_fn(app);
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

/// fn can't fail to register: without the Accessibility permission the
/// window says so, and the key works as soon as it is granted.
#[cfg(target_os = "macos")]
fn register_fn(app: &AppHandle) -> Result<(), String> {
    crate::fn_key::start(app);
    Ok(())
}

/// Elsewhere the keyboard keeps fn to itself.
#[cfg(not(target_os = "macos"))]
fn register_fn(_app: &AppHandle) -> Result<(), String> {
    Err("the fn key can only be the shortcut on macOS".to_string())
}

pub fn unregister(app: &AppHandle, accelerator: &str) {
    if accelerator == FN {
        #[cfg(target_os = "macos")]
        crate::fn_key::stop();
        return;
    }
    if let Err(e) = app.global_shortcut().unregister(accelerator) {
        log::debug!("Could not unregister {accelerator}: {e}");
    }
}

pub fn is_registered(app: &AppHandle, accelerator: &str) -> bool {
    if accelerator == FN {
        return cfg!(target_os = "macos") && fn_running();
    }
    app.global_shortcut().is_registered(accelerator)
}

#[cfg(target_os = "macos")]
fn fn_running() -> bool {
    crate::fn_key::is_running()
}

#[cfg(not(target_os = "macos"))]
fn fn_running() -> bool {
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

/// Shows that the dictation shortcut doesn't work, until it does.
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
