//! The dictation loop. One thread owns the microphone and the state; every
//! input (the shortcut, the tray, the command line, the microphone, a
//! finished transcription) arrives on its channel, so nothing races.

use crate::audio::{self, CaptureEvent, CaptureSink};
use crate::history::{self, History};
use crate::overlay::{self, Phase as OverlayPhase};
use crate::paste::{self, PasteError};
use crate::providers::{self, Request, TranscribeError};
use crate::settings::{Output, SettingsStore, ShortcutMode};
use crate::{shortcut, tray};
use handy_recorder::Recorder;
use serde::Serialize;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// In smart mode, a press held at least this long is push-to-talk; a shorter
/// tap keeps recording until the next press.
const HOLD_THRESHOLD: Duration = Duration::from_millis(350);

pub enum Event {
    /// The dictation shortcut went down.
    Press,
    /// The dictation shortcut came up.
    Release,
    /// Start or stop, from the tray, the window or `parler --toggle`.
    Toggle,
    Cancel,
    /// Send the last failed recording again.
    Retry,
    Capture(CaptureEvent),
    Finished {
        session: u64,
        outcome: Outcome,
    },
    PasteFailed(PasteError),
}

impl From<CaptureEvent> for Event {
    fn from(event: CaptureEvent) -> Self {
        Self::Capture(event)
    }
}

pub enum Outcome {
    Text {
        text: String,
        duration_ms: u64,
        provider: String,
    },
    /// Nobody spoke, or the model heard nothing.
    Silence,
    Failed {
        error: TranscribeError,
        samples: Vec<f32>,
    },
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Idle,
    Recording,
    Transcribing,
}

/// Something the user should know about, shown until the next dictation.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Problem {
    Transcription { error: TranscribeError },
    Microphone { detail: String },
    Paste { error: PasteError },
    Shortcut { shortcut: String, detail: String },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: State,
    pub problem: Option<Problem>,
    pub can_retry: bool,
}

/// The latest status, for a window that opens mid-way.
pub struct StatusState(pub Mutex<Status>);

#[derive(Clone)]
pub struct Dictation(Sender<Event>);

impl Dictation {
    pub fn spawn(app: &AppHandle) -> Self {
        let (tx, rx) = mpsc::channel();
        let controller = Controller {
            app: app.clone(),
            tx: tx.clone(),
            phase: Phase::Idle,
            recorder: None,
            next_recorder: 0,
            session: 0,
            retry: None,
            retrying: false,
        };
        thread::Builder::new()
            .name("dictation".into())
            .spawn(move || controller.run(rx))
            .expect("the dictation thread must start");
        Self(tx)
    }

