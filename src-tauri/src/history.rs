//! The last few transcripts, so one that landed in the wrong window can be
//! copied again.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
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

pub struct History {
    path: PathBuf,
    entries: Mutex<Vec<Entry>>,
}

impl History {
    pub fn load(path: PathBuf) -> Self {
        let entries = fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Self {
            path,
            entries: Mutex::new(entries),
        }
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
        let written = serde_json::to_vec(entries)
            .map_err(std::io::Error::other)
            .and_then(|json| crate::settings::write_private(&self.path, &json));
        if let Err(e) = written {
            log::warn!("Could not save the history: {e}");
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Entry>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Tells the main window the list changed.
pub fn notify(app: &AppHandle) {
    let entries = app.state::<History>().entries();
    let _ = app.emit_to("main", "parler://history", entries);
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
}
