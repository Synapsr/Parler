//! A report for support, sent from Settings → About: Parler's version, the
//! system, a few settings, the permissions and the end of the log, with what
//! the person wrote.
//! Parler Cloud forwards it by email and keeps nothing (`POST
//! /api/diagnostics`, see docs/architecture.md). The log holds no dictation
//! and no key; anything shaped like a key is masked here all the same.

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::audio::{self, MicrophoneAccess};
use crate::settings::{Settings, SettingsStore};
use crate::{cloud, providers};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// How much of the log is sent: the last few days of use.
const LOG_TAIL: usize = 200 * 1024;
/// The longest note the service takes.
const NOTE_MAX: usize = 4000;
/// A report is one small upload; past this, the service is unreachable.
const TIMEOUT: Duration = Duration::from_secs(30);
/// Words that start a secret: provider keys, Parler Cloud device tokens.
const SECRET_PREFIXES: &[&str] = &["sk-", "sk_", "gsk_", "fw_", "pc_", "xai-"];

/// Why a report didn't go.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SendError {
    /// No connection, or the service didn't answer.
    Offline,
    /// Several reports already went out this hour.
    TooMany,
    /// The service answered, but couldn't take it.
    Failed,
}

#[derive(Serialize)]
struct Report<'a> {
    note: &'a str,
    app: BTreeMap<&'static str, String>,
    log: String,
}

#[derive(Deserialize)]
struct Sent {
    reference: String,
}

/// Sends the report; the reference the person can quote.
pub async fn send(app: &AppHandle, note: &str) -> Result<String, SendError> {
    let settings = app.state::<SettingsStore>().get();
    let note: String = note.trim().chars().take(NOTE_MAX).collect();
    let report = Report {
        note: &note,
        app: fields(
            &app.package_info().version.to_string(),
            &settings,
            &permissions().await,
        ),
        log: mask_secrets(&read_tail(&log_files(app), LOG_TAIL)),
    };
    // Signed in to Parler Cloud: support can answer the account's address.
    let token = settings
        .api_keys
        .get(cloud::ID)
        .map(String::as_str)
        .filter(|t| !t.is_empty());
    post(cloud::base_url(), token, &report).await
}

async fn post(base: &str, token: Option<&str>, report: &Report<'_>) -> Result<String, SendError> {
    let client = providers::http_client(TIMEOUT).map_err(|_| SendError::Offline)?;
    let mut request = client.post(format!("{base}/api/diagnostics")).json(report);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|e| {
        log::warn!("Could not send the log to support: {e}");
        SendError::Offline
    })?;
    match response.status().as_u16() {
        200 => {
            let sent: Sent = response.json().await.map_err(|_| SendError::Failed)?;
            log::info!("Sent the log to support ({})", sent.reference);
            Ok(sent.reference)
        }
        429 => Err(SendError::TooMany),
        status => {
            log::warn!("Support answered {status} to the log");
            Err(SendError::Failed)
        }
    }
}

/// The log being written now, to show in the file manager.
pub fn current_log(app: &AppHandle) -> Option<PathBuf> {
    log_files(app).pop()
}

/// What support needs to know besides the log: never a key, a word of
/// vocabulary or a microphone's name.
fn fields(
    version: &str,
    settings: &Settings,
    permissions: &[(&'static str, &'static str)],
) -> BTreeMap<&'static str, String> {
    let mut fields = BTreeMap::new();
    fields.insert("version", version.to_string());
    fields.insert("os", std::env::consts::OS.to_string());
    if let Some(version) = os_version() {
        fields.insert("osVersion", version);
    }
    fields.insert("arch", std::env::consts::ARCH.to_string());
    fields.insert("provider", settings.provider.clone());
    fields.insert("shortcut", settings.shortcut.clone());
    fields.insert(
        "shortcutMode",
        format!("{:?}", settings.shortcut_mode).to_lowercase(),
    );
    fields.insert("output", format!("{:?}", settings.output).to_lowercase());
    fields.insert("language", settings.language.clone());
    fields.insert("uiLanguage", settings.ui_language.clone());
    fields.insert(
        "microphone",
        if settings.microphone.is_some() {
            "chosen"
        } else {
            "default"
        }
        .to_string(),
    );
    for &(name, state) in permissions {
        fields.insert(name, state.to_string());
    }
    fields
}