    pub fn send(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

enum Phase {
    Idle,
    Recording {
        pressed_at: Instant,
        latched: bool,
        /// A press came to stop this hands-free recording; it stops when that
        /// key comes up.
        stopping: bool,
    },
    Transcribing,
}

struct Controller {
    app: AppHandle,
    tx: Sender<Event>,
    phase: Phase,
    recorder: Option<(u64, Recorder<CaptureSink>)>,
    next_recorder: u64,
    /// Bumped by every transcription and cancel; a result from an older
    /// session is dropped.
    session: u64,
    /// The last recording that failed to transcribe.
    retry: Option<Vec<f32>>,
    /// The current transcription is a retry from the window, so its text is
    /// copied rather than pasted into Parler itself.
    retrying: bool,
}

impl Controller {
    fn run(mut self, rx: Receiver<Event>) {
        for event in rx {
            match event {
                Event::Press => self.press(),
                Event::Release => self.release(),
                Event::Toggle => self.toggle(),
                Event::Cancel => self.cancel(),
                Event::Retry => self.retry(),
                Event::Capture(CaptureEvent::Ready { id }) => {
                    if self.is_current(id) {
                        overlay::show(&self.app, OverlayPhase::Recording, self.sounds());
                    }
                }
                Event::Capture(CaptureEvent::Failed { id, error }) => {
                    if self.is_current(id) {
                        log::warn!("Microphone failed: {error}");
                        // Keep what was captured before it failed.
                        let recording = matches!(self.phase, Phase::Recording { .. });
                        self.finish();
                        if recording {
                            self.report(Problem::Microphone { detail: error });
                        }
                    }
                }
                Event::Finished { session, outcome } => self.finished(session, outcome),
                Event::PasteFailed(error) => {
                    let needs_permission = error == PasteError::NeedsAccessibility;
                    self.report(Problem::Paste { error });
                    if needs_permission {
                        crate::show_main_window(&self.app);
                    }
                }
            }
        }
    }

    fn settings(&self) -> crate::settings::Settings {
        self.app.state::<SettingsStore>().get()
    }

    fn sounds(&self) -> bool {
        self.settings().sounds
    }

    fn is_current(&self, recorder_id: u64) -> bool {
        self.recorder
            .as_ref()
            .is_some_and(|(id, _)| *id == recorder_id)
    }

    fn press(&mut self) {
        match self.phase {
            Phase::Idle => {
                let latched = self.settings().shortcut_mode == ShortcutMode::Toggle;
                self.start(latched);
            }
            // Stopped when the key comes back up, with all it heard.
            Phase::Recording {
                pressed_at,
                latched: true,
                ..
            } => {
                self.phase = Phase::Recording {
                    pressed_at,
                    latched: true,
                    stopping: true,
                }
            }
            // Key repeat while held, or a press while the last one is still
            // being transcribed.
            Phase::Recording { latched: false, .. } | Phase::Transcribing => {}
        }
    }

    fn release(&mut self) {
        let Phase::Recording {
            pressed_at,
            latched,
            stopping,
        } = self.phase
        else {
            return;
        };
        if stopping {
            self.finish();
            return;
        }
        if latched {
            return;
        }
        match self.settings().shortcut_mode {
            ShortcutMode::Hold => self.finish(),
            ShortcutMode::Smart if pressed_at.elapsed() >= HOLD_THRESHOLD => self.finish(),
            ShortcutMode::Smart | ShortcutMode::Toggle => {
                self.phase = Phase::Recording {
                    pressed_at,
                    latched: true,
                    stopping: false,
                };
            }
        }
    }

    fn toggle(&mut self) {
        match self.phase {
            Phase::Idle => self.start(true),
            Phase::Recording { .. } => self.finish(),
            Phase::Transcribing => {}
        }
    }

    fn start(&mut self, latched: bool) {
        let settings = self.settings();
        // Recording is pointless until a provider is set up: say so instead.
        if let Err(error) = Request::from_settings(&settings) {
            self.fail(Problem::Transcription { error }, true);
            return;
        }

        overlay::show(&self.app, OverlayPhase::Connecting, settings.sounds);
        shortcut::set_cancel_active(&self.app, true);
        self.next_recorder += 1;
        let id = self.next_recorder;
        match audio::start(&self.app, id, settings.microphone.clone(), &self.tx) {
            Ok(recorder) => {
                self.recorder = Some((id, recorder));
                self.phase = Phase::Recording {
                    pressed_at: Instant::now(),
                    latched,
                    stopping: false,
                };
                self.retrying = false;
                self.publish(State::Recording, None);
            }
            Err(detail) => {
                log::warn!("Could not start recording: {detail}");
                shortcut::set_cancel_active(&self.app, false);
                self.fail(Problem::Microphone { detail }, false);
            }
        }
    }

    /// Stops recording and sends what was captured.
    fn finish(&mut self) {
        let Some((_, recorder)) = self.recorder.take() else {
            return;
        };
        let samples = match recorder.stop() {
            Ok(stopped) => stopped.sink.into_samples(),
            Err(e) => {
                log::warn!("Recording lost: {e}");
                Vec::new()
            }
        };
        // Dropping the recorder closes the microphone.
        drop(recorder);
        self.transcribe(samples);
    }

    fn transcribe(&mut self, samples: Vec<f32>) {
        self.session += 1;
        self.phase = Phase::Transcribing;
        let settings = self.settings();
        overlay::show(&self.app, OverlayPhase::Transcribing, settings.sounds);
        self.publish(State::Transcribing, None);

        let session = self.session;
        let tx = self.tx.clone();
        tauri::async_runtime::spawn(async move {
            let outcome = transcribe(&settings, samples).await;
            let _ = tx.send(Event::Finished { session, outcome });
        });
    }

    fn finished(&mut self, session: u64, outcome: Outcome) {
        if session != self.session || !matches!(self.phase, Phase::Transcribing) {
            return; // cancelled
        }
        self.phase = Phase::Idle;
        shortcut::set_cancel_active(&self.app, false);
        let settings = self.settings();

        match outcome {
            Outcome::Text {
                text,
                duration_ms,
                provider,
            } => {
                overlay::hide(&self.app);
                self.retry = None;
                self.app
                    .state::<History>()
                    .add(text.clone(), duration_ms, provider);
                history::notify(&self.app);
                self.publish(State::Idle, None);
                self.deliver(text, settings.output);
            }
            Outcome::Silence => {
                overlay::flash(&self.app, OverlayPhase::Empty, settings.sounds, None);
                self.publish(State::Idle, None);
            }
            Outcome::Failed { error, samples } => {
                log::warn!("Transcription failed: {error}");
                // A key or plan problem needs the window; anything else can
                // be retried from it later.
                let needs_setup = matches!(
                    error,
                    TranscribeError::MissingApiKey { .. }
                        | TranscribeError::Unauthorized { .. }
                        | TranscribeError::SignedOut { .. }
                        | TranscribeError::SubscriptionRequired { .. }
                );
                self.retry = Some(samples);
                self.fail(Problem::Transcription { error }, needs_setup);
            }
        }
    }

    fn deliver(&self, text: String, output: Output) {
        // A retry is started from Parler's own window: pasting would type
        // into Parler, so the text goes to the clipboard.
        if self.retrying || output == Output::Copy {
            if let Err(error) = paste::copy(&self.app, &text) {
                let _ = self.tx.send(Event::PasteFailed(error));
            }
            return;
        }
        let app = self.app.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            if let Err(error) = paste::paste(&app, &text) {
                log::warn!("Paste failed: {error:?}");
                let _ = tx.send(Event::PasteFailed(error));
            }
        });
    }

