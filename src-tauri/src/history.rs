//! The last few transcripts, so one that landed in the wrong window can be
//! copied again, and lifetime totals for the home screen.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const LIMIT: usize = 50;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Creation time in Unix milliseconds; doubles as the id.
    pub id: u64,
    pub text: String,
    /// Length of the speech that was sent.
    pub duration_ms: u64,
    pub provider: String,
}

/// Totals since the first dictation. Clearing the history keeps them.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Stats {
    pub dictations: u64,
    pub words: u64,
    pub speech_ms: u64,
}

pub struct History {
    path: PathBuf,
    stats_path: PathBuf,
    entries: Mutex<Vec<Entry>>,
    stats: Mutex<Stats>,
}

fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    let written = serde_json::to_vec(value)
        .map_err(std::io::Error::other)
        .and_then(|json| crate::settings::write_private(path, &json));
    if let Err(e) = written {
        log::warn!("Could not save {}: {e}", path.display());
    }
}

impl History {
    pub fn load(path: PathBuf) -> Self {
        let stats_path = path.with_file_name("stats.json");
        Self {
            entries: Mutex::new(read_json(&path)),
            stats: Mutex::new(read_json(&stats_path)),
            path,
            stats_path,
        }
    }

    pub fn stats(&self) -> Stats {
        *self.stats.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Newest first.
    pub fn entries(&self) -> Vec<Entry> {
        self.lock().clone()
    }

    pub fn add(&self, text: String, duration_ms: u64, provider: String) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();
        let mut entries = self.lock();
        // Two dictations in one millisecond would share an id.
        let id = entries.first().map_or(now, |last| now.max(last.id + 1));
        entries.insert(
            0,
            Entry {
                id,
                text,
                duration_ms,
                provider,
            },
        );
        entries.truncate(LIMIT);
        self.save(&entries);

        let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
        stats.dictations += 1;
        stats.words += entries[0].text.split_whitespace().count() as u64;
        stats.speech_ms += duration_ms;
        write_json(&self.stats_path, &*stats);
    }

    pub fn remove(&self, id: u64) {
        let mut entries = self.lock();
        entries.retain(|e| e.id != id);
        self.save(&entries);
    }

    pub fn clear(&self) {
        let mut entries = self.lock();
        entries.clear();
        self.save(&entries);
    }

    fn save(&self, entries: &[Entry]) {
        write_json(&self.path, &entries);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Entry>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Tells the main window the list or the totals changed.
pub fn notify(app: &AppHandle) {
    let history = app.state::<History>();
    let _ = app.emit_to("main", "parler://history", history.entries());
    let _ = app.emit_to("main", "parler://stats", history.stats());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest_entries_first_and_bounded() {
        let dir = std::env::temp_dir().join(format!("parler-history-{}", std::process::id()));
        let history = History::load(dir.join("history.json"));
        for i in 0..(LIMIT + 5) {
            history.add(format!("entry {i}"), 1000, "Groq".into());
        }
        let entries = history.entries();
        assert_eq!(entries.len(), LIMIT);
        assert_eq!(entries[0].text, format!("entry {}", LIMIT + 4));
        assert!(entries.windows(2).all(|w| w[0].id > w[1].id));

        let reloaded = History::load(dir.join("history.json"));
        assert_eq!(reloaded.entries(), entries);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn totals_survive_clearing_the_history() {
        let dir = std::env::temp_dir().join(format!("parler-stats-{}", std::process::id()));
        let history = History::load(dir.join("history.json"));
        history.add("bonjour tout le monde".into(), 1500, "Groq".into());
        history.add("à demain".into(), 800, "Groq".into());
        history.clear();

        let reloaded = History::load(dir.join("history.json"));
        assert!(reloaded.entries().is_empty());
        assert_eq!(
            reloaded.stats(),
            Stats {
                dictations: 2,
                words: 6,
                speech_ms: 2300,
            }
        );
        let _ = fs::remove_dir_all(dir);
    }
}
