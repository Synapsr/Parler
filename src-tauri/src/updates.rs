//! Updates, through Tauri's updater plugin.
//!
//! A release publishes `latest.json` next to signed bundles (see
//! `.github/workflows/release.yml`). Parler reads it a few seconds after
//! launch and every six hours, without a word unless there is something new:
//! then the window offers it. Installing downloads and checks the signature,
//! waits for any dictation to end, and restarts Parler.
//!
//! `PARLER_DISABLE_UPDATER=1` turns all of this off, for packagers that update
//! Parler themselves (Nix, distributions).

use crate::dictation::{State as DictationState, StatusState};
use serde::Serialize;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Error, Update, UpdaterExt};

/// Leaves launch to the things that matter first.
const FIRST_CHECK: Duration = Duration::from_secs(8);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// How often an update ready to install looks for the end of a dictation.
const DICTATION_POLL: Duration = Duration::from_millis(500);
/// Progress events, at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(120);

/// Where updates stand, as the window shows it.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateStatus {
    /// Not checked yet.
    Idle,
    Checking,
    UpToDate,
    Available {
        version: String,
    },
    Downloading {
        version: String,
        /// From 0 to 1, when the size is known.
        progress: Option<f64>,
    },
    /// Downloaded and verified: installs and restarts as soon as no
    /// dictation is under way (`waiting` while one is).
    Restarting {
        version: String,
        waiting: bool,
    },
    Failed {
        error: UpdateError,
        /// The update that failed to install, which can be tried again;
        /// `None` when checking failed.
        version: Option<String>,
    },
    /// Turned off by `PARLER_DISABLE_UPDATER`.
    Disabled,
}

/// Why checking or installing failed, in terms the window can explain.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateError {
    /// The release information couldn't be fetched.
    Offline,
    /// No update is published for this kind of install.
    Unsupported,
    /// The download isn't signed by Parler's key: it was not installed.
    Signature,
    Failed {
        detail: String,
    },
}

impl From<&Error> for UpdateError {
    fn from(error: &Error) -> Self {
        match error {
            Error::Reqwest(_) | Error::Network(_) | Error::Http(_) | Error::ReleaseNotFound => {
                Self::Offline
            }
            Error::TargetNotFound(_)
            | Error::TargetsNotFound(_)
            | Error::UnsupportedOs
            | Error::UnsupportedArch => Self::Unsupported,
            Error::Minisign(_)
            | Error::Base64(_)
            | Error::SignatureUtf8(_)
            | Error::SignedVersionMismatch { .. }
            | Error::MissingSignedVersion => Self::Signature,
            other => Self::Failed {
                detail: other.to_string(),
            },
        }
    }
}

