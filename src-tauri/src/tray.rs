//! The menu bar / system tray icon.

use crate::dictation::{Dictation, Event};
use crate::history::History;
use crate::paste;
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

const ID: &str = "parler";
const IDLE_ICON: &[u8] = include_bytes!("../icons/tray.png");
const RECORDING_ICON: &[u8] = include_bytes!("../icons/tray-recording.png");

/// The menu's words, sent by the window in the interface language (see
/// `set_labels`); English until it loads.
#[derive(serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Labels {
    open: String,
    toggle: String,
    copy_last: String,
    history: String,
    quit: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            open: "Open Parler".into(),
            toggle: "Start or stop dictation".into(),
            copy_last: "Copy last transcript".into(),
            history: "History…".into(),
            quit: "Quit Parler".into(),
        }
    }
}

static LABELS: Mutex<Option<Labels>> = Mutex::new(None);

fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let labels = LABELS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default();
    // Nothing to copy before the first dictation.
    let has_history = app
        .try_state::<History>()
        .is_some_and(|history| !history.entries().is_empty());
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", &labels.open, true, None::<&str>)?,
            &MenuItem::with_id(app, "toggle", &labels.toggle, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                "copy_last",
                &labels.copy_last,
                has_history,
                None::<&str>,
            )?,
            &MenuItem::with_id(app, "history", &labels.history, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", &labels.quit, true, None::<&str>)?,
        ],
    )
}

fn copy_last(app: &AppHandle) {
    let last = app
        .state::<History>()
        .entries()
        .into_iter()
        .next()
        .map(|entry| entry.text);
    if let Some(text) = last {
        if let Err(e) = paste::copy(app, &text) {
            log::warn!("Could not copy the last transcript: {e:?}");
        }
    }
}

/// Opens the window on its History page.
fn show_history(app: &AppHandle) {
    crate::show_main_window(app);
    let _ = app.emit_to("main", "parler://navigate", "history");
}

pub fn create(app: &AppHandle) -> tauri::Result<TrayIcon> {
    TrayIconBuilder::with_id(ID)
        .icon(Image::from_bytes(IDLE_ICON)?)
        .icon_as_template(true)
        .tooltip("Parler")
        .menu(&menu(app)?)
        // Elsewhere a left click opens the window; on macOS it opens the
        // menu, as menu bar items do.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => crate::show_main_window(app),
            "toggle" => app.state::<Dictation>().send(Event::Toggle),
            "copy_last" => copy_last(app),
            "history" => show_history(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if !cfg!(target_os = "macos") {
                    crate::show_main_window(tray.app_handle());
                }
            }
        })
        .build(app)
}

/// Rebuilds the menu with words in the interface language.
pub fn set_labels(app: &AppHandle, labels: Labels) {
    *LABELS.lock().unwrap_or_else(|e| e.into_inner()) = Some(labels);
    refresh(app);
}

/// Rebuilds the menu, e.g. once the history gains or loses its entries.
pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(ID), menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

pub fn set_recording(app: &AppHandle, recording: bool) {
    let Some(tray) = app.tray_by_id(ID) else {
        return;
    };
    let icon = if recording { RECORDING_ICON } else { IDLE_ICON };
    if let Ok(image) = Image::from_bytes(icon) {
        let _ = tray.set_icon(Some(image));
        // The recording icon keeps its red dot; template icons are drawn in
        // the menu bar's own colour.
        let _ = tray.set_icon_as_template(!recording);
    }
}
