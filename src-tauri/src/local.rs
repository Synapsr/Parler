//! Transcription on this computer: a speech model downloaded once, then run
//! by [transcribe.cpp](https://github.com/handy-computer/transcribe.cpp).
//! The audio never leaves the computer.
//!
//! - **Two models**, pinned to one revision on Hugging Face and checked
//!   against their SHA-256 before use. They live in
//!   `<app data>/models/<file>`. A download goes to `<file>.part`, picks up
//!   where it stopped (HTTP Range), and is renamed once verified, so a file
//!   under its own name is always whole. Whether a model is ready is read
//!   from that folder, never stored.
//! - **One model loaded at a time**, kept between dictations. It loads in
//!   the background as soon as `local` is the provider, or a dictation
//!   starts, so nobody waits for it after speaking; it is let go after 15
//!   minutes unused or when another provider is picked. One transcription
//!   runs at a time, on a blocking thread.
//! - **The GPU when there is one**: Metal on Apple Silicon, Vulkan on Windows
//!   and Linux; the CPU otherwise, and on Intel Macs. A GPU that fails to
//!   load the model, or to run it, makes way for the CPU. One that took
//!   Parler down (ggml ends the process on some driver faults) is not tried
//!   again by this version: a marker file is written around each GPU load
//!   and run, and a launch that finds it turns the GPU off.

use crate::providers::TranscribeError;
use crate::settings::{Settings, SettingsStore};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use transcribe_cpp::{Backend as Compute, DeviceType, Feature, ModelOptions, RunOptions, Session};

pub const ID: &str = "local";
/// The provider's name in logs and the history; the window shows its own.
pub const LABEL: &str = "Parler (local)";
pub const PARAKEET: &str = "parakeet-ultra";
pub const WHISPER: &str = "whisper-large-v3-turbo";

/// Where the models live, in the app's data directory.
const MODELS_DIR: &str = "models";
/// Written before each GPU load or run and removed after: found at launch,
/// the GPU took Parler down.
const GPU_RUNNING: &str = "local-gpu-running";
/// The GPU is off for the version written in it, after a crash.
const GPU_OFF: &str = "local-gpu-off";
/// A model unused this long is let go, freeing its memory.
const IDLE_UNLOAD: Duration = Duration::from_secs(15 * 60);
const IDLE_CHECK: Duration = Duration::from_secs(30);
/// Progress events, at most this often (like updates.rs).
const PROGRESS_EVERY: Duration = Duration::from_millis(120);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// No byte for this long and the download is given up, keeping what came.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// What a dictation kept before the engine runs says, should Parler stop
/// before it is transcribed.
const STOPPED_WHILE_TRANSCRIBING: &str = "Parler stopped while transcribing";

/// A speech model Parler can download.
#[derive(Debug)]
pub struct Model {
    pub id: &'static str,
    pub name: &'static str,
    file: &'static str,
    url: &'static str,
    pub size: u64,
    sha256: &'static str,
    /// ISO 639-1 codes; `None` for every language Whisper knows.
    pub languages: Option<&'static [&'static str]>,
    /// Takes the vocabulary as a spelling hint.
    pub supports_vocabulary: bool,
}

/// From `handy-computer` on Hugging Face, at a fixed revision. Languages and
/// hashes as in Handy's catalog.
pub const MODELS: &[Model] = &[
    Model {
        id: PARAKEET,
        name: "Parakeet Ultra",
        file: "parakeet-ultra-Q8_0.gguf",
        url: "https://huggingface.co/handy-computer/parakeet-ultra-gguf/resolve/39eeb55181f0d354fd934f06e92fd8d5037fed8e/parakeet-ultra-Q8_0.gguf",
        size: 740_363_168,
        sha256: "007a59761e9258779f189df396b50d12b83bb5b79e66df4b955c230d2f2a0a59",
        languages: Some(&[
            "en", "de", "fr", "es", "it", "pt", "ru", "uk", "hr", "sl", "lv", "lt", "et", "fi",
            "sv", "da", "nl", "pl", "cs", "sk", "hu", "ro", "bg", "el", "mt",
        ]),
        supports_vocabulary: false,
    },
    Model {
        id: WHISPER,
        name: "Whisper Large v3 Turbo",
        file: "whisper-large-v3-turbo-Q5_K_M.gguf",
        url: "https://huggingface.co/handy-computer/whisper-large-v3-turbo-gguf/resolve/5eaf945c7978e564bae5b28a5b1639dd93c2bfb1/whisper-large-v3-turbo-Q5_K_M.gguf",
        size: 619_628_128,
        sha256: "977b5db4e004349dffd1ab9caa10ba5aaba3fc3edd3ba72cadb84328a3203e36",
        languages: None,
        supports_vocabulary: true,
    },
];

pub fn find(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.id == id)
}

fn parakeet() -> &'static Model {
    &MODELS[0]
}

fn whisper() -> &'static Model {
    &MODELS[1]
}

/// The model these settings use: the one picked, else the recommended one.
pub fn chosen(settings: &Settings) -> &'static Model {
    settings
        .models
        .get(ID)
        .and_then(|id| find(id))
        .unwrap_or_else(|| recommended(settings))
}

/// Parakeet Ultra, the more accurate, when it knows the language people
/// speak; Whisper otherwise.
pub fn recommended(settings: &Settings) -> &'static Model {
    recommended_for(&spoken_language(settings, sys_locale::get_locale()))
}

fn recommended_for(language: &str) -> &'static Model {
    let parakeet = parakeet();
    let known = parakeet
        .languages
        .is_some_and(|languages| languages.contains(&language));
    // Nothing to go by: most likely a European language, Parakeet's.
    if known || language.is_empty() {
        parakeet
    } else {
        whisper()
    }
}

/// The language people are expected to speak: the one set for dictation,
/// else the interface's, else the system's (which the interface follows).
fn spoken_language(settings: &Settings, system: Option<String>) -> String {
    [settings.language.as_str(), settings.ui_language.as_str()]
        .into_iter()
        .map(base_language)
        .find(|code| !code.is_empty() && code != "auto")
        .or_else(|| system.as_deref().map(base_language))
        .unwrap_or_default()
}

/// "pt-BR" → "pt".
fn base_language(code: &str) -> String {
    code.trim()
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// A model as the window shows it.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub id: &'static str,
    pub name: &'static str,
    pub size_bytes: u64,
    pub languages: Option<&'static [&'static str]>,
    pub recommended: bool,
    pub supports_vocabulary: bool,
    pub state: ModelState,
    /// Where it runs, while it is the model loaded: "Metal · Apple M4",
    /// "Vulkan · <GPU>", "CPU".
    pub backend: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ModelState {
    Missing,
    Downloading { downloaded: u64, total: u64 },
    Verifying,
    Ready,
    Failed { error: DownloadError },
}

