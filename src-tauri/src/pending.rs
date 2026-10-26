//! Dictations that couldn't be sent, kept on this computer until they are.
//! Parler transcribes in the cloud only: without this, a dropped network or
//! quitting Parler would lose minutes of speech.
//!
//! Each one is two files under `<app data>/pending/`, readable by the user
//! only: its speech as WAV, and a few facts in JSON (when, how long, why it
//! failed). The audio goes nowhere but to the service it is sent to. A
//! dictation leaves as soon as it is transcribed or deleted, and after 7
//! days at most; past 20 of them or 50 MB, the oldest go first.

use crate::providers::TranscribeError;
use crate::settings::write_private;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// How much is kept, at most.
#[derive(Clone, Copy)]
struct Limits {
    age_ms: u64,
    count: usize,
    bytes: u64,
}

const LIMITS: Limits = Limits {
    age_ms: 7 * DAY_MS,
    count: 20,
    // About 26 minutes of speech.
    bytes: 50 * 1024 * 1024,
};

/// A dictation that couldn't be sent, as the window shows it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Unsent {
    /// When it was recorded, in Unix milliseconds; doubles as the id and
    /// the name of its files.
    pub id: u64,
    /// Length of the speech kept.
    pub duration_ms: u64,
    /// Why the last attempt failed.
    pub error: TranscribeError,
}

struct Kept {
    unsent: Unsent,
    bytes: u64,
    /// The audio itself when the disk refused it, kept until Parler quits.
    in_memory: Option<Vec<u8>>,
}

pub struct Pending {
    dir: PathBuf,
    limits: Limits,
    /// Newest first.
    kept: Mutex<Vec<Kept>>,
}

impl Pending {
    /// Picks up what an earlier run left in `dir`.
    pub fn load(dir: PathBuf) -> Self {
        Self::load_with(dir, LIMITS)
    }

    fn load_with(dir: PathBuf, limits: Limits) -> Self {
        let mut kept = read_dir(&dir);
        kept.sort_by_key(|k| std::cmp::Reverse(k.unsent.id));
        let pending = Self {
            dir,
            limits,
            kept: Mutex::new(kept),
        };
        pending.prune(now());
        let count = pending.lock().len();
        if count > 0 {
            log::info!("{count} unsent dictations kept from before");
        }
        pending
    }

    /// Newest first, without those past their time.
    pub fn list(&self) -> Vec<Unsent> {
        self.prune(now());
        self.lock().iter().map(|k| k.unsent.clone()).collect()
    }

    pub fn contains(&self, id: u64) -> bool {
        self.lock().iter().any(|k| k.unsent.id == id)
    }

    /// Keeps a dictation that couldn't be sent; its id.
    pub fn save(&self, wav: Vec<u8>, duration_ms: u64, error: TranscribeError) -> u64 {
        self.save_at(now(), wav, duration_ms, error)
    }

    fn save_at(&self, at: u64, wav: Vec<u8>, duration_ms: u64, error: TranscribeError) -> u64 {
        let mut kept = self.lock();
        // Two in one millisecond would share an id.
        let id = kept
            .first()
            .map_or(at, |newest| at.max(newest.unsent.id + 1));
        let unsent = Unsent {
            id,
            duration_ms,
            error,
        };
        let bytes = wav.len() as u64;
        let in_memory = match self.write(&unsent, &wav) {
            Ok(()) => None,
            Err(e) => {
                log::warn!(
                    "Could not keep the unsent dictation on disk, only until Parler quits: {e}"
                );
                self.delete_files(id);
                Some(wav)
            }
        };
        kept.insert(
            0,
            Kept {
                unsent,
                bytes,
                in_memory,
            },
        );
        drop(kept);
        self.prune(at);
        id
    }

    /// A dictation and its audio, to send again.
    pub fn get(&self, id: u64) -> Option<(Unsent, Vec<u8>)> {
        let kept = self.lock();
        let item = kept.iter().find(|k| k.unsent.id == id)?;
        let wav = match &item.in_memory {
            Some(wav) => wav.clone(),
            None => fs::read(self.wav_path(id))
                .inspect_err(|e| log::warn!("Could not read the unsent dictation: {e}"))
                .ok()?,
        };
        Some((item.unsent.clone(), wav))
    }

