//! The small pill shown at the bottom of the screen while dictating. It must
//! never take focus: the transcript is pasted into whatever app has it.
//!
//! The per-platform window setup follows Handy
//! (https://github.com/cjpais/Handy, MIT, Copyright (c) 2025 CJ Pais).

use crate::dictation::Problem;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl};

pub const LABEL: &str = "overlay";
const WIDTH: f64 = 240.0;
const HEIGHT: f64 = 72.0;
/// Logical points between the pill and the bottom of the work area.
const BOTTOM_MARGIN: f64 = 40.0;

/// Bumped by every show, so a delayed hide from an earlier dictation never
/// hides a newer one.
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Shortcut pressed, microphone still opening.
    Connecting,
    Recording,
    Transcribing,
    /// Nothing was heard.
    Empty,
    Error,
    Hidden,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OverlayEvent {
    phase: Phase,
    sounds: bool,
    /// What went wrong, for the error phase.
    problem: Option<Problem>,
}

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true
        }
    })
}

#[cfg(target_os = "macos")]
pub fn create(app: &AppHandle) {
    use tauri_nspanel::{CollectionBehavior, PanelBuilder, PanelLevel, StyleMask};

    match PanelBuilder::<_, OverlayPanel>::new(app, LABEL)
        .url(WebviewUrl::App("overlay.html".into()))
        .title("Parler")
        .level(PanelLevel::Status)
        .size(tauri::Size::Logical(tauri::LogicalSize {
            width: WIDTH,
            height: HEIGHT,
        }))
        .has_shadow(false)
        .transparent(true)
        .no_activate(true)
        .corner_radius(0.0)
        .style_mask(StyleMask::empty().borderless().nonactivating_panel())
        .with_window(|w| w.decorations(false).transparent(true).focusable(false))
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary(),
        )
        .build()
    {
        Ok(panel) => panel.hide(),
        Err(e) => log::error!("Could not create the overlay: {e}"),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn create(app: &AppHandle) {
    let built =
        tauri::WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay.html".into()))
            .title("Parler")
            .inner_size(WIDTH, HEIGHT)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .closable(false)
            .decorations(false)
            .shadow(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .focusable(false)
            .focused(false)
            .visible(false)
            .build();
    if let Err(e) = built {
        log::error!("Could not create the overlay: {e}");
    }
}

/// Shows the pill in `phase`.
pub fn show(app: &AppHandle, phase: Phase, sounds: bool) {
    show_with(app, phase, sounds, None);
}

/// Window geometry is only safe on the main thread on every platform (GTK
/// and AppKit both insist).
fn show_with(app: &AppHandle, phase: Phase, sounds: bool, problem: Option<Problem>) {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(window) = handle.get_webview_window(LABEL) else {
            return;
        };
        place(&handle, &window);
        let _ = window.show();
        #[cfg(target_os = "windows")]
        keep_on_top_without_focus(&window);
        let _ = window.emit(
            "parler://overlay",
            OverlayEvent {
                phase,
                sounds,
                problem,
            },
        );
    });
}

/// Fades the pill out, then hides its window.
pub fn hide(app: &AppHandle) {
    hide_after(app, Duration::ZERO);
}

/// Shows a short-lived phase (nothing heard, an error) and hides it again.
pub fn flash(app: &AppHandle, phase: Phase, sounds: bool, problem: Option<Problem>) {
    show_with(app, phase, sounds, problem);
    hide_after(app, Duration::from_millis(2400));
}

fn hide_after(app: &AppHandle, delay: Duration) {
    let generation = GENERATION.load(Ordering::SeqCst);
    let handle = app.clone();
    thread::spawn(move || {
        thread::sleep(delay);
        if GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        let Some(window) = handle.get_webview_window(LABEL) else {
            return;
        };
        let _ = window.emit(
            "parler://overlay",
            OverlayEvent {
                phase: Phase::Hidden,
                sounds: false,
                problem: None,
            },
        );
        // Let the fade-out play before the window goes.
        thread::sleep(Duration::from_millis(220));
        if GENERATION.load(Ordering::SeqCst) == generation {
            let _ = window.hide();
        }
    });
}

/// Bottom centre of the work area of the screen the pointer is on.
fn place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let width = WIDTH * scale;
    let height = HEIGHT * scale;
    let x = area.position.x as f64 + (area.size.width as f64 - width) / 2.0;
    let y = area.position.y as f64 + area.size.height as f64 - height - BOTTOM_MARGIN * scale;
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

/// `always_on_top` alone gets overridden by other topmost windows; re-assert
/// it natively without activating the window.
#[cfg(target_os = "windows")]
fn keep_on_top_without_focus(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    if let Ok(hwnd) = window.hwnd() {
        // SAFETY: the handle belongs to a live window owned by this process.
        unsafe {
            let _ = SetWindowPos(
                HWND(hwnd.0),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
}
