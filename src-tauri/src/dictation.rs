//! The dictation loop. One thread owns the microphone and the state; every
//! input (the shortcut, the tray, the command line, the microphone, a
//! finished transcription) arrives on its channel, so nothing races.

use crate::audio::{self, CaptureEvent, CaptureSink, MicrophoneError};
use crate::history::{self, History};
use crate::overlay::{self, Hint, Phase as OverlayPhase};
use crate::paste::{self, PasteError};
use crate::pending::{Pending, Unsent};
use crate::providers::{self, Request, TranscribeError};
use crate::settings::{Output, Settings, SettingsStore, ShortcutMode};
use crate::{shortcut, tray};
use handy_recorder::Recorder;
use serde::Serialize;
use std::borrow::Cow;
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// In smart mode, a press held at least this long is push-to-talk; a shorter
/// tap keeps recording until the next press.
const HOLD_THRESHOLD: Duration = Duration::from_millis(350);
/// Esc cancels when pressed twice within this. Once, it only says so, so an
/// Esc meant for another app never throws a dictation away.
const ESCAPE_AGAIN: Duration = Duration::from_millis(1500);
/// A dictation that failed for a passing reason (the network, a server
/// error) is sent once more after this, while the pill keeps writing.
const RETRY_AFTER: Duration = Duration::from_millis(1500);
/// Where dictations that couldn't be sent wait, in the app's data directory.
const PENDING_DIR: &str = "pending";