    /// Sent again and failed again: it stays, with the new reason. False
    /// when it was deleted meanwhile.
    pub fn failed_again(&self, id: u64, error: &TranscribeError) -> bool {
        let mut kept = self.lock();
        let Some(item) = kept.iter_mut().find(|k| k.unsent.id == id) else {
            return false;
        };
        item.unsent.error = error.clone();
        if item.in_memory.is_none() {
            if let Err(e) = self.write_facts(&item.unsent) {
                log::warn!("Could not update the unsent dictation: {e}");
            }
        }
        true
    }

    /// Forgets a dictation, sent or deleted. False when it was gone already.
    pub fn remove(&self, id: u64) -> bool {
        let mut kept = self.lock();
        let before = kept.len();
        kept.retain(|k| k.unsent.id != id);
        let removed = kept.len() < before;
        if removed {
            self.delete_files(id);
        }
        removed
    }

    /// Drops what is past 7 days, then the oldest beyond the count and size
    /// limits. The newest always stays, however long it is.
    fn prune(&self, now: u64) {
        let mut kept = self.lock();
        let mut total = 0;
        let mut index = 0;
        kept.retain(|item| {
            total += item.bytes;
            let fits = index == 0 || (index < self.limits.count && total <= self.limits.bytes);
            let fresh = now.saturating_sub(item.unsent.id) <= self.limits.age_ms;
            index += 1;
            if !(fits && fresh) {
                log::info!("Dropping an unsent dictation from {}", item.unsent.id);
                self.delete_files(item.unsent.id);
            }
            fits && fresh
        });
    }

    /// The audio first: a JSON file on disk always has its audio.
    fn write(&self, unsent: &Unsent, wav: &[u8]) -> io::Result<()> {
        make_private_dir(&self.dir)?;
        write_private(&self.wav_path(unsent.id), wav)?;
        self.write_facts(unsent)
    }

    fn write_facts(&self, unsent: &Unsent) -> io::Result<()> {
        write_private(&self.json_path(unsent.id), &serde_json::to_vec(unsent)?)
    }

    fn delete_files(&self, id: u64) {
        for path in [self.wav_path(id), self.json_path(id)] {
            match fs::remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => {
                    log::warn!("Could not delete {}: {e}", path.display())
                }
                _ => {}
            }
        }
    }

    fn wav_path(&self, id: u64) -> PathBuf {
        self.dir.join(format!("{id}.wav"))
    }

    fn json_path(&self, id: u64) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Kept>> {
        self.kept.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// What an earlier run left: each JSON file with its audio. Leftovers of a
/// write cut short (audio without its JSON, temporary files) are deleted.
fn read_dir(dir: &Path) -> Vec<Kept> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    let kept: Vec<Kept> = paths
        .iter()
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .filter_map(|path| {
            let unsent: Unsent = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
            let bytes = fs::metadata(dir.join(format!("{}.wav", unsent.id)))
                .ok()?
                .len();
            Some(Kept {
                unsent,
                bytes,
                in_memory: None,
            })
        })
        .collect();
    let ids: HashSet<String> = kept.iter().map(|k| k.unsent.id.to_string()).collect();
    for path in paths {
        let ours = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("wav" | "json" | "tmp")
        );
        let id = path
            .file_name()
            .and_then(|name| name.to_str()?.split('.').next());
        if ours && !id.is_some_and(|id| ids.contains(id)) {
            let _ = fs::remove_file(&path);
        }
    }
    kept
}