/// Why a download stopped, in terms the window can explain.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadError {
    pub kind: DownloadErrorKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadErrorKind {
    /// No connection, or it dropped: trying again resumes.
    Offline,
    /// The disk is full.
    Disk,
    /// What arrived isn't the model; it was thrown away.
    Checksum,
    Other,
}

impl DownloadError {
    fn new(kind: DownloadErrorKind, detail: Option<String>) -> Self {
        Self { kind, detail }
    }

    fn other(detail: impl Into<String>) -> Self {
        Self::new(DownloadErrorKind::Other, Some(detail.into()))
    }

    fn offline(error: impl std::fmt::Display) -> Self {
        log::warn!("Model download interrupted: {error}");
        Self::new(DownloadErrorKind::Offline, None)
    }

    /// A full disk says so; any other file error is shown as it is.
    fn io(error: io::Error) -> Self {
        let kind = match error.kind() {
            io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => DownloadErrorKind::Disk,
            _ => DownloadErrorKind::Other,
        };
        Self::new(kind, Some(error.to_string()))
    }
}

/// The compute device that runs (or will run) the model, for Settings and a
/// report to support.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Backend {
    /// "metal", "vulkan" or "cpu".
    pub kind: String,
    /// The GPU's or processor's name, such as "Apple M4".
    pub device: String,
}

impl Backend {
    fn of(device: &transcribe_cpp::Device) -> Self {
        Self {
            kind: device.kind.clone(),
            device: device.description.trim().to_string(),
        }
    }

    fn on_gpu(&self) -> bool {
        self.kind != "cpu" && self.kind != "accel"
    }

    fn label(&self) -> String {
        match self.kind.as_str() {
            "metal" => format!("Metal · {}", self.device),
            "vulkan" => format!("Vulkan · {}", self.device),
            _ => "CPU".to_string(),
        }
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.kind, self.device)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------
// Downloads

/// Where a download stands.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Progress {
    Bytes { downloaded: u64, total: u64 },
    Verifying,
}

/// What to fetch and how to know it's right.
pub(crate) struct Source<'a> {
    url: &'a str,
    size: u64,
    sha256: &'a str,
}

impl Model {
    fn source(&self) -> Source<'static> {
        Source {
            url: self.url,
            size: self.size,
            sha256: self.sha256,
        }
    }
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

enum Received {
    Whole,
    /// The server can't continue where the part stops: start again.
    FromTheStart,
}

/// Downloads `source` to `dest`: into `<dest>.part`, from where an earlier
/// try stopped, then checked and renamed. Cancelling is dropping the future:
/// what came stays in the part for next time.
async fn fetch(
    source: &Source<'_>,
    dest: &Path,
    progress: &mut (dyn FnMut(Progress) + Send),
) -> Result<(), DownloadError> {
    let part = part_path(dest);
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).map_err(DownloadError::io)?;
    }
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .user_agent(concat!(
            "Parler/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/Synapsr/Parler)"
        ))
        .build()
        .map_err(|e| DownloadError::other(e.to_string()))?;

    // Once more from the start at most, when the server can't resume.
    for _ in 0..2 {
        let mut have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if have > source.size {
            remove_if_there(&part);
            have = 0;
        }
        if have < source.size {
            if let Received::FromTheStart = receive(&client, source, &part, have, progress).await? {
                remove_if_there(&part);
                continue;
            }
        }
        progress(Progress::Verifying);
        let (path, expected) = (part.clone(), source.sha256.to_string());
        let hash = tauri::async_runtime::spawn_blocking(move || sha256_of(&path))
            .await
            .map_err(|e| DownloadError::other(e.to_string()))?
            .map_err(DownloadError::io)?;
        if hash != expected {
            log::warn!("Downloaded model has SHA-256 {hash}, expected {expected}");
            remove_if_there(&part);
            return Err(DownloadError::new(DownloadErrorKind::Checksum, None));
        }
        fs::rename(&part, dest).map_err(DownloadError::io)?;
        return Ok(());
    }
    Err(DownloadError::other(
        "the server could not resume the download",
    ))
}

/// One request: the rest of the file after the `have` bytes already in the
/// part, appended to it.
async fn receive(
    client: &reqwest::Client,
    source: &Source<'_>,
    part: &Path,
    have: u64,
    progress: &mut (dyn FnMut(Progress) + Send),
) -> Result<Received, DownloadError> {
    let mut request = client.get(source.url);
    if have > 0 {
        log::info!("Resuming the model download at {} MB", have / 1_000_000);
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let mut response = request.send().await.map_err(DownloadError::offline)?;
    let status = response.status();
    let mut have = have;
    if have > 0 {
        match status {
            reqwest::StatusCode::PARTIAL_CONTENT => {
                let start = response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok())
                    .and_then(content_range_start);
                if start != Some(have) {
                    log::warn!("The server resumed at {start:?}, not {have}: starting over");
                    return Ok(Received::FromTheStart);
                }
            }
            reqwest::StatusCode::RANGE_NOT_SATISFIABLE => return Ok(Received::FromTheStart),
            // The whole file again: the Range was ignored.
            status if status.is_success() => {
                log::info!("The server sends the whole model again");
                have = 0;
            }
            _ => {}
        }
    }
    if !status.is_success() {
        log::warn!("Model download refused: HTTP {status}");
        return Err(DownloadError::other(format!("HTTP {}", status.as_u16())));
    }

    let mut options = fs::OpenOptions::new();
    if have > 0 {
        options.append(true);
    } else {
        options.write(true).create(true).truncate(true);
    }
    let mut file = options.open(part).map_err(DownloadError::io)?;
    progress(Progress::Bytes {
        downloaded: have,
        total: source.size,
    });
    while let Some(chunk) = response.chunk().await.map_err(DownloadError::offline)? {
        have += chunk.len() as u64;
        if have > source.size {
            drop(file);
            remove_if_there(part);
            return Err(DownloadError::other("the file is larger than expected"));
        }
        file.write_all(&chunk).map_err(DownloadError::io)?;
        progress(Progress::Bytes {
            downloaded: have,
            total: source.size,
        });
    }
    file.sync_all().map_err(DownloadError::io)?;
    if have < source.size {
        return Err(DownloadError::offline("the connection closed early"));
    }
    Ok(Received::Whole)
}