pub struct Updates {
    status: Mutex<UpdateStatus>,
    /// The update found by the last check, ready to install.
    pending: Mutex<Option<Update>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Updates {
    pub fn status(&self) -> UpdateStatus {
        lock(&self.status).clone()
    }

    fn set(&self, app: &AppHandle, status: UpdateStatus) {
        *lock(&self.status) = status.clone();
        let _ = app.emit_to("main", "parler://update", status);
    }
}

/// Whether `PARLER_DISABLE_UPDATER` asks for no updates.
fn disabled_by(value: Option<&str>) -> bool {
    value
        .map(|v| v.trim().to_ascii_lowercase())
        .is_some_and(|v| !v.is_empty() && !matches!(v.as_str(), "0" | "false" | "no" | "off"))
}

/// The share of a download done, when its size is known.
pub fn progress(downloaded: u64, total: Option<u64>) -> Option<f64> {
    total
        .filter(|total| *total > 0)
        .map(|total| (downloaded as f64 / total as f64).min(1.0))
}

/// Keeps track of updates and checks for them in the background.
pub fn start(app: &AppHandle) {
    let disabled = disabled_by(std::env::var("PARLER_DISABLE_UPDATER").ok().as_deref());
    app.manage(Updates {
        status: Mutex::new(if disabled {
            UpdateStatus::Disabled
        } else {
            UpdateStatus::Idle
        }),
        pending: Mutex::new(None),
    });
    if disabled {
        log::info!("Updates are turned off (PARLER_DISABLE_UPDATER)");
        return;
    }
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("updates".into())
        .spawn(move || {
            thread::sleep(FIRST_CHECK);
            loop {
                tauri::async_runtime::block_on(check(&app, false));
                thread::sleep(CHECK_EVERY);
            }
        });
    if let Err(e) = spawned {
        log::warn!("Could not schedule update checks: {e}");
    }
}

/// Asks whether a newer Parler is published. A check from the window shows
/// its outcome, failures included; a background one only reports news.
pub async fn check(app: &AppHandle, manual: bool) -> UpdateStatus {
    let updates = app.state::<Updates>();
    let current = updates.status();
    if matches!(
        current,
        UpdateStatus::Disabled | UpdateStatus::Downloading { .. } | UpdateStatus::Restarting { .. }
    ) {
        return current;
    }
    if manual {
        updates.set(app, UpdateStatus::Checking);
    }
    let found = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let next = match found {
        Ok(Some(update)) => {
            log::info!("Parler {} is available", update.version);
            let version = update.version.clone();
            *lock(&updates.pending) = Some(update);
            UpdateStatus::Available { version }
        }
        Ok(None) => {
            *lock(&updates.pending) = None;
            UpdateStatus::UpToDate
        }
        Err(e) => {
            log::warn!("Update check failed: {e}");
            if !manual {
                return current;
            }
            UpdateStatus::Failed {
                error: UpdateError::from(&e),
                version: None,
            }
        }
    };
    updates.set(app, next.clone());
    next
}

/// Downloads the update found by the last check, installs it once no
/// dictation is under way, and restarts Parler.
pub async fn install(app: &AppHandle) -> Result<(), UpdateError> {
    let updates = app.state::<Updates>();
    let Some(update) = lock(&updates.pending).take() else {
        return Err(UpdateError::Failed {
            detail: "no update to install".to_string(),
        });
    };
    let version = update.version.clone();
    updates.set(
        app,
        UpdateStatus::Downloading {
            version: version.clone(),
            progress: Some(0.0),
        },
    );

    let mut downloaded = 0u64;
    let mut last_report = Instant::now();
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if last_report.elapsed() >= PROGRESS_EVERY {
                    last_report = Instant::now();
                    updates.set(
                        app,
                        UpdateStatus::Downloading {
                            version: version.clone(),
                            progress: progress(downloaded, total),
                        },
                    );
                }
            },
            || {},
        )
        .await;
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(e) => return Err(fail(app, &updates, update, &e)),
    };

    // Never in the middle of a dictation.
    let mut waiting = false;
    while dictating(app) {
        if !waiting {
            waiting = true;
            log::info!("Parler {version} will install once this dictation ends");
            updates.set(
                app,
                UpdateStatus::Restarting {
                    version: version.clone(),
                    waiting: true,
                },
            );
        }
        pause(DICTATION_POLL).await;
    }
    updates.set(
        app,
        UpdateStatus::Restarting {
            version: version.clone(),
            waiting: false,
        },
    );
    if let Err(e) = update.install(&bytes) {
        return Err(fail(app, &updates, update, &e));
    }
    log::info!("Installed Parler {version}; restarting");
    app.restart();
}

/// Reports a failed install, keeping the update so it can be tried again.
fn fail(app: &AppHandle, updates: &Updates, update: Update, error: &Error) -> UpdateError {
    log::warn!("Update to {} failed: {error}", update.version);
    let error = UpdateError::from(error);
    let version = Some(update.version.clone());
    *lock(&updates.pending) = Some(update);
    updates.set(
        app,
        UpdateStatus::Failed {
            error: error.clone(),
            version,
        },
    );
    error
}

fn dictating(app: &AppHandle) -> bool {
    let status = app.state::<StatusState>();
    let state = status.0.lock().unwrap_or_else(|e| e.into_inner()).state;
    state != DictationState::Idle
}