    fn cancel(&mut self) {
        match self.phase {
            Phase::Idle => return,
            // Dropping the recorder discards the audio and closes the mic.
            Phase::Recording { .. } => drop(self.recorder.take()),
            Phase::Transcribing => self.session += 1,
        }
        log::info!("Dictation cancelled");
        self.phase = Phase::Idle;
        shortcut::set_cancel_active(&self.app, false);
        overlay::hide(&self.app);
        self.publish(State::Idle, None);
    }

    fn retry(&mut self) {
        if !matches!(self.phase, Phase::Idle) {
            return;
        }
        if let Some(samples) = self.retry.take() {
            self.retrying = true;
            shortcut::set_cancel_active(&self.app, true);
            self.transcribe(samples);
        }
    }

    fn state(&self) -> State {
        match self.phase {
            Phase::Idle => State::Idle,
            Phase::Recording { .. } => State::Recording,
            Phase::Transcribing => State::Transcribing,
        }
    }

    fn report(&self, problem: Problem) {
        self.publish(self.state(), Some(problem));
    }

    /// Reports a problem that ended the dictation, on the pill as well.
    fn fail(&self, problem: Problem, show_window: bool) {
        overlay::flash(
            &self.app,
            OverlayPhase::Error,
            self.sounds(),
            Some(problem.clone()),
        );
        self.report(problem);
        if show_window {
            crate::show_main_window(&self.app);
        }
    }

    /// Records and broadcasts the status. A new dictation clears the last
    /// problem; finishing one leaves it visible.
    fn publish(&self, state: State, problem: Option<Problem>) {
        let shared = self.app.state::<StatusState>();
        let status = {
            let mut current = shared.0.lock().unwrap_or_else(|e| e.into_inner());
            current.state = state;
            if problem.is_some() || state == State::Recording {
                current.problem = problem;
            }
            current.can_retry = self.retry.is_some() && state == State::Idle;
            current.clone()
        };
        tray::set_recording(&self.app, state != State::Idle);
        let _ = self.app.emit_to("main", "parler://status", status);
    }
}

async fn transcribe(settings: &crate::settings::Settings, samples: Vec<f32>) -> Outcome {
    let Some(speech) = audio::speech(&samples) else {
        log::info!("Nothing heard in {} samples", samples.len());
        return Outcome::Silence;
    };
    let duration_ms = speech.len() as u64 * 1000 / audio::SAMPLE_RATE as u64;
    let request = match Request::from_settings(settings) {
        Ok(request) => request,
        Err(error) => return Outcome::Failed { error, samples },
    };
    let started = Instant::now();
    let wav = audio::encode_wav(speech);
    match providers::transcribe(&request, wav).await {
        Ok(text) if text.is_empty() => Outcome::Silence,
        Ok(text) => {
            log::info!(
                "Transcribed {:.1}s of speech in {:.2}s",
                duration_ms as f64 / 1000.0,
                started.elapsed().as_secs_f64()
            );
            Outcome::Text {
                text,
                duration_ms,
                provider: request.provider_label().to_string(),
            }
        }
        Err(error) => Outcome::Failed { error, samples },
    }
}