pub enum Event {
    /// The dictation shortcut went down.
    Press,
    /// The dictation shortcut came up.
    Release,
    /// Start or stop, from the tray, the window or `parler --toggle`.
    Toggle,
    /// Esc, while dictating: twice in a row cancels.
    Escape,
    /// Send an unsent dictation again: the one that just failed (`None`), or
    /// one picked in the window.
    Retry(Option<u64>),
    /// Delete an unsent dictation.
    Discard(u64),
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

/// Speech ready to send: a WAV file and how long it lasts.
pub struct Speech {
    wav: Vec<u8>,
    duration_ms: u64,
}

pub enum Outcome {
    Text {
        text: String,
        duration_ms: u64,
        provider: String,
    },
    /// Nobody spoke, or the model heard nothing.
    Silence,
    /// The microphone sent nothing at all, not even noise.
    NoSound,
    /// A passing failure, sent once more unless cancelled meanwhile.
    TryAgain {
        error: TranscribeError,
        speech: Speech,
    },
    Failed {
        error: TranscribeError,
        speech: Speech,
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
    Transcription {
        error: TranscribeError,
    },
    Microphone {
        detail: String,
    },
    /// The system's privacy settings keep Parler from the microphone.
    MicrophoneDenied,
    /// A whole recording of digital silence: a muted microphone, or one the
    /// system lets Parler open but not hear.
    NoSound,
    Paste {
        error: PasteError,
    },
    Shortcut {
        shortcut: String,
        detail: String,
    },
}

impl From<MicrophoneError> for Problem {
    fn from(error: MicrophoneError) -> Self {
        match error {
            MicrophoneError::Denied => Self::MicrophoneDenied,
            MicrophoneError::Failed(detail) => Self::Microphone { detail },
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: State,
    pub problem: Option<Problem>,
    /// The dictation the problem is about waits to be sent again.
    pub can_retry: bool,
}

/// The latest status, for a window that opens mid-way.
pub struct StatusState(pub Mutex<Status>);

#[derive(Clone)]
pub struct Dictation {
    tx: Sender<Event>,
    pending: Arc<Pending>,
}

impl Dictation {
    pub fn spawn(app: &AppHandle) -> Self {
        let dir = app
            .path()
            .app_data_dir()
            .expect("the setup resolves the data directory first")
            .join(PENDING_DIR);
        let pending = Arc::new(Pending::load(dir));
        let (tx, rx) = mpsc::channel();
        let controller = Controller {
            app: app.clone(),
            tx: tx.clone(),
            phase: Phase::Idle,
            recorder: None,
            next_recorder: 0,
            session: 0,
            pending: pending.clone(),
            resending: None,
            last_failed: None,
            escaped_at: None,
        };
        thread::Builder::new()
            .name("dictation".into())
            .spawn(move || controller.run(rx))
            .expect("the dictation thread must start");
        Self { tx, pending }
    }

    pub fn send(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    /// The dictations that couldn't be sent, newest first.
    pub fn unsent(&self) -> Vec<Unsent> {
        self.pending.list()
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
    /// Dictations that couldn't be sent, on disk.
    pending: Arc<Pending>,
    /// The unsent dictation being sent again from the window: its text is
    /// copied rather than pasted into Parler itself, and once sent it
    /// leaves the store.
    resending: Option<u64>,
    /// The unsent dictation the current problem is about, which the
    /// window's Retry sends again.
    last_failed: Option<u64>,
    /// When Esc was last pressed during this dictation.
    escaped_at: Option<Instant>,
}

impl Controller {
    fn run(mut self, rx: Receiver<Event>) {
        for event in rx {
            // A bug in one event mustn't end dictation until Parler restarts:
            // the panic is logged (see lib.rs), and the loop starts afresh.
            if panic::catch_unwind(AssertUnwindSafe(|| self.handle(event))).is_err() {
                self.recover();
            }
        }
    }

    /// Back to idle after a panic: the microphone is let go, any result on
    /// its way is dropped, and the pill says something went wrong.
    fn recover(&mut self) {
        self.recorder = None;
        self.session += 1;
        self.phase = Phase::Idle;
        self.resending = None;
        self.escaped_at = None;
        shortcut::set_cancel_active(&self.app, false);
        overlay::flash(&self.app, OverlayPhase::Error, self.sounds(), None);
        self.publish(State::Idle, None);
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Press => self.press(),
            Event::Release => self.release(),
            Event::Toggle => self.toggle(),
            Event::Escape => self.escape(),
            Event::Retry(id) => self.retry(id),
            Event::Discard(id) => self.discard(id),
            Event::Capture(CaptureEvent::Ready { id }) => {
                if self.is_current(id) {
                    overlay::show(&self.app, OverlayPhase::Recording, self.sounds());
                }
            }
            // Recording goes on: only the person can tell a muted
            // microphone from one they haven't spoken into yet.
            Event::Capture(CaptureEvent::NoSignal { id }) => {
                if self.is_current(id) {
                    log::info!("No signal from the microphone yet");
                    overlay::keep_hint(&self.app, Hint::NoSound, self.sounds());
                }
            }
            Event::Capture(CaptureEvent::SignalBack { id }) => {
                if self.is_current(id) {
                    overlay::clear_hint(&self.app);
                }
            }
            Event::Capture(CaptureEvent::Failed { id, error }) => {
                if self.is_current(id) {
                    log::warn!("Microphone failed: {error}");
                    // Keep what was captured before it failed.
                    let recording = matches!(self.phase, Phase::Recording { .. });
                    self.finish();
                    if recording {
                        self.report(error.into());
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

    fn settings(&self) -> Settings {
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
        self.escaped_at = None;
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
                self.resending = None;
                self.last_failed = None;
                self.publish(State::Recording, None);
            }
            Err(error) => {
                log::warn!("Could not start recording: {error}");
                shortcut::set_cancel_active(&self.app, false);
                // Only the system settings can let Parler in.
                let denied = matches!(error, MicrophoneError::Denied);
                self.fail(error.into(), denied);
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

    /// Sends what was just recorded.
    fn transcribe(&mut self, samples: Vec<f32>) {
        let settings = self.begin_sending();
        let session = self.session;
        let tx = self.tx.clone();
        tauri::async_runtime::spawn(async move {
            let outcome = match prepare(&samples) {
                Ok(speech) => send(&settings, speech, Some(RETRY_AFTER)).await,
                Err(outcome) => outcome,
            };
            let _ = tx.send(Event::Finished { session, outcome });
        });
    }

    /// Moves to transcribing, on the pill and in the window; the settings to
    /// send with.
    fn begin_sending(&mut self) -> Settings {
        self.session += 1;
        self.phase = Phase::Transcribing;
        let settings = self.settings();
        overlay::show(&self.app, OverlayPhase::Transcribing, settings.sounds);
        self.publish(State::Transcribing, None);
        settings
    }

    /// Sends speech already prepared, in the current session: an unsent
    /// dictation, or the second try of a passing failure.
    fn resend(&self, speech: Speech, retry_after: Option<Duration>) {
        let settings = self.settings();
        let session = self.session;
        let tx = self.tx.clone();
        tauri::async_runtime::spawn(async move {
            let outcome = send(&settings, speech, retry_after).await;
            let _ = tx.send(Event::Finished { session, outcome });
        });
    }

    fn finished(&mut self, session: u64, outcome: Outcome) {
        if session != self.session || !matches!(self.phase, Phase::Transcribing) {
            return; // cancelled
        }
        match outcome {
            // The pill keeps writing through the second try.
            Outcome::TryAgain { error, speech } => {
                log::info!("Transcription failed ({error}); trying once more");
                self.resend(speech, None);
            }
            Outcome::Text {
                text,
                duration_ms,
                provider,
            } => {
                self.end();
                overlay::hide(&self.app);
                self.forget_resent();
                self.app
                    .state::<History>()
                    .add(text.clone(), duration_ms, provider);
                history::notify(&self.app);
                self.publish(State::Idle, None);
                self.deliver(text, self.settings().output);
            }
            Outcome::Silence => {
                self.end();
                self.forget_resent();
                overlay::flash(&self.app, OverlayPhase::Empty, self.sounds(), None);
                self.publish(State::Idle, None);
            }
            Outcome::NoSound => {
                self.end();
                log::warn!("The microphone sent only digital silence");
                self.fail(Problem::NoSound, true);
            }
            Outcome::Failed { error, speech } => {
                self.end();
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
                self.keep_unsent(speech, &error);
                self.fail(Problem::Transcription { error }, needs_setup);
            }
        }
    }

    /// The dictation is over, whatever came of it.
    fn end(&mut self) {
        self.phase = Phase::Idle;
        shortcut::set_cancel_active(&self.app, false);
    }

    /// Keeps a dictation that couldn't be sent, on disk, so it can be sent
    /// later, even after a restart.
    fn keep_unsent(&mut self, speech: Speech, error: &TranscribeError) {
        self.last_failed = match self.resending {
            Some(id) => self.pending.failed_again(id, error).then_some(id),
            None => Some(
                self.pending
                    .save(speech.wav, speech.duration_ms, error.clone()),
            ),
        };
        self.pending_changed();
    }

    /// An unsent dictation went through: it leaves the store, and a
    /// transcription problem still shown is out of date.
    fn forget_resent(&mut self) {
        let Some(id) = self.resending else {
            return;
        };
        if self.pending.remove(id) {
            self.pending_changed();
        }
        let shared = self.app.state::<StatusState>();
        let mut status = shared.0.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(status.problem, Some(Problem::Transcription { .. })) {
            status.problem = None;
        }
    }

    fn pending_changed(&self) {
        let _ = self
            .app
            .emit_to("main", "parler://pending", self.pending.list());
    }

    fn deliver(&self, text: String, output: Output) {
        // A retry is started from Parler's own window: pasting would type
        // into Parler, so the text goes to the clipboard.
        if self.resending.is_some() || output == Output::Copy {
            if let Err(error) = paste::copy(&self.app, &text) {
                let _ = self.tx.send(Event::PasteFailed(error));
            }
            return;
        }
        let app = self.app.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            if let Err(error) = paste::paste(&app, &with_trailing_space(&text)) {
                log::warn!("Paste failed: {error:?}");
                let _ = tx.send(Event::PasteFailed(error));
            }
        });
    }

    /// Esc: the first press says a second one cancels; the second does.
    fn escape(&mut self) {
        if matches!(self.phase, Phase::Idle) {
            return;
        }
        let now = Instant::now();
        if is_second_escape(self.escaped_at, now) {
            self.cancel();
        } else {
            self.escaped_at = Some(now);
            overlay::hint(&self.app, Hint::EscapeAgain, ESCAPE_AGAIN, self.sounds());
        }
    }

    fn cancel(&mut self) {
        self.escaped_at = None;
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

    fn retry(&mut self, id: Option<u64>) {
        if !matches!(self.phase, Phase::Idle) {
            return;
        }
        let Some(id) = id.or(self.last_failed) else {
            return;
        };
        let Some((unsent, wav)) = self.pending.get(id) else {
            // Its audio is gone: nothing left to send.
            if self.pending.remove(id) {
                self.pending_changed();
            }
            return;
        };
        self.resending = Some(id);
        self.last_failed = None;
        shortcut::set_cancel_active(&self.app, true);
        self.begin_sending();
        let speech = Speech {
            wav,
            duration_ms: unsent.duration_ms,
        };
        self.resend(speech, Some(RETRY_AFTER));
    }

    fn discard(&mut self, id: u64) {
        if self.pending.remove(id) {
            self.pending_changed();
        }
        if self.last_failed == Some(id) {
            self.last_failed = None;
            // The problem stays; only its Retry goes.
            self.publish(self.state(), None);
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
            current.can_retry = state == State::Idle
                && self.last_failed.is_some_and(|id| self.pending.contains(id));
            current.clone()
        };
        tray::set_recording(&self.app, state != State::Idle);
        let _ = self.app.emit_to("main", "parler://status", status);
    }
}

/// Whether an Esc at `now` follows the last one closely enough to cancel.
fn is_second_escape(last: Option<Instant>, now: Instant) -> bool {
    last.is_some_and(|at| now.duration_since(at) < ESCAPE_AGAIN)
}

/// The speech in a recording, ready to send, or why there is none.
fn prepare(samples: &[f32]) -> Result<Speech, Outcome> {
    if audio::is_digital_silence(samples) {
        return Err(Outcome::NoSound);
    }
    let Some(speech) = audio::speech(samples) else {
        log::info!("Nothing heard in {} samples", samples.len());
        return Err(Outcome::Silence);
    };
    Ok(Speech {
        duration_ms: speech.len() as u64 * 1000 / audio::SAMPLE_RATE as u64,
        wav: audio::encode_wav(speech),
    })
}

/// Sends speech to the service. With `retry_after`, a passing failure comes
/// back as `TryAgain` once that time has gone by.
async fn send(settings: &Settings, speech: Speech, retry_after: Option<Duration>) -> Outcome {
    let request = match Request::from_settings(settings) {
        Ok(request) => request,
        Err(error) => return Outcome::Failed { error, speech },
    };
    let started = Instant::now();
    match providers::transcribe(&request, speech.wav.clone()).await {
        Ok(text) if text.is_empty() => Outcome::Silence,
        Ok(text) => {
            log::info!(
                "Transcribed {:.1}s of speech in {:.2}s",
                speech.duration_ms as f64 / 1000.0,
                started.elapsed().as_secs_f64()
            );
            Outcome::Text {
                text,
                duration_ms: speech.duration_ms,
                provider: request.provider_label().to_string(),
            }
        }
        Err(error) => match retry_after {
            Some(wait) if error.is_transient() => {
                let _ = tauri::async_runtime::spawn_blocking(move || thread::sleep(wait)).await;
                Outcome::TryAgain { error, speech }
            }
            _ => Outcome::Failed { error, speech },
        },
    }
}

/// Closing quotes and brackets that may follow the end of a sentence.
const CLOSERS: &[char] = &['"', '\'', '”', '’', '»', '›', ')', ']', '}'];
/// Marks that end a sentence or a clause: the usual ones, then Arabic's and
/// Hindi's own.
const ENDINGS: &[char] = &['.', '!', '?', '…', ':', ';', ',', '؟', '،', '؛', '।'];

/// What is typed for `text`: one more space after a closing punctuation
/// mark, so the next dictation doesn't stick to it ("…today.Next"). Chinese,
/// Japanese and Thai put no space between sentences: text with any of their
/// characters is typed as it is.
fn with_trailing_space(text: &str) -> Cow<'_, str> {
    let sentence = text.trim_end_matches(|c: char| CLOSERS.contains(&c) || c.is_whitespace());
    let ends_sentence = sentence.ends_with(ENDINGS);
    if ends_sentence && !text.chars().any(history::is_unspaced) {
        Cow::Owned(format!("{text} "))
    } else {
        Cow::Borrowed(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::tests::one_shot_server;

    #[test]
    fn esc_cancels_only_when_pressed_twice_quickly() {
        let t0 = Instant::now();
        assert!(!is_second_escape(None, t0));
        assert!(is_second_escape(Some(t0), t0 + Duration::from_millis(400)));
        assert!(!is_second_escape(Some(t0), t0 + ESCAPE_AGAIN));
        assert!(!is_second_escape(Some(t0), t0 + Duration::from_secs(5)));
    }

    #[test]
    fn a_pasted_sentence_leaves_room_for_the_next() {
        for (text, typed) in [
            ("See you today.", "See you today. "),
            ("Really?", "Really? "),
            ("Bonjour !", "Bonjour ! "),
            ("Wait…", "Wait… "),
            ("First, second,", "First, second, "),
            ("He said “yes.”", "He said “yes.” "),
            ("« Bonjour. »", "« Bonjour. » "),
            ("(see above.)", "(see above.) "),
            ("هل أنت هنا؟", "هل أنت هنا؟ "),
            // No closing mark: the text may go on.
            ("meet at 14:00", "meet at 14:00"),
            ("parler.app", "parler.app"),
            ("He said “yes”", "He said “yes”"),
            ("", ""),
            // Written without spaces between sentences.
            ("谢谢你的反馈。", "谢谢你的反馈。"),
            ("我们用 Parler.", "我们用 Parler."),
            ("ありがとう!", "ありがとう!"),
            ("ขอบคุณครับ.", "ขอบคุณครับ."),
        ] {
            assert_eq!(with_trailing_space(text), typed, "{text}");
        }
    }

    fn tone(seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(seconds * audio::SAMPLE_RATE as f32) as usize)
            .map(|i| amplitude * (i as f32 * 0.07).sin())
            .collect()
    }

    #[test]
    fn only_speech_is_prepared_for_sending() {
        assert!(matches!(prepare(&tone(4.0, 0.0)), Err(Outcome::NoSound)));
        assert!(matches!(prepare(&tone(0.4, 0.0)), Err(Outcome::Silence)));
        assert!(matches!(prepare(&tone(4.0, 0.001)), Err(Outcome::Silence)));
        let mut samples = tone(1.0, 0.001);
        samples.extend(tone(2.0, 0.3));
        let speech = prepare(&samples).ok().expect("speech to send");
        assert!((2000..2800).contains(&speech.duration_ms));
    }

    fn custom(base_url: String) -> Settings {
        Settings {
            provider: providers::CUSTOM.to_string(),
            custom_base_url: base_url,
            ..Settings::default()
        }
    }

    fn speech() -> Speech {
        Speech {
            wav: audio::encode_wav(&tone(0.5, 0.3)),
            duration_ms: 500,
        }
    }

    fn send_once(status: &str, body: &str, retry_after: Option<Duration>) -> Outcome {
        let (base_url, server) = one_shot_server(status, body);
        let outcome =
            tauri::async_runtime::block_on(send(&custom(base_url), speech(), retry_after));
        server.join().unwrap();
        outcome
    }

    #[test]
    fn a_server_error_is_tried_once_more_and_a_refusal_is_not() {
        let wait = Some(Duration::ZERO);
        assert!(matches!(
            send_once("200 OK", r#"{"text":"Bonjour"}"#, wait),
            Outcome::Text { text, duration_ms: 500, .. } if text == "Bonjour"
        ));
        assert!(matches!(
            send_once("503 Service Unavailable", "busy", wait),
            Outcome::TryAgain {
                error: TranscribeError::Rejected { status: 503, .. },
                ..
            }
        ));
        // The second try is the last.
        assert!(matches!(
            send_once("503 Service Unavailable", "busy", None),
            Outcome::Failed { .. }
        ));
        assert!(matches!(
            send_once("400 Bad Request", "{}", wait),
            Outcome::Failed {
                error: TranscribeError::Rejected { status: 400, .. },
                ..
            }
        ));
        assert!(matches!(
            send_once("401 Unauthorized", "{}", wait),
            Outcome::Failed { error: TranscribeError::Unauthorized { .. }, speech }
                if speech.duration_ms == 500
        ));
    }

    #[test]
    fn no_connection_is_tried_once_more() {
        // A port nothing listens on any more.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let settings = custom(format!("http://127.0.0.1:{port}/v1"));
        let outcome =
            tauri::async_runtime::block_on(send(&settings, speech(), Some(Duration::ZERO)));
        assert!(matches!(
            outcome,
            Outcome::TryAgain {
                error: TranscribeError::Offline { .. },
                ..
            }
        ));
    }
}
