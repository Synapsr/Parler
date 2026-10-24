//! User settings, kept as one JSON file in the app's config directory.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// How the dictation shortcut behaves.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutMode {
    /// Hold to talk, or tap once to start and again to stop.
    #[default]
    Smart,
    /// Recording lasts exactly as long as the shortcut is held.
    Hold,
    /// Each press starts or stops recording.
    Toggle,
}

/// What happens to the transcript.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Output {
    /// Typed into the focused app; the clipboard is left as it was.
    #[default]
    Paste,
    /// Only copied to the clipboard.
    Copy,
}

/// The interface's colours.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// Follows the system's light or dark appearance.
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Set once the first-run setup is done.
    pub onboarded: bool,
    pub provider: String,
    /// API key per provider id. Parler Cloud's is the device token from
    /// signing in, which only the backend writes.
    pub api_keys: HashMap<String, String>,
    /// The service in use before Parler Cloud was picked, to go back to on
    /// signing out. Kept by the backend.
    pub provider_before_cloud: Option<String>,
    /// Model per provider id; a provider without one uses its default.
    pub models: HashMap<String, String>,
    /// Base URL of the custom provider, the only editable one.
    pub custom_base_url: String,
    /// ISO-639-1 code, or "auto" to let the model detect it.
    pub language: String,
    /// Names and jargon, sent to the model as a spelling hint.
    pub vocabulary: String,
    /// Accelerator such as "Alt+Space", as understood by the global-shortcut
    /// plugin.
    pub shortcut: String,
    pub shortcut_mode: ShortcutMode,
    /// An input device id; `None` follows the system default.
    pub microphone: Option<String>,
    pub output: Output,
    pub sounds: bool,
    /// "auto", "en" or "fr".
    pub ui_language: String,
    pub theme: Theme,
}

pub fn default_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "Alt+Space"
    } else {
        "Control+Space"
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarded: false,
            provider: "groq".to_string(),
            api_keys: HashMap::new(),
            provider_before_cloud: None,
            models: HashMap::new(),
            custom_base_url: crate::providers::DEFAULT_CUSTOM_BASE_URL.to_string(),
            language: "auto".to_string(),
            vocabulary: String::new(),
            shortcut: default_shortcut().to_string(),
            shortcut_mode: ShortcutMode::default(),
            microphone: None,
            output: Output::default(),
            sounds: true,
            ui_language: "auto".to_string(),
            theme: Theme::default(),
        }
    }
}

/// API keys never reach the logs.
impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<&String> = self
            .api_keys
            .iter()
            .filter(|(_, key)| !key.is_empty())
            .map(|(provider, _)| provider)
            .collect();
        f.debug_struct("Settings")
            .field("onboarded", &self.onboarded)
            .field("provider", &self.provider)
            .field("api_keys_for", &keys)
            .field("provider_before_cloud", &self.provider_before_cloud)
            .field("models", &self.models)
            .field("custom_base_url", &self.custom_base_url)
            .field("language", &self.language)
            .field("shortcut", &self.shortcut)
            .field("shortcut_mode", &self.shortcut_mode)
            .field("microphone", &self.microphone)
            .field("output", &self.output)
            .field("sounds", &self.sounds)
            .field("ui_language", &self.ui_language)
            .field("theme", &self.theme)
            .finish_non_exhaustive()
    }
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Reads the settings file. A missing or unreadable file starts from the
    /// defaults rather than failing the launch.
    pub fn load(path: PathBuf) -> Self {
        let current = match fs::read_to_string(&path) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
                log::warn!("Settings file is unreadable, using defaults: {e}");
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        log::info!("Loaded settings: {current:?}");
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Settings {
        self.lock().clone()
    }

    pub fn set(&self, settings: Settings) -> std::io::Result<()> {
        write_private(&self.path, &serde_json::to_vec_pretty(&settings)?)?;
        *self.lock() = settings;
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Settings> {
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Writes through a temporary file so a crash never leaves half a file, and
/// keeps it readable by the user only: it holds API keys.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_files_fill_in_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"provider":"openai"}"#).unwrap();
        assert_eq!(settings.provider, "openai");
        assert_eq!(settings.language, "auto");
        assert!(settings.sounds);
    }

    #[test]
    fn debug_output_hides_api_keys() {
        let mut settings = Settings::default();
        settings
            .api_keys
            .insert("groq".to_string(), "gsk_secret".to_string());
        let debug = format!("{settings:?}");
        assert!(!debug.contains("gsk_secret"));
        assert!(debug.contains("groq"));
    }
}