/// The folder, readable by the user only like the files in it.
fn make_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offline() -> TranscribeError {
        TranscribeError::Offline {
            provider: "Groq".into(),
        }
    }

    /// An empty folder of its own for each test.
    fn folder(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("parler-pending-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn keeps_dictations_across_a_restart_until_deleted() {
        let dir = folder("restart");
        let pending = Pending::load(dir.clone());
        let first = pending.save(vec![1, 2, 3], 1200, offline());
        let second = pending.save(vec![4, 5], 800, offline());
        assert!(second > first);

        let reloaded = Pending::load(dir.clone());
        assert_eq!(reloaded.list(), pending.list());
        assert_eq!(reloaded.list()[0].id, second);
        let (unsent, wav) = reloaded.get(first).unwrap();
        assert_eq!((unsent.duration_ms, wav), (1200, vec![1, 2, 3]));

        assert!(reloaded.remove(first));
        assert!(!reloaded.remove(first));
        assert!(reloaded.get(first).is_none());
        assert_eq!(
            files(&dir),
            [format!("{second}.json"), format!("{second}.wav")]
        );
        assert_eq!(Pending::load(dir.clone()).list().len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_second_failure_keeps_it_with_the_new_reason() {
        let dir = folder("again");
        let pending = Pending::load(dir.clone());
        let id = pending.save(vec![0; 10], 500, offline());
        let timeout = TranscribeError::Timeout {
            provider: "Groq".into(),
        };
        assert!(pending.failed_again(id, &timeout));
        assert_eq!(Pending::load(dir.clone()).list()[0].error, timeout);
        pending.remove(id);
        assert!(!pending.failed_again(id, &timeout));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn drops_dictations_older_than_a_week() {
        let dir = folder("age");
        let pending = Pending::load(dir.clone());
        let old = pending.save_at(now() - 8 * DAY_MS, vec![1], 100, offline());
        // A save prunes as of its own time: a day later the old one stays.
        pending.save_at(now() - 6 * DAY_MS, vec![2], 100, offline());
        assert!(pending.contains(old));

        let recent = pending.list();
        assert_eq!(recent.len(), 1);
        assert!(!pending.contains(old));
        assert!(!files(&dir).contains(&format!("{old}.wav")));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn beyond_the_limits_the_oldest_go_first_but_never_the_newest() {
        let dir = folder("limits");
        let limits = Limits {
            age_ms: 7 * DAY_MS,
            count: 3,
            bytes: 250,
        };
        let pending = Pending::load_with(dir.clone(), limits);
        let ids: Vec<u64> = (0..5)
            .map(|_| pending.save(vec![0; 10], 100, offline()))
            .collect();
        let kept: Vec<u64> = pending.list().iter().map(|u| u.id).collect();
        assert_eq!(kept, [ids[4], ids[3], ids[2]]);

        // 100 + 100 fit in 250 bytes; a third doesn't.
        for _ in 0..3 {
            pending.save(vec![0; 100], 100, offline());
        }
        assert_eq!(pending.list().len(), 2);

        // Longer than the limit on its own, the newest still stays.
        let long = pending.save(vec![0; 1000], 100, offline());
        let kept: Vec<u64> = pending.list().iter().map(|u| u.id).collect();
        assert_eq!(kept, [long]);
        assert_eq!(files(&dir).len(), 2);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn leftovers_of_an_interrupted_write_are_cleaned_up() {
        let dir = folder("leftovers");
        let pending = Pending::load(dir.clone());
        let id = pending.save(vec![1, 2], 100, offline());
        fs::write(dir.join("1.wav"), [0]).unwrap();
        fs::write(dir.join("2.json"), b"{}").unwrap();
        fs::write(dir.join("3.json.tmp"), b"{").unwrap();

        let reloaded = Pending::load(dir.clone());
        assert_eq!(reloaded.list().len(), 1);
        assert_eq!(files(&dir), [format!("{id}.json"), format!("{id}.wav")]);
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn only_the_user_can_read_them() {
        use std::os::unix::fs::PermissionsExt;
        let dir = folder("private");
        let pending = Pending::load(dir.clone());
        let id = pending.save(vec![1], 100, offline());
        let mode = |path: PathBuf| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(dir.clone()), 0o700);
        assert_eq!(mode(dir.join(format!("{id}.wav"))), 0o600);
        assert_eq!(mode(dir.join(format!("{id}.json"))), 0o600);
        let _ = fs::remove_dir_all(dir);
    }
}