/// `bytes 100-199/200` → 100.
fn content_range_start(value: &str) -> Option<u64> {
    value
        .trim()
        .strip_prefix("bytes")?
        .trim_start()
        .split('-')
        .next()?
        .trim()
        .parse()
        .ok()
}

fn sha256_of(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn remove_if_there(path: &Path) {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => {
            log::warn!("Could not delete {}: {e}", path.display())
        }
        _ => {}
    }
}

/// A model under its own name, at its size: downloaded and verified.
fn on_disk(dir: &Path, model: &Model) -> bool {
    fs::metadata(dir.join(model.file)).is_ok_and(|m| m.len() == model.size)
}

// ---------------------------------------------------------------------------
// The engine

/// Whether this launch may use the GPU, decided once from the marker files.
struct Gpu {
    /// Where the markers go; `None` in tests, which write none.
    dir: Option<PathBuf>,
    allowed: AtomicBool,
}

impl Gpu {
    /// A marker left by the last run means the GPU took Parler down: off
    /// for this version (a newer one may have the fix).
    fn decide(dir: PathBuf) -> Self {
        let version = env!("CARGO_PKG_VERSION");
        let running = dir.join(GPU_RUNNING);
        let off = dir.join(GPU_OFF);
        if running.exists() {
            log::error!("The GPU stopped Parler during a transcription: using the CPU from now on");
            remove_if_there(&running);
            if let Err(e) = fs::write(&off, version) {
                log::warn!("Could not remember to keep off the GPU: {e}");
            }
        }
        let allowed = match fs::read_to_string(&off) {
            Ok(off_for) if off_for.trim() == version => false,
            Ok(_) => {
                log::info!("Trying the GPU again with this version");
                remove_if_there(&off);
                true
            }
            Err(_) => true,
        };
        if !allowed {
            log::info!("The GPU is off for transcription (it crashed before)");
        }
        Self {
            dir: Some(dir),
            allowed: AtomicBool::new(allowed),
        }
    }

    fn allowed(&self) -> bool {
        self.allowed.load(Ordering::Relaxed)
    }

    /// Runs `f`, with the marker on disk meanwhile when the GPU is involved.
    fn guard<T>(&self, on_gpu: bool, f: impl FnOnce() -> T) -> T {
        let marker = self
            .dir
            .as_ref()
            .filter(|_| on_gpu)
            .map(|d| d.join(GPU_RUNNING));
        if let Some(marker) = &marker {
            let _ = fs::write(marker, b"");
        }
        let result = f();
        if let Some(marker) = &marker {
            remove_if_there(marker);
        }
        result
    }
}

/// Registers the compute backends, once per process and before anything
/// else touches them (a dynamic build loads its modules here). With the GPU
/// off, its driver is never even loaded.
fn init_backends(gpu: &Gpu) -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        transcribe_cpp::init_logging();
        let result = if gpu.allowed() {
            gpu.guard(true, transcribe_cpp::init_backends_default)
        } else {
            transcribe_cpp::init_backends_with(
                None::<&Path>,
                transcribe_cpp::BackendMask::for_backend(Compute::Cpu),
            )
        };
        match &result {
            Ok(()) => {
                for device in transcribe_cpp::devices() {
                    log::info!(
                        "Compute device: {} ({}, {:?})",
                        device.description,
                        device.kind,
                        device.device_type
                    );
                }
            }
            Err(e) => log::error!("No compute device for transcription: {e}"),
        }
        result.map_err(|e| e.to_string())
    })
    .clone()
}

/// The device a load would pick: the first GPU, else the CPU.
fn expected_backend(gpu: &Gpu) -> Option<Backend> {
    let devices = transcribe_cpp::devices();
    let is_gpu =
        |d: &&transcribe_cpp::Device| matches!(d.device_type, DeviceType::Gpu | DeviceType::Igpu);
    let pick = if gpu.allowed() {
        devices.iter().find(is_gpu)
    } else {
        None
    };
    pick.or_else(|| devices.iter().find(|d| d.device_type == DeviceType::Cpu))
        .map(Backend::of)
}

struct Loaded {
    id: &'static str,
    path: PathBuf,
    session: Session,
    backend: Backend,
    /// The languages it takes as a hint; empty for any.
    languages: Vec<String>,
    vocabulary: bool,
}

/// What one transcription needs besides the audio.
pub(crate) struct Job {
    pub model: &'static Model,
    pub path: PathBuf,
    /// ISO 639-1, or `None` to let the model tell.
    pub language: Option<String>,
    pub vocabulary: Vec<String>,
}

struct Engine {
    loaded: Option<Loaded>,
    last_used: Instant,
}

fn failed(detail: impl std::fmt::Display) -> TranscribeError {
    TranscribeError::LocalFailed {
        provider: LABEL.to_string(),
        detail: detail.to_string(),
    }
}

fn missing() -> TranscribeError {
    TranscribeError::ModelMissing {
        provider: LABEL.to_string(),
    }
}

fn engine_error(error: transcribe_cpp::Error) -> TranscribeError {
    match error {
        transcribe_cpp::Error::ModelFileNotFound(_) => missing(),
        other => failed(other),
    }
}

impl Engine {
    fn new() -> Self {
        Self {
            loaded: None,
            last_used: Instant::now(),
        }
    }

