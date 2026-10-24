//! What the windows can ask of the backend.

use crate::audio::{self, Microphone};
use crate::cloud::{self, Account, DeviceStart, Poll, SignIn};
use crate::dictation::{Dictation, Event, Problem, Status, StatusState};
use crate::history::{self, Entry, History, Stats};
use crate::paste::{self, PasteError};
use crate::providers::{self, Provider, Request, TranscribeError};
use crate::settings::{Settings, SettingsStore};
use crate::{shortcut, tray};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

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
pub fn save_settings(app: AppHandle, mut settings: Settings) -> Result<Settings, String> {
    let store = app.state::<SettingsStore>();
    let previous = store.get();
    keep_cloud_state(&mut settings, &previous);
    if settings.shortcut != previous.shortcut {
        shortcut::replace(&app, &previous.shortcut, &settings.shortcut)?;
        shortcut::clear_problem(&app);
    }
    store.set(settings.clone()).map_err(|e| e.to_string())?;
    Ok(settings)
}

/// The Parler Cloud token and the service to go back to are the backend's:
/// a window saving its copy of the settings never changes them, so a copy
/// taken before signing in can't erase the token.
fn keep_cloud_state(settings: &mut Settings, previous: &Settings) {
    match previous.api_keys.get(cloud::ID) {
        Some(token) => settings
            .api_keys
            .insert(cloud::ID.to_string(), token.clone()),
        None => settings.api_keys.remove(cloud::ID),
    };
    settings.provider_before_cloud =
        if settings.provider == cloud::ID && previous.provider != cloud::ID {
            Some(previous.provider.clone())
        } else {
            previous.provider_before_cloud.clone()
        };
}

/// Saves settings changed by the backend and hands them to the window, so
/// its copy stays current.
fn save_and_notify(app: &AppHandle, settings: Settings) -> Result<Settings, TranscribeError> {
    app.state::<SettingsStore>()
        .set(settings.clone())
        .map_err(|e| TranscribeError::Rejected {
            provider: cloud::LABEL.to_string(),
            status: 0,
            message: e.to_string(),
        })?;
    let _ = app.emit_to("main", "parler://settings", settings.clone());
    Ok(settings)
}

/// Once signed in again, a problem that asked for it is out of date.
fn clear_cloud_problem(app: &AppHandle) {
    let state = app.state::<StatusState>();
    let mut status = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(Problem::Transcription {
        error: TranscribeError::SignedOut { .. } | TranscribeError::SubscriptionRequired { .. },
    }) = status.problem
    {
        status.problem = None;
        let _ = app.emit_to("main", "parler://status", status.clone());
    }
}

#[tauri::command]
pub fn get_providers() -> Vec<Provider> {
    providers::all()
}

/// Sends half a second of silence with `settings`, which need not be saved
/// yet, so a wrong key, model or URL shows up during setup. Parler Cloud
/// checks the account instead, which costs no dictation time.
#[tauri::command]
pub async fn test_provider(settings: Settings) -> Result<(), TranscribeError> {
    let request = Request::from_settings(&settings)?;
    if settings.provider == cloud::ID {
        let token = settings
            .api_keys
            .get(cloud::ID)
            .cloned()
            .unwrap_or_default();
        return cloud::account(cloud::base_url(), token.trim())
            .await
            .map(|_| ());
    }
    let silence = audio::encode_wav(&[0.0; audio::SAMPLE_RATE as usize / 2]);
    providers::transcribe(&request, silence).await.map(|_| ())
}

/// Starts linking this computer to Parler Cloud: gets a code and opens the
/// page where the person signs in and authorizes it.
#[tauri::command]
pub async fn cloud_sign_in_start(app: AppHandle) -> Result<DeviceStart, TranscribeError> {
    let name = tauri::async_runtime::spawn_blocking(cloud::device_name)
        .await
        .unwrap_or_else(|_| "Computer".to_string());
    let start = cloud::start(cloud::base_url(), &name, std::env::consts::OS).await?;
    let url = &start.verification_url;
    if url.starts_with("https://") || url.starts_with("http://") {
        if let Err(e) = app.opener().open_url(url, None::<&str>) {
            log::warn!("Could not open the browser: {e}");
        }
    }
    Ok(start)
}

