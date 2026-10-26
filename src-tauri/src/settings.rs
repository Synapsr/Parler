//! User settings, kept as one JSON file in the app's config directory.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::{self, Write};
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

/// fn alone on macOS, like the dictation key Apple prints on it; a settings
/// file always names its shortcut, so this never changes an existing setup.
pub fn default_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "Fn"
    } else {
        "Control+Space"
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarded: false,
            // A first launch suggests Parler Cloud; a settings file always
            // names its provider, so this never changes an existing setup.
            provider: crate::cloud::ID.to_string(),
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
    /// defaults rather than failing the launch; an unreadable one is moved
    /// aside first, so the next save can't erase the keys it may still hold.
    pub fn load(path: PathBuf) -> Self {
        let current = read_or_set_aside(&path).unwrap_or_default();
        log::info!("Loaded settings: {current:?}");
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Settings {
        self.lock().clone()
    }

    pub fn set(&self, settings: Settings) -> io::Result<()> {
        // Held while writing, so two saves never cross on disk and the file
        // always matches what `get` returns.
        let mut current = self.lock();
        write_private(&self.path, &serde_json::to_vec_pretty(&settings)?)?;
        *current = settings;
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Settings> {
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The JSON file at `path`, or `None` when there is none. One that can't be
/// read or parsed is moved aside (`settings.json.broken`) instead of being
/// left for the next save to replace, so what it holds can be recovered.
pub fn read_or_set_aside<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let name = file_name(path);
    let problem = match fs::read_to_string(path) {
        Ok(json) => match serde_json::from_str(&json) {
            Ok(value) => return Some(value),
            // Where, not what: the file holds API keys.
            Err(e) => format!(
                "{:?} error at line {}, column {}",
                e.classify(),
                e.line(),
                e.column()
            ),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => e.to_string(),
    };
    match set_aside(path) {
        Ok(aside) => log::warn!(
            "{name} is unreadable ({problem}): kept as {}, starting from the defaults",
            file_name(&aside)
        ),
        Err(e) => log::error!(
            "{name} is unreadable ({problem}) and couldn't be moved aside ({e}): starting from the defaults"
        ),
    }
    None
}

/// Renames the file to the first free `<name>.broken`, `<name>.broken-2`…,
/// so a later failure never replaces an earlier one.
fn set_aside(path: &Path) -> io::Result<PathBuf> {
    let name = file_name(path);
    for n in 1..100 {
        let suffix = if n == 1 {
            ".broken".to_string()
        } else {
            format!(".broken-{n}")
        };
        let aside = path.with_file_name(format!("{name}{suffix}"));
        if !aside.exists() {
            fs::rename(path, &aside)?;
            return Ok(aside);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many broken copies already",
    ))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Writes through a temporary file, flushed to disk before it replaces the
/// old one, so a crash or a power cut leaves the old file or the new one,
/// never half of either. Readable by the user only: it holds API keys.
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent();
    if let Some(dir) = dir {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Private from the start, not only once written.
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // The mode only applies to a new file; one left by a crash keeps its
        // own.
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)?;
    // The rename itself survives a power cut once the directory is flushed.
    #[cfg(unix)]
    if let Some(dir) = dir {
        if let Ok(dir) = fs::File::open(dir) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
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
    fn a_first_launch_on_macos_dictates_with_fn() {
        let expected = if cfg!(target_os = "macos") {
            "Fn"
        } else {
            "Control+Space"
        };
        assert_eq!(Settings::default().shortcut, expected);
        let existing: Settings = serde_json::from_str(r#"{"shortcut":"Alt+Space"}"#).unwrap();
        assert_eq!(existing.shortcut, "Alt+Space");
    }

    #[test]
    fn a_first_launch_suggests_parler_cloud_but_keeps_existing_choices() {
        assert_eq!(Settings::default().provider, crate::cloud::ID);
        let existing: Settings = serde_json::from_str(r#"{"provider":"groq"}"#).unwrap();
        assert_eq!(existing.provider, "groq");
    }

    /// A fresh directory for one test.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("parler-settings-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_unreadable_file_is_set_aside_rather_than_overwritten() {
        let dir = scratch("broken");
        let path = dir.join("settings.json");
        let cut = r#"{"provider":"groq","apiKeys":{"groq":"gsk_kept"},"shortc"#;
        fs::write(&path, cut).unwrap();
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.get(), Settings::default());
        store.set(store.get()).unwrap();
        let first = dir.join("settings.json.broken");
        assert_eq!(fs::read_to_string(&first).unwrap(), cut);

        // A second failure, here not even text, keeps the first one.
        fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(SettingsStore::load(path.clone()).get(), Settings::default());
        assert_eq!(fs::read_to_string(&first).unwrap(), cut);
        assert_eq!(
            fs::read(dir.join("settings.json.broken-2")).unwrap(),
            [0xff, 0xfe, 0x00]
        );
        assert!(!path.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_file_starts_from_the_defaults_and_sets_nothing_aside() {
        let dir = scratch("missing");
        let store = SettingsStore::load(dir.join("settings.json"));
        assert_eq!(store.get(), Settings::default());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saved_settings_load_back_whole_and_private() {
        let dir = scratch("saved");
        let path = dir.join("config").join("settings.json");
        let settings = Settings {
            provider: "groq".to_string(),
            api_keys: HashMap::from([("groq".to_string(), "gsk_secret".to_string())]),
            ..Settings::default()
        };
        let store = SettingsStore::load(path.clone());
        store.set(Settings::default()).unwrap();
        // A temporary file left by a crash, readable by everyone.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, "half").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644)).unwrap();
        }
        store.set(settings.clone()).unwrap();

        assert_eq!(SettingsStore::load(path.clone()).get(), settings);
        let names: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["settings.json"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        fs::remove_dir_all(&dir).unwrap();
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