    fn loaded_id(&self) -> Option<&'static str> {
        self.loaded.as_ref().map(|l| l.id)
    }

    fn unload(&mut self) {
        if let Some(loaded) = self.loaded.take() {
            log::info!("Letting go of the {} model", loaded.id);
        }
    }

    /// `model` loaded from `path`, on the GPU when there is one and it
    /// takes it, else on the CPU.
    fn ensure(
        &mut self,
        gpu: &Gpu,
        model: &'static Model,
        path: &Path,
    ) -> Result<(), TranscribeError> {
        self.last_used = Instant::now();
        if self
            .loaded
            .as_ref()
            .is_some_and(|l| l.id == model.id && l.path == path)
        {
            return Ok(());
        }
        self.unload();
        if !path.exists() {
            return Err(missing());
        }
        init_backends(gpu).map_err(failed)?;
        let started = Instant::now();
        let attempt = if gpu.allowed() {
            match gpu.guard(true, || load(path, Compute::Auto)) {
                Ok(loaded) => Ok(loaded),
                Err(e) => {
                    log::warn!("Could not load the model on the GPU ({e}); using the CPU");
                    gpu.allowed.store(false, Ordering::Relaxed);
                    load(path, Compute::Cpu)
                }
            }
        } else {
            load(path, Compute::Cpu)
        };
        let (handle, session) = attempt.map_err(engine_error)?;
        let backend = handle.device().map(|d| Backend::of(&d)).unwrap_or(Backend {
            kind: "cpu".into(),
            device: String::new(),
        });
        let capabilities = handle.capabilities().map_err(engine_error)?;
        log::info!(
            "Loaded the {} model on {backend} in {:.1}s",
            model.id,
            started.elapsed().as_secs_f64()
        );
        self.loaded = Some(Loaded {
            id: model.id,
            path: path.to_path_buf(),
            languages: capabilities.languages,
            vocabulary: model.supports_vocabulary && handle.supports(Feature::Vocabulary),
            session,
            backend,
        });
        Ok(())
    }

    /// Transcribes 16 kHz mono samples. A run that fails on the GPU is run
    /// again on the CPU.
    fn transcribe(
        &mut self,
        gpu: &Gpu,
        job: &Job,
        samples: &[f32],
    ) -> Result<String, TranscribeError> {
        self.ensure(gpu, job.model, &job.path)?;
        let loaded = self.loaded.as_mut().expect("just loaded");
        let on_gpu = loaded.backend.on_gpu();
        let result = gpu.guard(on_gpu, || run(loaded, job, samples));
        self.last_used = Instant::now();
        match result {
            // The device itself failed, not the request: the CPU may manage.
            Err(
                e @ (transcribe_cpp::Error::Backend(_)
                | transcribe_cpp::Error::OutOfMemory(_)
                | transcribe_cpp::Error::Other(_)),
            ) if on_gpu => {
                log::warn!("Transcription failed on the GPU ({e}); trying the CPU");
                gpu.allowed.store(false, Ordering::Relaxed);
                self.unload();
                self.ensure(gpu, job.model, &job.path)?;
                let loaded = self.loaded.as_mut().expect("just loaded");
                run(loaded, job, samples).map_err(engine_error)
            }
            result => result.map_err(engine_error),
        }
    }
}

fn load(path: &Path, compute: Compute) -> transcribe_cpp::Result<(transcribe_cpp::Model, Session)> {
    let options = ModelOptions {
        backend: compute,
        ..ModelOptions::default()
    };
    let model = transcribe_cpp::Model::load_with(path, &options)?;
    let session = model.session()?;
    Ok((model, session))
}

/// One dictation through the loaded model.
fn run(loaded: &mut Loaded, job: &Job, samples: &[f32]) -> transcribe_cpp::Result<String> {
    let language = job
        .language
        .clone()
        .filter(|l| loaded.languages.is_empty() || loaded.languages.contains(l));
    if language.is_none() && job.language.is_some() {
        log::info!(
            "The model takes no {:?} hint: it detects the language",
            job.language
        );
    }
    let options = RunOptions {
        language,
        vocabulary: if loaded.vocabulary {
            job.vocabulary.clone()
        } else {
            Vec::new()
        },
        ..RunOptions::default()
    };
    // Both models decode long audio themselves (Whisper in 30 s windows,
    // Parakeet cutting at the pauses its voice detector finds).
    let transcript = match loaded.session.run(samples, &options) {
        Ok(transcript) => transcript,
        // What came before the model stopped or started repeating itself
        // is kept, rather than losing the dictation.
        Err(
            transcribe_cpp::Error::OutputTruncated { partial, message }
            | transcribe_cpp::Error::OutputRepetition { partial, message },
        ) if partial.is_some() => {
            log::warn!("Transcript cut short: {message}");
            *partial.expect("checked")
        }
        Err(e) => return Err(e),
    };
    Ok(transcript.text.trim().to_string())
}

// ---------------------------------------------------------------------------
// The app's side: one per process, from launch.

struct Downloading {
    job: u64,
    progress: Progress,
    task: Option<tauri::async_runtime::JoinHandle<()>>,
}

pub struct Local {
    app: AppHandle,
    dir: PathBuf,
    gpu: Gpu,
    downloads: Mutex<HashMap<&'static str, Downloading>>,
    /// The last download's failure, shown until the next try.
    failures: Mutex<HashMap<&'static str, DownloadError>>,
    next_job: AtomicU64,
    engine: Mutex<Engine>,
    /// A background load is on its way; another isn't queued behind it.
    warming: AtomicBool,
    last_progress: Mutex<Instant>,
    /// The model loaded and where, as last seen: read without waiting for
    /// a transcription to finish.
    loaded: Mutex<Option<(&'static str, Backend)>>,
}

static LOCAL: OnceLock<Local> = OnceLock::new();

fn get() -> Option<&'static Local> {
    LOCAL.get()
}

/// Sets up local transcription at launch and lets go of an idle model.
pub fn init(app: &AppHandle, data_dir: PathBuf) {
    let local = Local {
        app: app.clone(),
        dir: data_dir.join(MODELS_DIR),
        gpu: Gpu::decide(data_dir),
        downloads: Mutex::new(HashMap::new()),
        failures: Mutex::new(HashMap::new()),
        next_job: AtomicU64::new(1),
        engine: Mutex::new(Engine::new()),
        warming: AtomicBool::new(false),
        last_progress: Mutex::new(Instant::now()),
        loaded: Mutex::new(None),
    };
    if LOCAL.set(local).is_err() {
        return;
    }
    let spawned = thread::Builder::new()
        .name("local-idle".into())
        .spawn(|| loop {
            thread::sleep(IDLE_CHECK);
            let Some(local) = get() else { continue };
            // Busy: a load or a transcription, so not idle.
            let Ok(mut engine) = local.engine.try_lock() else {
                continue;
            };
            if engine.loaded.is_some() && engine.last_used.elapsed() >= IDLE_UNLOAD {
                log::info!("Unused for {} min", IDLE_UNLOAD.as_secs() / 60);
                engine.unload();
                local.seen(engine);
            }
        });
    if let Err(e) = spawned {
        log::warn!("Could not watch the local model for idleness: {e}");
    }
}

/// The path of a downloaded model, `None` when it isn't ready.
pub fn ready_path(model: &Model) -> Option<PathBuf> {
    let local = get()?;
    on_disk(&local.dir, model).then(|| local.dir.join(model.file))
}

/// The models and where they stand.
pub fn models(settings: &Settings) -> Vec<LocalModel> {
    let recommended = recommended(settings).id;
    let loaded = get().and_then(|local| lock(&local.loaded).clone());
    MODELS
        .iter()
        .map(|model| LocalModel {
            id: model.id,
            name: model.name,
            size_bytes: model.size,
            languages: model.languages,
            recommended: model.id == recommended,
            supports_vocabulary: model.supports_vocabulary,
            state: get().map_or(ModelState::Missing, |local| local.state(model)),
            backend: loaded
                .as_ref()
                .filter(|(id, _)| *id == model.id)
                .map(|(_, backend)| backend.label()),
        })
        .collect()
}