/// What the system lets Parler do, the first thing support checks when
/// nothing is heard or nothing is pasted. Only read: nothing here asks.
// What gets pushed depends on the platform: none on Linux, one on Windows,
// two on macOS.
#[allow(clippy::vec_init_then_push)]
async fn permissions() -> Vec<(&'static str, &'static str)> {
    #[allow(unused_mut)]
    let mut permissions = Vec::new();
    // Linux can't tell before a recording fails.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    permissions.push((
        "microphoneAccess",
        match audio::microphone_access() {
            MicrophoneAccess::Granted => "granted",
            // Restricted by a policy (a managed Mac) reads as denied too.
            MicrophoneAccess::Denied => "denied",
            MicrophoneAccess::NotDetermined => "not determined",
            MicrophoneAccess::Unknown => "unknown",
        },
    ));
    // To paste, and to follow a key alone as the shortcut.
    #[cfg(target_os = "macos")]
    permissions.push((
        "accessibility",
        if tauri_plugin_macos_permissions::check_accessibility_permission().await {
            "granted"
        } else {
            "not granted"
        },
    ));
    permissions
}

fn os_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    return cloud::command_output("sw_vers", &["-productVersion"]).map(|v| v.trim().to_string());
    #[cfg(target_os = "linux")]
    return std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|release| {
            release
                .lines()
                .find_map(|line| line.strip_prefix("PRETTY_NAME="))
                .map(|name| name.trim_matches('"').to_string())
        });
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    None
}

/// Parler's log files, oldest first: archived ones, then the current one.
fn log_files(app: &AppHandle) -> Vec<PathBuf> {
    let Ok(dir) = app.path().app_log_dir() else {
        return Vec::new();
    };
    let name = app.package_info().name.clone();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_log_of(path, &name))
        .filter_map(|path| Some((path.metadata().ok()?.modified().ok()?, path)))
        .collect();
    files.sort();
    files.into_iter().map(|(_, path)| path).collect()
}

fn is_log_of(path: &Path, name: &str) -> bool {
    path.extension().is_some_and(|ext| ext == "log")
        && path
            .file_name()
            .and_then(|file| file.to_str())
            .is_some_and(|file| file.starts_with(name))
}

