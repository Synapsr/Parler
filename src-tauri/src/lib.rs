mod audio;
mod cloud;
mod commands;
mod dictation;
mod history;
#[cfg(target_os = "macos")]
mod modifier_key;
mod overlay;
mod paste;
mod providers;
mod settings;
mod shortcut;
mod tray;

use dictation::{Dictation, Event, Problem, State, Status, StatusState};
use history::History;
use paste::ClipboardState;
use settings::SettingsStore;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// Passed by the login item: start in the tray without opening the window.
const HIDDEN_FLAG: &str = "--hidden";
/// `parler --toggle` starts or stops a dictation in the running Parler; bind
/// it to a desktop shortcut where global shortcuts don't work (Wayland).
const TOGGLE_FLAG: &str = "--toggle";

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        // First: a second launch hands its arguments over and exits before
        // anything else starts.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|arg| arg == TOGGLE_FLAG) {
                app.state::<Dictation>().send(Event::Toggle);
            } else {
                show_main_window(app);
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![HIDDEN_FLAG]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_macos_permissions::init());

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .setup(|app| {
            let handle = app.handle().clone();
            if cloud::base_url() != cloud::DEFAULT_URL {
                log::info!("Parler Cloud at {}", cloud::base_url());
            }
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            app.manage(SettingsStore::load(config_dir.join("settings.json")));
            app.manage(History::load(data_dir.join("history.json")));
            app.manage(ClipboardState::new());
            app.manage(StatusState(Mutex::new(Status {
                state: State::Idle,
                problem: None,
                can_retry: false,
            })));
            app.manage(Dictation::spawn(&handle));

            overlay::create(&handle);
            tray::create(&handle)?;

            let settings = app.state::<SettingsStore>().get();
            if let Err(detail) = shortcut::register(&handle, &settings.shortcut) {
                log::warn!("Shortcut {} unavailable: {detail}", settings.shortcut);
                let status = app.state::<StatusState>();
                status.0.lock().unwrap_or_else(|e| e.into_inner()).problem =
                    Some(Problem::Shortcut {
                        shortcut: settings.shortcut.clone(),
                        detail,
                    });
            }

            let hidden = std::env::args().any(|arg| arg == HIDDEN_FLAG);
            if !hidden || !settings.onboarded {
                show_main_window(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps Parler running in the tray, ready to
            // dictate.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_settings,
            commands::save_settings,
            commands::get_providers,
            commands::test_provider,
            commands::cloud_sign_in_start,
            commands::cloud_sign_in_poll,
            commands::cloud_account,
            commands::cloud_sign_out,
            commands::list_microphones,
            commands::get_history,
            commands::get_stats,
            commands::delete_history_entry,
            commands::clear_history,
            commands::copy_text,
            commands::get_status,
            commands::toggle_dictation,
            commands::retry_dictation,
            commands::set_tray_labels,
            commands::suspend_shortcut,
            commands::resume_shortcut,
            commands::globe_key,
            commands::open_keyboard_settings,
        ])
        .build(tauri::generate_context!())
        .expect("Parler failed to start")
        .run(|app, event| {
            // macOS: clicking the Dock icon brings the window back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main_window(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