/// After the settings changed (or at launch): the chosen model is loaded
/// in the background when `local` is the provider, let go otherwise.
pub fn settings_changed(settings: &Settings) {
    if settings.provider == ID {
        warm(settings);
    } else if let Some(local) = get() {
        match local.engine.try_lock() {
            Ok(mut engine) => {
                engine.unload();
                local.seen(engine);
            }
            // Busy transcribing: once it's done.
            Err(_) => {
                thread::spawn(move || local.with_engine(Engine::unload));
            }
        }
    }
}

/// Loads the chosen model in the background, so the next dictation finds
/// it ready: when `local` becomes the provider, and when a dictation starts.
pub fn warm(settings: &Settings) {
    let Some(local) = get() else { return };
    let model = chosen(settings);
    let Some(path) = ready_path(model) else {
        return;
    };
    // Busy loading or transcribing: never wait for it here, on the
    // dictation's thread.
    match local.engine.try_lock() {
        Ok(engine) if engine.loaded_id() != Some(model.id) => {}
        _ => return,
    }
    if local.warming.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = thread::Builder::new()
        .name("local-load".into())
        .spawn(move || {
            if let Err(e) = local.with_engine(|engine| engine.ensure(&local.gpu, model, &path)) {
                log::warn!("Could not load the {} model: {e}", model.id);
            }
            local.warming.store(false, Ordering::SeqCst);
        });
    if spawned.is_err() {
        local.warming.store(false, Ordering::SeqCst);
    }
}

/// Transcribes speech on this computer, on a blocking thread.
pub(crate) async fn transcribe(job: Job, samples: Vec<f32>) -> Result<String, TranscribeError> {
    let local = get().ok_or_else(missing)?;
    tauri::async_runtime::spawn_blocking(move || {
        local.with_engine(|engine| engine.transcribe(&local.gpu, &job, &samples))
    })
    .await
    .map_err(|e| failed(format!("the engine stopped: {e}")))?
}

/// The setup check: the chosen model is downloaded and loads (and stays
/// loaded, ready for the first dictation).
pub async fn test(settings: &Settings) -> Result<(), TranscribeError> {
    let local = get().ok_or_else(missing)?;
    let model = chosen(settings);
    let path = ready_path(model).ok_or_else(missing)?;
    tauri::async_runtime::spawn_blocking(move || {
        local.with_engine(|engine| engine.ensure(&local.gpu, model, &path))
    })
    .await
    .map_err(|e| failed(format!("the engine stopped: {e}")))?
}

/// The device that runs the loaded model, or would run the next one.
pub async fn backend() -> Option<Backend> {
    let local = get()?;
    tauri::async_runtime::spawn_blocking(move || local.backend())
        .await
        .ok()
        .flatten()
}

/// Facts for a report to support: the model picked and where it runs.
pub fn report_fields(settings: &Settings) -> Vec<(&'static str, String)> {
    let Some(local) = get() else {
        return Vec::new();
    };
    let model = chosen(settings);
    let mut fields = vec![
        ("localModel", model.id.to_string()),
        ("localModelReady", on_disk(&local.dir, model).to_string()),
    ];
    // Only once loaded: a report never loads a GPU driver.
    if let Some((_, backend)) = lock(&local.loaded).clone() {
        fields.push(("localBackend", backend.to_string()));
    }
    if !local.gpu.allowed() {
        fields.push(("localGpu", "off".to_string()));
    }
    fields
}

/// Starts or resumes a download in the background.
pub fn download(id: &str) -> Result<(), String> {
    let model = find(id).ok_or_else(|| format!("unknown model {id}"))?;
    let local = get().ok_or("local transcription is not set up")?;
    local.download(model);
    Ok(())
}

/// Stops a download, keeping what came for the next try.
pub fn cancel(id: &str) {
    let (Some(model), Some(local)) = (find(id), get()) else {
        return;
    };
    let stopped = lock(&local.downloads).remove(model.id);
    if let Some(task) = stopped.and_then(|d| d.task) {
        task.abort();
        log::info!("Download of the {} model cancelled", model.id);
        local.notify();
    }
}