/// Asks once whether the person authorized this computer. Once they have,
/// the token is saved, Parler Cloud becomes the service and the window comes
/// back to the front.
#[tauri::command]
pub async fn cloud_sign_in_poll(
    app: AppHandle,
    device_code: String,
) -> Result<SignIn, TranscribeError> {
    Ok(match cloud::poll(cloud::base_url(), &device_code).await? {
        Poll::Pending => SignIn::Pending,
        Poll::Denied => SignIn::Denied,
        Poll::Expired => SignIn::Expired,
        Poll::Approved { token, account } => {
            let store = app.state::<SettingsStore>();
            let mut settings = store.get();
            if settings.provider != cloud::ID {
                settings.provider_before_cloud = Some(settings.provider.clone());
                settings.provider = cloud::ID.to_string();
            }
            settings.api_keys.insert(cloud::ID.to_string(), token);
            save_and_notify(&app, settings)?;
            clear_cloud_problem(&app);
            log::info!("Signed in to {}", cloud::LABEL);
            crate::show_main_window(&app);
            SignIn::Approved { account }
        }
    })
}

/// The Parler Cloud account, or `None` when this computer isn't signed in.
/// A token the service no longer knows is forgotten.
#[tauri::command]
pub async fn cloud_account(app: AppHandle) -> Result<Option<Account>, TranscribeError> {
    let token = cloud_token(&app);
    if token.is_empty() {
        return Ok(None);
    }
    match cloud::account(cloud::base_url(), &token).await {
        Ok(account) => Ok(Some(account)),
        Err(TranscribeError::SignedOut { .. }) => {
            let mut settings = app.state::<SettingsStore>().get();
            // Unless a new sign-in replaced it meanwhile.
            if settings.api_keys.get(cloud::ID) == Some(&token) {
                log::info!("{} no longer knows this computer", cloud::LABEL);
                settings.api_keys.remove(cloud::ID);
                save_and_notify(&app, settings)?;
            }
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Unlinks this computer from Parler Cloud and goes back to the service used
/// before it, or Groq.
#[tauri::command]
pub async fn cloud_sign_out(app: AppHandle) -> Result<Settings, TranscribeError> {
    let token = cloud_token(&app);
    if !token.is_empty() {
        // Best effort: offline, the token is still forgotten here.
        if let Err(e) = cloud::revoke(cloud::base_url(), &token).await {
            log::warn!("Could not revoke the {} token: {e}", cloud::LABEL);
        }
    }
    let mut settings = app.state::<SettingsStore>().get();
    settings.api_keys.remove(cloud::ID);
    if settings.provider == cloud::ID {
        settings.provider = settings
            .provider_before_cloud
            .take()
            .filter(|id| id != cloud::ID && providers::find(id).is_some())
            .unwrap_or_else(|| "groq".to_string());
    }
    log::info!("Signed out of {}", cloud::LABEL);
    save_and_notify(&app, settings)
}

fn cloud_token(app: &AppHandle) -> String {
    app.state::<SettingsStore>()
        .get()
        .api_keys
        .get(cloud::ID)
        .map(|token| token.trim().to_string())
        .unwrap_or_default()
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

/// The tray menu's words, in the interface language.
#[tauri::command]
pub fn set_tray_labels(app: AppHandle, labels: tray::Labels) {
    tray::set_labels(&app, labels);
}

/// While the window records a new shortcut, the current one must not start
/// a dictation.
#[tauri::command]
pub fn suspend_shortcut(store: State<SettingsStore>, app: AppHandle) {
    shortcut::unregister(&app, &store.get().shortcut);
}

#[tauri::command]
pub fn resume_shortcut(store: State<SettingsStore>, app: AppHandle) -> Result<(), String> {
    let accelerator = store.get().shortcut;
    if shortcut::is_registered(&app, &accelerator) {
        return Ok(());
    }
    shortcut::register(&app, &accelerator)
}

/// What macOS itself does when the fn (🌐) key is pressed, as set in System
/// Settings → Keyboard: 0 nothing, 1 change the input source, 2 show emoji,
/// 3 start Dictation; `None` when never changed (the system default, which
/// does something). Always `None` on other systems.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobeKey {
    action: Option<i64>,
}

#[tauri::command]
pub async fn globe_key() -> GlobeKey {
    #[cfg(target_os = "macos")]
    let action = tauri::async_runtime::spawn_blocking(|| {
        let output = std::process::Command::new("defaults")
            .args(["read", "com.apple.HIToolbox", "AppleFnUsageType"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    })
    .await
    .ok()
    .flatten();
    #[cfg(not(target_os = "macos"))]
    let action = None;
    GlobeKey { action }
}

/// System Settings → Keyboard, where the 🌐 key's own action is chosen.
#[tauri::command]
pub fn open_keyboard_settings(app: AppHandle) {
    if let Err(e) = app.opener().open_url(
        "x-apple.systempreferences:com.apple.Keyboard-Settings.extension",
        None::<&str>,
    ) {
        log::warn!("Could not open the keyboard settings: {e}");
    }
}