/// The last `limit` bytes of the files, read in order, from a line start.
fn read_tail(files: &[PathBuf], limit: usize) -> String {
    let mut text = String::new();
    for file in files {
        if let Ok(bytes) = std::fs::read(file) {
            text.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    if text.len() <= limit {
        return text;
    }
    let mut start = text.len() - limit;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let tail = &text[start..];
    // Drop the cut line.
    match tail.find('\n') {
        Some(newline) => tail[newline + 1..].to_string(),
        None => tail.to_string(),
    }
}

/// Masks words shaped like secrets (`sk-proj-abc…`, `gsk_…`, `pc_…`) and
/// passwords in URLs (`https://user:pass@host`), keeping their start so a
/// reader still sees which kind it was.
fn mask_secrets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        out.push_str(&mask_word(word));
        word.clear();
    };
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '*' | '.' | ':' | '/' | '@') {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

fn mask_word(word: &str) -> String {
    // A URL with a password: keep the scheme and the host.
    if let Some(scheme_end) = word.find("://") {
        let rest = &word[scheme_end + 3..];
        if let Some(at) = rest.find('@') {
            if rest[..at].contains(':') {
                return format!("{}://…@{}", &word[..scheme_end], &rest[at + 1..]);
            }
        }
        return word.to_string();
    }
    // A secret starts the word, or follows `key:`; `task-queue-worker` is
    // not one.
    for (position, _) in word.char_indices() {
        let starts = position == 0 || word[..position].ends_with([':', '/', '@']);
        if !starts {
            continue;
        }
        let rest = &word[position..];
        for prefix in SECRET_PREFIXES {
            if rest.starts_with(prefix) && rest.len() >= prefix.len() + 8 {
                return format!("{}{prefix}…", &word[..position]);
            }
        }
    }
    word.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_tokens_are_masked() {
        let line = "OpenAI answered 401: Incorrect API key provided: sk-proj-abc*****wxyz. Use gsk_8f7d6e5c4b3a2910 or pc_Zx9yW8vU7tS6rQ5p.";
        let masked = mask_secrets(line);
        assert_eq!(
            masked,
            "OpenAI answered 401: Incorrect API key provided: sk-… Use gsk_… or pc_…"
        );
        assert_eq!(mask_secrets("key:gsk_8f7d6e5c4b3a2910"), "key:gsk_…");
    }

    #[test]
    fn ordinary_words_are_kept() {
        let line = "[parler_lib::audio][INFO] Recording from Micro MacBook Air: task-queue-worker-thread, rpc_connection_pool, sk-8";
        assert_eq!(mask_secrets(line), line);
    }

    #[test]
    fn passwords_in_urls_are_masked() {
        assert_eq!(
            mask_secrets("custom_base_url: \"http://me:hunter2@box.local:8000/v1\""),
            "custom_base_url: \"http://…@box.local:8000/v1\""
        );
        assert_eq!(
            mask_secrets("Parler Cloud at https://cloud.parler.synapsr.io"),
            "Parler Cloud at https://cloud.parler.synapsr.io"
        );
    }

    /// Sends a report to a Parler Cloud running locally:
    /// `PARLER_CLOUD_URL=http://localhost:3100 cargo test --no-default-features diagnostics_e2e -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn diagnostics_e2e_against_a_running_service() {
        let base = std::env::var("PARLER_CLOUD_URL").expect("PARLER_CLOUD_URL");
        let report = Report {
            note: "Sent by the e2e test",
            app: BTreeMap::from([
                ("version", "0.0.0-test".to_string()),
                ("os", "test".to_string()),
            ]),
            log: mask_secrets(
                "[parler_lib::dictation][INFO] Dictation cancelled\nkey gsk_8f7d6e5c4b3a2910\n",
            ),
        };
        let reference = tauri::async_runtime::block_on(post(&base, None, &report)).unwrap();
        println!("REFERENCE {reference}");
        assert!(reference.starts_with("PRL-") && reference.len() == 10);
    }

    #[test]
    fn fields_name_settings_and_permissions_but_no_secret() {
        let settings = Settings {
            api_keys: [("groq".to_string(), "gsk_8f7d6e5c4b3a2910".to_string())].into(),
            vocabulary: "Synapsr".to_string(),
            microphone: Some("MacBook Air Microphone".to_string()),
            ..Settings::default()
        };
        let fields = fields(
            "0.1.5",
            &settings,
            &[("microphoneAccess", "denied"), ("accessibility", "granted")],
        );
        assert_eq!(fields["version"], "0.1.5");
        assert_eq!(fields["microphone"], "chosen");
        assert_eq!(fields["microphoneAccess"], "denied");
        assert_eq!(fields["accessibility"], "granted");
        let all = format!("{fields:?}");
        assert!(!all.contains("gsk_") && !all.contains("Synapsr") && !all.contains("MacBook"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_reads_both_permissions() {
        let permissions = tauri::async_runtime::block_on(permissions());
        let names: Vec<&str> = permissions.iter().map(|&(name, _)| name).collect();
        assert_eq!(names, ["microphoneAccess", "accessibility"]);
        let microphone = permissions[0].1;
        assert!(["granted", "denied", "not determined", "unknown"].contains(&microphone));
        assert!(["granted", "not granted"].contains(&permissions[1].1));
    }

    #[test]
    fn the_tail_starts_at_a_whole_line() {
        let dir = std::env::temp_dir().join(format!("parler-log-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("Parler_2026-10-01.log");
        let current = dir.join("Parler.log");
        std::fs::write(&old, "first line\nsecond line\n").unwrap();
        std::fs::write(&current, "third line\nfourth é line\n").unwrap();
        let files = vec![old, current];
        assert_eq!(
            read_tail(&files, 1000),
            "first line\nsecond line\nthird line\nfourth é line\n"
        );
        assert_eq!(read_tail(&files, 20), "fourth é line\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