async fn pause(duration: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || thread::sleep(duration)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_turns_updates_off() {
        for on in ["1", "true", "yes", " TRUE "] {
            assert!(disabled_by(Some(on)), "{on}");
        }
        for off in ["", "0", "false", "No", "off"] {
            assert!(!disabled_by(Some(off)), "{off:?}");
        }
        assert!(!disabled_by(None));
    }

    #[test]
    fn progress_needs_a_known_size() {
        assert_eq!(progress(50, Some(200)), Some(0.25));
        assert_eq!(progress(300, Some(200)), Some(1.0));
        assert_eq!(progress(50, None), None);
        assert_eq!(progress(50, Some(0)), None);
    }

    #[test]
    fn errors_are_sorted_for_the_window() {
        assert_eq!(
            UpdateError::from(&Error::Network("unreachable".into())),
            UpdateError::Offline
        );
        assert_eq!(
            UpdateError::from(&Error::TargetNotFound("linux-x86_64-rpm".into())),
            UpdateError::Unsupported
        );
        assert_eq!(
            UpdateError::from(&Error::MissingSignedVersion),
            UpdateError::Signature
        );
        assert!(matches!(
            UpdateError::from(&Error::DebInstallFailed),
            UpdateError::Failed { .. }
        ));
    }

    /// The shape `.github/workflows/release.yml` writes to `latest.json`.
    const LATEST: &str = r#"{
        "version": "0.1.3",
        "notes": "What's new: https://github.com/Synapsr/Parler/releases/tag/v0.1.3",
        "pub_date": "2026-10-24T15:43:32Z",
        "platforms": {
            "darwin-aarch64": {"signature": "c2lnLW1hYw==", "url": "https://github.com/Synapsr/Parler/releases/download/v0.1.3/Parler-macOS.app.tar.gz"},
            "darwin-x86_64": {"signature": "c2lnLW1hYw==", "url": "https://github.com/Synapsr/Parler/releases/download/v0.1.3/Parler-macOS.app.tar.gz"},
            "windows-x86_64-nsis": {"signature": "c2lnLW5zaXM=", "url": "https://github.com/Synapsr/Parler/releases/download/v0.1.3/Parler-Windows-x64-setup.exe"},
            "windows-x86_64-msi": {"signature": "c2lnLW1zaQ==", "url": "https://github.com/Synapsr/Parler/releases/download/v0.1.3/Parler-Windows-x64.msi"},
            "linux-x86_64-appimage": {"signature": "c2lnLWFp", "url": "https://github.com/Synapsr/Parler/releases/download/v0.1.3/Parler-Linux-x86_64.AppImage"}
        }
    }"#;

    #[test]
    fn the_release_manifest_reads_as_the_plugin_expects() {
        let release: tauri_plugin_updater::RemoteRelease = serde_json::from_str(LATEST).unwrap();
        assert_eq!(release.version.to_string(), "0.1.3");
        // One universal archive serves both kinds of Mac.
        for mac in ["darwin-aarch64", "darwin-x86_64"] {
            assert!(release
                .download_url(mac)
                .unwrap()
                .as_str()
                .ends_with("/v0.1.3/Parler-macOS.app.tar.gz"));
            assert_eq!(release.signature(mac).unwrap(), "c2lnLW1hYw==");
        }
        assert!(release
            .download_url("windows-x86_64-msi")
            .unwrap()
            .as_str()
            .ends_with(".msi"));
        assert!(release.download_url("linux-x86_64-rpm").is_err());
    }

    #[test]
    fn statuses_read_well_in_the_window() {
        let json = serde_json::to_value(UpdateStatus::Downloading {
            version: "0.1.3".into(),
            progress: Some(0.5),
        })
        .unwrap();
        assert_eq!(json["state"], "downloading");
        assert_eq!(json["version"], "0.1.3");
        let failed = serde_json::to_value(UpdateStatus::Failed {
            error: UpdateError::Offline,
            version: None,
        })
        .unwrap();
        assert_eq!(failed["error"]["kind"], "offline");
        assert_eq!(failed["version"], serde_json::Value::Null);
    }
}
