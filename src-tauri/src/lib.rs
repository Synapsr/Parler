mod audio;
mod cloud;
mod commands;
mod diagnostics;
mod dictation;
mod history;
#[cfg(target_os = "macos")]
mod modifier_key;
mod overlay;
mod paste;
mod pending;
mod providers;
mod settings;
mod shortcut;
mod tray;
mod updates;

use dictation::{Dictation, Event, Problem, State, Status, StatusState};
use history::History;
use paste::ClipboardState;
use settings::SettingsStore;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// Passed by the login item: start in the tray without opening the window.
const HIDDEN_FLAG: &str = "--hidden";
/// `parler --toggle` starts or stops a dictation in the running Parler; bind
/// it to a desktop shortcut where global shortcuts don't work (Wayland).
const TOGGLE_FLAG: &str = "--toggle";

/// macOS: set while the window is hidden and Parler lives in the menu bar
/// alone, out of the Dock and the app switcher.
#[cfg(target_os = "macos")]
static MENU_BAR_ONLY: AtomicBool = AtomicBool::new(false);

pub fn show_main_window(app: &AppHandle) {
    // macOS: back in the Dock first, so the window comes to the front with
    // its app menu.
    #[cfg(target_os = "macos")]
    if MENU_BAR_ONLY.swap(false, Ordering::SeqCst) {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Parler keeps running in the tray, ready to dictate. On macOS it leaves the
/// Dock as well, like other menu bar apps; the menu bar icon, or opening
/// Parler again, brings the window back.
fn hide_main_window(window: &tauri::Window) {
    let _ = window.hide();
    #[cfg(target_os = "macos")]
    {
        MENU_BAR_ONLY.store(true, Ordering::SeqCst);
        let _ = window
            .app_handle()
            .set_activation_policy(tauri::ActivationPolicy::Accessory);
    }
}

/// Panics reach the log, and so a report to support, as well as stderr; a
/// panic before the log plugin starts only reaches stderr.
fn log_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("no message");
        let place = info
            .location()
            .map(ToString::to_string)
            .unwrap_or_else(|| "an unknown place".to_string());
        let thread = std::thread::current();
        log::error!(
            "Panic on the {} thread at {place}: {message}",
            thread.name().unwrap_or("unnamed")
        );
        // The file is written in batches; the process may be about to end.
        log::logger().flush();
        previous(info);
    }));
}

pub fn run() {
    log_panics();
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
                // Weeks of use rather than hours, for a report to support.
                .max_file_size(512 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(2))
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            // Launch at login names whichever copy of Parler turned it on (a
            // dev build, an older download): the installed app takes it back.
            #[cfg(not(debug_assertions))]
            {
                use tauri_plugin_autostart::ManagerExt;
                let launcher = app.autolaunch();
                if launcher.is_enabled().unwrap_or(false) {
                    if let Err(e) = launcher.enable() {
                        log::warn!("Couldn't refresh launch at login: {e}");
                    }
                }
            }
            if cloud::base_url() != cloud::DEFAULT_URL {
                log::info!("Parler Cloud at {}", cloud::base_url());
            }
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            app.manage(SettingsStore::load(config_dir.join("settings.json")));
            // Started at login: in the menu bar alone, once set up.
            let hidden = app.state::<SettingsStore>().get().onboarded
                && std::env::args().any(|arg| arg == HIDDEN_FLAG);
            #[cfg(target_os = "macos")]
            if hidden {
                MENU_BAR_ONLY.store(true, Ordering::SeqCst);
                // Before the overlay is built: its panel puts back the policy
                // it finds, and an app that was ever regular keeps its Dock
                // icon. Tao applies its own once launched: the same one.
                let _ = handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            app.manage(History::load(data_dir.join("history.json")));
            app.manage(ClipboardState::new());
            app.manage(StatusState(Mutex::new(Status {
                state: State::Idle,
                problem: None,
                can_retry: false,
            })));
            app.manage(Dictation::spawn(&handle));
            updates::start(&handle);

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

            if !hidden {
                show_main_window(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps Parler running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    hide_main_window(window);
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
            commands::get_pending,
            commands::delete_pending,
            commands::microphone_access,
            commands::open_microphone_settings,
            commands::set_tray_labels,
            commands::suspend_shortcut,
            commands::resume_shortcut,
            commands::globe_key,
            commands::open_keyboard_settings,
            commands::github_stars,
            commands::send_diagnostics,
            commands::reveal_log,
            commands::update_status,
            commands::check_for_update,
            commands::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("Parler failed to start")
        .run(|app, event| {
            // macOS: clicking the Dock icon, or opening Parler again from
            // the Finder or Spotlight, brings the window back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main_window(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
