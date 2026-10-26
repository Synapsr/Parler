//! Past transcripts, to find and copy again (one that landed in the wrong
//! window, say), and lifetime totals for the home screen.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

/// Thousands of dictations stay well under a megabyte of JSON.
const LIMIT: usize = 5000;

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
    limit: usize,
    entries: Mutex<Vec<Entry>>,
    stats: Mutex<Stats>,
}

/// The file's contents, or empty. A file that can't be read is moved aside
/// rather than replaced by the next save, so no dictation is lost to it.
fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    crate::settings::read_or_set_aside(path).unwrap_or_default()
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
        Self::load_with_limit(path, LIMIT)
    }

    fn load_with_limit(path: PathBuf, limit: usize) -> Self {
        let stats_path = path.with_file_name("stats.json");
        Self {
            entries: Mutex::new(read_json(&path)),
            stats: Mutex::new(read_json(&stats_path)),
            path,
            stats_path,
            limit,
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
        entries.truncate(self.limit);
        self.save(&entries);

        let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
        stats.dictations += 1;
        stats.words += count_words(&entries[0].text);
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
    // The tray's "copy last transcript" depends on there being one.
    crate::tray::refresh(app);
}

/// Words in a dictation. Chinese and Japanese put no spaces between words, so
/// there every two characters count as one word, about their average length.
fn count_words(text: &str) -> u64 {
    let mut words = 0;
    let mut wide = 0;
    for token in text.split_whitespace() {
        wide += token.chars().filter(|&c| is_cjk(c)).count() as u64;
        // Latin words, numbers and the like count once ("Parler", "14:00").
        if token.chars().any(|c| c.is_alphanumeric() && !is_cjk(c)) {
            words += 1;
        }
    }
    words + wide.div_ceil(2)
}

/// Kana and Han characters (Korean separates its words with spaces).
fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF)
}

/// A character of a script written without spaces between sentences: Han,
/// kana and Thai.
pub(crate) fn is_unspaced(c: char) -> bool {
    is_cjk(c) || matches!(c as u32, 0x0E00..=0x0E7F)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn an_unreadable_history_is_kept_aside_not_overwritten() {
        let dir =
            std::env::temp_dir().join(format!("parler-history-broken-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("history.json");
        fs::write(&path, "[{\"id\": 1, \"text\": \"cut off").unwrap();
        let history = History::load(path.clone());
        assert!(history.entries().is_empty());
        history.add("after".into(), 1000, "Groq".into());
        assert!(fs::read_to_string(dir.join("history.json.broken"))
            .unwrap()
            .contains("cut off"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn keeps_the_newest_entries_first_and_bounded() {
        let dir = std::env::temp_dir().join(format!("parler-history-{}", std::process::id()));
        let limit = 20;
        let history = History::load_with_limit(dir.join("history.json"), limit);
        for i in 0..(limit + 5) {
            history.add(format!("entry {i}"), 1000, "Groq".into());
        }
        let entries = history.entries();
        assert_eq!(entries.len(), limit);
        assert_eq!(entries[0].text, format!("entry {}", limit + 4));
        assert!(entries.windows(2).all(|w| w[0].id > w[1].id));

        let reloaded = History::load(dir.join("history.json"));
        assert_eq!(reloaded.entries(), entries);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn counts_words_without_spaces_too() {
        assert_eq!(count_words("Bonjour Camille, à demain !"), 4);
        assert_eq!(count_words("谢谢你的反馈，我明天上午看一下。"), 7);
        assert_eq!(count_words("Camille 你好"), 2);
        assert_eq!(count_words("피드백 고마워요."), 2);
        assert_eq!(count_words(""), 0);
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
