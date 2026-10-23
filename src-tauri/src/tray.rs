//! The menu bar / system tray icon.

use crate::dictation::{Dictation, Event};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const ID: &str = "parler";
const IDLE_ICON: &[u8] = include_bytes!("../icons/tray.png");
const RECORDING_ICON: &[u8] = include_bytes!("../icons/tray-recording.png");

struct Labels {
    open: &'static str,
    toggle: &'static str,
    quit: &'static str,
}

fn labels(locale: &str) -> Labels {
    if locale.starts_with("fr") {
        Labels {
            open: "Ouvrir Parler",
            toggle: "Démarrer ou arrêter la dictée",
            quit: "Quitter Parler",
        }
    } else {
        Labels {
            open: "Open Parler",
            toggle: "Start or stop dictation",
            quit: "Quit Parler",
        }
    }
}

fn menu(app: &AppHandle, locale: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let labels = labels(locale);
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", labels.open, true, None::<&str>)?,
            &MenuItem::with_id(app, "toggle", labels.toggle, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?,
        ],
    )
}

pub fn create(app: &AppHandle) -> tauri::Result<TrayIcon> {
    TrayIconBuilder::with_id(ID)
        .icon(Image::from_bytes(IDLE_ICON)?)
        .icon_as_template(true)
        .tooltip("Parler")
        .menu(&menu(app, "en")?)
        // Elsewhere a left click opens the window; on macOS it opens the
        // menu, as menu bar items do.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => crate::show_main_window(app),
            "toggle" => app.state::<Dictation>().send(Event::Toggle),
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

/// Rebuilds the menu in the interface language.
pub fn set_locale(app: &AppHandle, locale: &str) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(ID), menu(app, locale)) {
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