/// Deletes a model, and any download of it. A model in use is let go
/// first; the provider then reports it missing.
pub async fn delete(id: &str) -> Result<(), String> {
    let model = find(id).ok_or_else(|| format!("unknown model {id}"))?;
    let local = get().ok_or("local transcription is not set up")?;
    cancel(id);
    let result = tauri::async_runtime::spawn_blocking(move || {
        local.with_engine(|engine| {
            if engine.loaded_id() == Some(model.id) {
                engine.unload();
            }
        });
        let path = local.dir.join(model.file);
        remove_if_there(&part_path(&path));
        match fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    lock(&local.failures).remove(model.id);
    log::info!("Deleted the {} model", model.id);
    local.notify();
    result
}

impl Local {
    /// Runs `f` on the engine, then tells the window if another model, or
    /// none, is loaded now.
    fn with_engine<T>(&self, f: impl FnOnce(&mut Engine) -> T) -> T {
        let mut engine = lock(&self.engine);
        let result = f(&mut engine);
        self.seen(engine);
        result
    }

    /// Notes what `engine` has loaded, and lets the window know of a change.
    fn seen(&self, engine: MutexGuard<'_, Engine>) {
        let now = engine.loaded.as_ref().map(|l| (l.id, l.backend.clone()));
        drop(engine);
        let changed = {
            let mut loaded = lock(&self.loaded);
            let changed = *loaded != now;
            *loaded = now;
            changed
        };
        if changed {
            self.notify();
        }
    }

    fn state(&self, model: &Model) -> ModelState {
        if let Some(download) = lock(&self.downloads).get(model.id) {
            return match download.progress {
                Progress::Bytes { downloaded, total } => {
                    ModelState::Downloading { downloaded, total }
                }
                Progress::Verifying => ModelState::Verifying,
            };
        }
        if on_disk(&self.dir, model) {
            return ModelState::Ready;
        }
        match lock(&self.failures).get(model.id) {
            Some(error) => ModelState::Failed {
                error: error.clone(),
            },
            None => ModelState::Missing,
        }
    }

    fn settings(&self) -> Settings {
        self.app.state::<SettingsStore>().get()
    }

    /// Sends the models' states to the window.
    fn notify(&self) {
        *lock(&self.last_progress) = Instant::now();
        let _ = self
            .app
            .emit_to("main", "parler://local-models", models(&self.settings()));
    }

    fn download(&'static self, model: &'static Model) {
        let mut downloads = lock(&self.downloads);
        if downloads.contains_key(model.id) || on_disk(&self.dir, model) {
            return;
        }
        lock(&self.failures).remove(model.id);
        let job = self.next_job.fetch_add(1, Ordering::Relaxed);
        let part = part_path(&self.dir.join(model.file));
        downloads.insert(
            model.id,
            Downloading {
                job,
                progress: Progress::Bytes {
                    downloaded: fs::metadata(part).map(|m| m.len()).unwrap_or(0),
                    total: model.size,
                },
                task: None,
            },
        );
        let task = tauri::async_runtime::spawn(self.run_download(model, job));
        if let Some(entry) = downloads.get_mut(model.id) {
            entry.task = Some(task);
        }
        drop(downloads);
        log::info!("Downloading the {} model", model.id);
        self.notify();
    }

    async fn run_download(&'static self, model: &'static Model, job: u64) {
        let dest = self.dir.join(model.file);
        let started = Instant::now();
        let result = fetch(&model.source(), &dest, &mut |progress| {
            self.progressed(model.id, job, progress)
        })
        .await;
        {
            let mut downloads = lock(&self.downloads);
            if downloads.get(model.id).map(|d| d.job) != Some(job) {
                return; // cancelled, or deleted
            }
            downloads.remove(model.id);
        }
        match result {
            Ok(()) => {
                log::info!(
                    "Downloaded the {} model in {:.0}s",
                    model.id,
                    started.elapsed().as_secs_f64()
                );
                settings_changed(&self.settings());
            }
            Err(error) => {
                log::warn!("Download of the {} model failed: {error:?}", model.id);
                lock(&self.failures).insert(model.id, error);
            }
        }
        self.notify();
    }

    fn progressed(&self, id: &'static str, job: u64, progress: Progress) {
        {
            let mut downloads = lock(&self.downloads);
            match downloads.get_mut(id) {
                Some(entry) if entry.job == job => entry.progress = progress,
                _ => return,
            }
        }
        let due = lock(&self.last_progress).elapsed() >= PROGRESS_EVERY;
        if due || progress == Progress::Verifying {
            self.notify();
        }
    }

    fn backend(&self) -> Option<Backend> {
        if let Some((_, backend)) = lock(&self.loaded).clone() {
            return Some(backend);
        }
        init_backends(&self.gpu).ok()?;
        expected_backend(&self.gpu)
    }
}

/// What a dictation kept on disk before the engine runs says about itself,
/// should Parler stop before it is transcribed.
pub fn interrupted() -> TranscribeError {
    failed(STOPPED_WHILE_TRANSCRIBING)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::SAMPLE_RATE;
    use crate::providers;
    use std::io::{BufRead, BufReader};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;

    #[test]
    fn the_catalog_is_pinned() {
        assert_eq!(MODELS.len(), 2);
        for model in MODELS {
            assert!(model
                .url
                .starts_with("https://huggingface.co/handy-computer/"));
            assert!(
                model.url.contains("/resolve/"),
                "pinned revision: {}",
                model.url
            );
            assert!(model.url.ends_with(model.file));
            assert_eq!(model.sha256.len(), 64);
            assert!(model.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(model.size > 500_000_000);
        }
        assert_eq!(parakeet().languages.unwrap().len(), 25);
        assert!(whisper().languages.is_none());
        assert!(whisper().supports_vocabulary && !parakeet().supports_vocabulary);
    }

    fn settings(language: &str, ui_language: &str) -> Settings {
        Settings {
            provider: ID.to_string(),
            language: language.to_string(),
            ui_language: ui_language.to_string(),
            ..Settings::default()
        }
    }

    #[test]
    fn parakeet_is_recommended_for_its_languages() {
        let pick = |language: &str, ui: &str, system: Option<&str>| {
            recommended_for(&spoken_language(
                &settings(language, ui),
                system.map(str::to_string),
            ))
            .id
        };
        assert_eq!(pick("fr", "en", None), PARAKEET);
        assert_eq!(pick("ja", "fr", None), WHISPER);
        // Detected: the interface's language decides, then the system's.
        assert_eq!(pick("auto", "de", Some("ja-JP")), PARAKEET);
        assert_eq!(pick("auto", "zh", None), WHISPER);
        assert_eq!(pick("auto", "auto", Some("pt-BR")), PARAKEET);
        assert_eq!(pick("auto", "auto", Some("ko_KR")), WHISPER);
        assert_eq!(pick("auto", "auto", Some("sv-SE")), PARAKEET);
        assert_eq!(pick("auto", "auto", None), PARAKEET);
    }

    #[test]
    fn the_chosen_model_is_the_one_picked_or_the_recommended_one() {
        let mut s = settings("ja", "en");
        assert_eq!(chosen(&s).id, WHISPER);
        s.models.insert(ID.into(), PARAKEET.into());
        assert_eq!(chosen(&s).id, PARAKEET);
        s.models.insert(ID.into(), "gone-model".into());
        assert_eq!(chosen(&s).id, WHISPER);
    }

    #[test]
    fn models_read_as_the_window_expects() {
        let listed = models(&settings("fr", "fr"));
        let json = serde_json::to_value(&listed).unwrap();
        assert_eq!(json[0]["id"], PARAKEET);
        assert_eq!(json[0]["name"], "Parakeet Ultra");
        assert_eq!(json[0]["sizeBytes"], 740_363_168u64);
        assert_eq!(json[0]["languages"][2], "fr");
        assert_eq!(json[0]["recommended"], true);
        assert_eq!(json[0]["supportsVocabulary"], false);
        assert_eq!(json[0]["state"]["state"], "missing");
        assert_eq!(json[1]["languages"], serde_json::Value::Null);
        assert_eq!(json[1]["recommended"], false);

        let downloading = serde_json::to_value(ModelState::Downloading {
            downloaded: 5,
            total: 10,
        })
        .unwrap();
        assert_eq!(
            downloading,
            serde_json::json!({"state": "downloading", "downloaded": 5, "total": 10})
        );
        let failed = serde_json::to_value(ModelState::Failed {
            error: DownloadError::new(DownloadErrorKind::Checksum, None),
        })
        .unwrap();
        assert_eq!(
            failed,
            serde_json::json!({"state": "failed", "error": {"kind": "checksum"}})
        );
        let disk = DownloadError::io(io::Error::from(io::ErrorKind::StorageFull));
        assert_eq!(disk.kind, DownloadErrorKind::Disk);
        assert!(disk.detail.is_some());
        let denied = DownloadError::io(io::Error::from(io::ErrorKind::PermissionDenied));
        assert_eq!(denied.kind, DownloadErrorKind::Other);
    }

    /// An empty folder of its own for each test.
    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("parler-local-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn readiness_is_read_from_the_folder() {
        let dir = folder("state");
        let model = Model {
            size: 4,
            ..*whisper()
        };
        assert!(!on_disk(&dir, &model));
        fs::write(part_path(&dir.join(model.file)), b"half").unwrap();
        assert!(!on_disk(&dir, &model), "a part is not a model");
        fs::write(dir.join(model.file), b"abc").unwrap();
        assert!(!on_disk(&dir, &model), "cut short");
        fs::write(dir.join(model.file), b"abcd").unwrap();
        assert!(on_disk(&dir, &model));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_part_sits_next_to_the_model() {
        assert_eq!(
            part_path(Path::new("/m/whisper.gguf")),
            Path::new("/m/whisper.gguf.part")
        );
        assert_eq!(content_range_start("bytes 100-199/200"), Some(100));
        assert_eq!(content_range_start("bytes */200"), None);
    }

    #[test]
    fn errors_say_what_the_window_should_do() {
        assert_eq!(
            engine_error(transcribe_cpp::Error::ModelFileNotFound("x".into())),
            missing()
        );
        let error = engine_error(transcribe_cpp::Error::OutOfMemory("4 GB".into()));
        assert!(
            matches!(error, TranscribeError::LocalFailed { ref detail, .. } if detail.contains("4 GB"))
        );
        assert!(!error.is_transient() && !missing().is_transient());
        assert_eq!(
            serde_json::to_value(missing()).unwrap()["kind"],
            "modelMissing"
        );
        assert_eq!(serde_json::to_value(&error).unwrap()["kind"], "localFailed");
    }

    #[test]
    fn a_gpu_crash_turns_the_gpu_off_until_the_next_version() {
        let dir = folder("gpu");
        assert!(Gpu::decide(dir.clone()).allowed());

        // A run on the GPU leaves no marker behind.
        let gpu = Gpu::decide(dir.clone());
        assert!(gpu.guard(true, || dir.join(GPU_RUNNING).exists()));
        assert!(!dir.join(GPU_RUNNING).exists());
        assert!(!gpu.guard(false, || dir.join(GPU_RUNNING).exists()));

        // One that never came back did.
        fs::write(dir.join(GPU_RUNNING), b"").unwrap();
        assert!(!Gpu::decide(dir.clone()).allowed());
        assert!(!Gpu::decide(dir.clone()).allowed(), "off from then on");
        fs::write(dir.join(GPU_OFF), "0.0.1").unwrap();
        assert!(
            Gpu::decide(dir.clone()).allowed(),
            "a new version tries again"
        );
        assert!(!dir.join(GPU_OFF).exists());
        let _ = fs::remove_dir_all(dir);
    }

    // -- Downloads, against a server on this computer ----------------------

    /// How the test server answers a Range request.
    #[derive(Clone, Copy, PartialEq)]
    enum Ranges {
        Honoured,
        Ignored,
    }

    struct Server {
        url: String,
        /// The Range header of each request, if any.
        ranges: Arc<Mutex<Vec<Option<String>>>>,
    }

    /// Serves `body`; with `stall_after`, the first response stops there and
    /// hangs, as a dropped connection would.
    fn serve(body: Vec<u8>, ranges: Ranges, stall_after: Option<usize>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model.gguf", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let body = Arc::new(body);
        thread::spawn(move || {
            let mut first = true;
            for stream in listener.incoming().flatten() {
                let range = read_range(&stream);
                lock(&log).push(range.clone());
                let stall = if first { stall_after } else { None };
                first = false;
                let body = body.clone();
                thread::spawn(move || answer(stream, &body, range, ranges, stall));
            }
        });
        Server { url, ranges: seen }
    }

    fn read_range(stream: &TcpStream) -> Option<String> {
        let mut reader = BufReader::new(stream);
        let mut range = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                return range;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("range:") {
                range = Some(value.trim().to_string());
            }
        }
    }

    fn answer(
        mut stream: TcpStream,
        body: &[u8],
        range: Option<String>,
        ranges: Ranges,
        stall: Option<usize>,
    ) {
        let start = range
            .as_deref()
            .and_then(|r| r.strip_prefix("bytes="))
            .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok())
            .filter(|_| ranges == Ranges::Honoured);
        let (head, sent) = match start {
            Some(start) => (
                format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{}/{}\r\n",
                    body.len() - start,
                    body.len() - 1,
                    body.len()
                ),
                &body[start..],
            ),
            None => (
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n", body.len()),
                body,
            ),
        };
        let _ = stream.write_all(format!("{head}Connection: close\r\n\r\n").as_bytes());
        match stall {
            Some(at) => {
                let _ = stream.write_all(&sent[..at]);
                let _ = stream.flush();
                thread::sleep(Duration::from_secs(30));
            }
            None => {
                let _ = stream.write_all(sent);
            }
        }
    }

    fn body() -> Vec<u8> {
        (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect()
    }

    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn fetch_now(
        url: &str,
        size: usize,
        sha256: &str,
        dest: &Path,
    ) -> (Result<(), DownloadError>, Vec<Progress>) {
        let source = Source {
            url,
            size: size as u64,
            sha256,
        };
        let mut seen = Vec::new();
        let result = tauri::async_runtime::block_on(fetch(&source, dest, &mut |p| seen.push(p)));
        (result, seen)
    }

    #[test]
    fn a_download_is_verified_then_renamed() {
        let dir = folder("download");
        let body = body();
        let server = serve(body.clone(), Ranges::Honoured, None);
        let dest = dir.join("model.gguf");
        let (result, progress) = fetch_now(&server.url, body.len(), &sha(&body), &dest);
        result.unwrap();
        assert_eq!(fs::read(&dest).unwrap(), body);
        assert!(!part_path(&dest).exists());
        assert_eq!(progress.last(), Some(&Progress::Verifying));
        assert!(progress.contains(&Progress::Bytes {
            downloaded: body.len() as u64,
            total: body.len() as u64
        }));
        assert_eq!(*lock(&server.ranges), [None]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_part_is_resumed_where_it_stopped() {
        let dir = folder("resume");
        let body = body();
        let dest = dir.join("model.gguf");
        fs::write(part_path(&dest), &body[..100_000]).unwrap();
        let server = serve(body.clone(), Ranges::Honoured, None);
        let (result, progress) = fetch_now(&server.url, body.len(), &sha(&body), &dest);
        result.unwrap();
        assert_eq!(fs::read(&dest).unwrap(), body);
        assert_eq!(*lock(&server.ranges), [Some("bytes=100000-".to_string())]);
        assert_eq!(
            progress[0],
            Progress::Bytes {
                downloaded: 100_000,
                total: body.len() as u64
            }
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_server_ignoring_the_range_starts_over() {
        let dir = folder("ignored");
        let body = body();
        let dest = dir.join("model.gguf");
        // Junk that resuming would wrongly keep.
        fs::write(part_path(&dest), vec![0xAA; 100_000]).unwrap();
        let server = serve(body.clone(), Ranges::Ignored, None);
        let (result, _) = fetch_now(&server.url, body.len(), &sha(&body), &dest);
        result.unwrap();
        assert_eq!(fs::read(&dest).unwrap(), body);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_wrong_file_is_thrown_away() {
        let dir = folder("checksum");
        let body = body();
        let dest = dir.join("model.gguf");
        let server = serve(body.clone(), Ranges::Honoured, None);
        let (result, _) = fetch_now(&server.url, body.len(), &sha(b"another model"), &dest);
        assert_eq!(result.unwrap_err().kind, DownloadErrorKind::Checksum);
        assert!(!dest.exists() && !part_path(&dest).exists());

        // Bigger than announced: stopped at once, not trusted.
        let (result, _) = fetch_now(&server.url, 1000, &sha(&body), &dest);
        assert_eq!(result.unwrap_err().kind, DownloadErrorKind::Other);
        assert!(!part_path(&dest).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn offline_and_refused_downloads_say_so() {
        let dir = folder("offline");
        let dest = dir.join("model.gguf");
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (result, _) = fetch_now(&format!("http://127.0.0.1:{port}/m"), 10, &sha(b""), &dest);
        assert_eq!(result.unwrap_err().kind, DownloadErrorKind::Offline);

        let (base, server) = crate::providers::tests::one_shot_server("404 Not Found", "{}");
        let (result, _) = fetch_now(&format!("{base}/m"), 10, &sha(b""), &dest);
        server.join().unwrap();
        assert_eq!(result.unwrap_err(), DownloadError::other("HTTP 404"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_cancelled_download_keeps_its_part_for_later() {
        let dir = folder("cancel");
        let body = body();
        let dest = dir.join("model.gguf");
        let server = serve(body.clone(), Ranges::Honoured, Some(120_000));
        let url = server.url.clone();
        let (size, hash, path) = (body.len() as u64, sha(&body), dest.clone());
        let received = Arc::new(AtomicU64::new(0));
        let counter = received.clone();
        let task = tauri::async_runtime::spawn(async move {
            let source = Source {
                url: &url,
                size,
                sha256: &hash,
            };
            let _ = fetch(&source, &path, &mut |p| {
                if let Progress::Bytes { downloaded, .. } = p {
                    counter.store(downloaded, Ordering::SeqCst);
                }
            })
            .await;
        });
        let waited = Instant::now();
        while received.load(Ordering::SeqCst) < 120_000
            && waited.elapsed() < Duration::from_secs(10)
        {
            thread::sleep(Duration::from_millis(20));
        }
        // The window's Cancel: the task is dropped mid-transfer.
        task.abort();
        thread::sleep(Duration::from_millis(100));
        assert_eq!(fs::metadata(part_path(&dest)).unwrap().len(), 120_000);
        assert!(!dest.exists());

        let (result, _) = fetch_now(&server.url, body.len(), &sha(&body), &dest);
        result.unwrap();
        assert_eq!(fs::read(&dest).unwrap(), body);
        assert_eq!(
            lock(&server.ranges).last().unwrap().as_deref(),
            Some("bytes=120000-")
        );
        let _ = fs::remove_dir_all(dir);
    }

    // -- The real thing ----------------------------------------------------

    /// Downloads a real model into `PARLER_MODELS_DIR` (kept there for next
    /// time) and transcribes the 16 kHz mono WAV files named in
    /// `PARLER_TEST_WAVS` (comma-separated), with `PARLER_TEST_LANGUAGE`
    /// (or detection) and `PARLER_TEST_MODEL` (Parakeet by default):
    ///
    /// PARLER_MODELS_DIR=/tmp/models PARLER_TEST_WAVS=fr.wav cargo test --release \
    ///   real_download_and_transcription -- --ignored --nocapture
    #[test]
    #[ignore = "downloads a 600-700 MB model"]
    fn real_download_and_transcription() {
        let dir = PathBuf::from(std::env::var("PARLER_MODELS_DIR").expect("PARLER_MODELS_DIR"));
        let model = find(&std::env::var("PARLER_TEST_MODEL").unwrap_or(PARAKEET.into())).unwrap();
        let dest = dir.join(model.file);
        if !on_disk(&dir, model) {
            let started = Instant::now();
            let mut last = Instant::now();
            let result = tauri::async_runtime::block_on(fetch(&model.source(), &dest, &mut |p| {
                if last.elapsed() > Duration::from_secs(5) || p == Progress::Verifying {
                    last = Instant::now();
                    println!("{p:?}");
                }
            }));
            result.unwrap();
            println!("DOWNLOAD {:.0}s", started.elapsed().as_secs_f64());
        }
        assert!(on_disk(&dir, model));

        let gpu = Gpu {
            dir: None,
            allowed: AtomicBool::new(std::env::var("PARLER_TEST_CPU").is_err()),
        };
        let mut engine = Engine::new();
        let started = Instant::now();
        engine.ensure(&gpu, model, &dest).unwrap();
        let loaded = engine.loaded.as_ref().unwrap();
        println!(
            "LOAD {:.2}s on {} (vocabulary: {})",
            started.elapsed().as_secs_f64(),
            loaded.backend,
            loaded.vocabulary
        );
        let language = std::env::var("PARLER_TEST_LANGUAGE").ok();
        let wavs = std::env::var("PARLER_TEST_WAVS").unwrap_or_default();
        for wav in wavs.split(',').filter(|w| !w.is_empty()) {
            let samples = providers::decode_wav(&fs::read(wav).unwrap()).unwrap();
            let seconds = samples.len() as f64 / SAMPLE_RATE as f64;
            let job = Job {
                model,
                path: dest.clone(),
                language: language.clone(),
                vocabulary: providers::vocabulary_terms("Parler, Synapsr"),
            };
            for round in 0..2 {
                let started = Instant::now();
                let text = engine.transcribe(&gpu, &job, &samples).unwrap();
                println!(
                    "RUN {round} {wav}: {seconds:.1}s of audio in {:.2}s: {text}",
                    started.elapsed().as_secs_f64()
                );
                assert!(!text.is_empty());
            }
        }
        let rss = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        println!("RSS {rss} KiB");
    }
}
