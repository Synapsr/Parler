//! What the windows can ask of the backend.

use crate::audio::{self, Microphone};
use crate::dictation::{Dictation, Event, Problem, Status, StatusState};
use crate::history::{self, Entry, History, Stats};
use crate::paste::{self, PasteError};
use crate::providers::{self, Provider, Request, TranscribeError, PROVIDERS};
use crate::settings::{Settings, SettingsStore};
use crate::{shortcut, tray};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    platform: &'static str,
    version: &'static str,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        platform: std::env::consts::OS,
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[tauri::command]
pub fn get_settings(store: State<SettingsStore>) -> Settings {
    store.get()
}

/// Saves the settings, registering a changed shortcut first: if the new one
/// can't be registered nothing is saved.
#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let store = app.state::<SettingsStore>();
    let previous = store.get();
    if settings.shortcut != previous.shortcut {
        shortcut::replace(&app, &previous.shortcut, &settings.shortcut)?;
        clear_shortcut_problem(&app);
    }
    store.set(settings.clone()).map_err(|e| e.to_string())?;
    Ok(settings)
}

fn clear_shortcut_problem(app: &AppHandle) {
    let state = app.state::<StatusState>();
    let mut status = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if matches!(status.problem, Some(Problem::Shortcut { .. })) {
        status.problem = None;
        let _ = app.emit_to("main", "parler://status", status.clone());
    }
}

#[tauri::command]
pub fn get_providers() -> Vec<Provider> {
    PROVIDERS.to_vec()
}

/// Sends half a second of silence with `settings`, which need not be saved
/// yet, so a wrong key, model or URL shows up during setup.
#[tauri::command]
pub async fn test_provider(settings: Settings) -> Result<(), TranscribeError> {
    let request = Request::from_settings(&settings)?;
    let silence = audio::encode_wav(&[0.0; audio::SAMPLE_RATE as usize / 2]);
    providers::transcribe(&request, silence).await.map(|_| ())
}

#[tauri::command]
pub async fn list_microphones() -> Vec<Microphone> {
    tauri::async_runtime::spawn_blocking(audio::list_microphones)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_history(history: State<History>) -> Vec<Entry> {
    history.entries()
}

#[tauri::command]
pub fn get_stats(history: State<History>) -> Stats {
    history.stats()
}

#[tauri::command]
pub fn delete_history_entry(app: AppHandle, id: u64) {
    app.state::<History>().remove(id);
    history::notify(&app);
}

#[tauri::command]
pub fn clear_history(app: AppHandle) {
    app.state::<History>().clear();
    history::notify(&app);
}

/// Async so it never runs on the main thread, which a paste in progress may
/// be waiting on while it holds the clipboard.
#[tauri::command]
pub async fn copy_text(app: AppHandle, text: String) -> Result<(), PasteError> {
    paste::copy(&app, &text)
}

#[tauri::command]
pub fn get_status(status: State<StatusState>) -> Status {
    status.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[tauri::command]
pub fn toggle_dictation(dictation: State<Dictation>) {
    dictation.send(Event::Toggle);
}

#[tauri::command]
pub fn retry_dictation(dictation: State<Dictation>) {
    dictation.send(Event::Retry);
}

#[tauri::command]
pub fn set_locale(app: AppHandle, locale: String) {
    tray::set_locale(&app, &locale);
}

/// While the window records a new shortcut, the current one must not start
/// a dictation.
#[tauri::command]
pub fn suspend_shortcut(store: State<SettingsStore>, app: AppHandle) {
    shortcut::unregister(&app, &store.get().shortcut);
}

#[tauri::command]
pub fn resume_shortcut(store: State<SettingsStore>, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let accelerator = store.get().shortcut;
    if app.global_shortcut().is_registered(accelerator.as_str()) {
        return Ok(());
    }
    shortcut::register(&app, &accelerator)
}
