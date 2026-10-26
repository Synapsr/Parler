//! Microphone capture and the little audio processing Parler needs before
//! sending a recording: noticing a microphone that sends nothing, dropping
//! silence and encoding WAV.

use handy_recorder::{AudioChunk, ErrorKind, Permission, Recorder, RecorderConfig, Sink};
use serde::Serialize;
use std::fmt;
use std::io::Cursor;
use std::sync::mpsc::Sender;
use tauri::{AppHandle, Emitter};

pub const SAMPLE_RATE: u32 = 16_000;
/// 30 ms at 16 kHz: the chunk size the recorder delivers, and the frame size
/// of the speech detection below.
const FRAME: usize = 480;

/// A 30 ms chunk whose loudest sample stays under -65 dBFS carries no
/// signal. The peak rather than the average: noise peaks about 10 dB above
/// its own average, so even a microphone hissing as low as -75 dBFS in a
/// silent room counts as working, while a muted, blocked or unplugged one
/// (zeros, or a few bits of noise) does not.
const NO_SIGNAL_PEAK: f32 = 0.000_56; // 10^(-65/20)
/// How long a microphone may send nothing, once live, before the pill says
/// so. Long enough for a breath before speaking.
const NO_SIGNAL_AFTER: usize = 3 * SAMPLE_RATE as usize;
/// One step of 16-bit audio (-90 dBFS): below it, a microphone sends only
/// zeros, give or take the dither some drivers add.
const DIGITAL_SILENCE: f32 = 1.0 / 32_768.0;

/// Messages the capture sends back to the dictation loop.
pub enum CaptureEvent {
    /// The first audio arrived: the microphone is live.
    Ready {
        id: u64,
    },
    /// Nothing at all has come from the microphone since it went live.
    NoSignal {
        id: u64,
    },
    /// Signal arrived after `NoSignal`.
    SignalBack {
        id: u64,
    },
    Failed {
        id: u64,
        error: MicrophoneError,
    },
}

/// Why the microphone can't be used.
#[derive(Debug)]
pub enum MicrophoneError {
    /// The system's privacy settings keep Parler from it.
    Denied,
    Failed(String),
}

impl From<handy_recorder::Error> for MicrophoneError {
    fn from(error: handy_recorder::Error) -> Self {
        if error.kind() == ErrorKind::PermissionDenied {
            Self::Denied
        } else {
            Self::Failed(error.to_string())
        }
    }
}

impl fmt::Display for MicrophoneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied => write!(f, "microphone access is denied"),
            Self::Failed(detail) => write!(f, "{detail}"),
        }
    }
}

pub struct CaptureSink {
    id: u64,
    app: AppHandle,
    events: Sender<crate::dictation::Event>,
    samples: Vec<f32>,
    watch: SignalWatch,
}

impl CaptureSink {
    pub fn into_samples(self) -> Vec<f32> {
        self.samples
    }
}

impl Sink for CaptureSink {
    fn process_chunk(&mut self, chunk: AudioChunk<'_>) {
        if self.samples.is_empty() {
            let _ = self.events.send(CaptureEvent::Ready { id: self.id }.into());
        }
        let real = &chunk.samples[..chunk.valid_frames];
        self.samples.extend_from_slice(real);
        // One level per 30 ms chunk drives the overlay's bars.
        let _ = self.app.emit_to("overlay", "parler://level", level(real));
        let id = self.id;
        let change = match self.watch.chunk(real) {
            Some(SignalChange::Missing) => CaptureEvent::NoSignal { id },
            Some(SignalChange::Back) => CaptureEvent::SignalBack { id },
            None => return,
        };
        let _ = self.events.send(change.into());
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SignalChange {
    /// No signal since the microphone went live, for `NO_SIGNAL_AFTER`.
    Missing,
    /// Signal, after all.
    Back,
}

/// Watches the start of a recording for a microphone that sends nothing.
/// Once it has heard something it says nothing more, so a pause later in a
/// dictation (or a microphone that gates its own noise) never sets it off.
#[derive(Default)]
pub struct SignalWatch {
    silent: usize,
    warned: bool,
    heard: bool,
}

impl SignalWatch {
    /// Takes the next chunk; says when the pill should start or stop
    /// telling the person.
    pub fn chunk(&mut self, samples: &[f32]) -> Option<SignalChange> {
        if self.heard {
            return None;
        }
        if peak(samples) > NO_SIGNAL_PEAK {
            self.heard = true;
            return self.warned.then_some(SignalChange::Back);
        }
        self.silent += samples.len();
        if !self.warned && self.silent >= NO_SIGNAL_AFTER {
            self.warned = true;
            return Some(SignalChange::Missing);
        }
        None
    }
}

/// Opens the microphone and starts recording into a fresh sink. A saved
/// device that is gone falls back to the system default.
pub fn start(
    app: &AppHandle,
    id: u64,
    device: Option<String>,
    events: &Sender<crate::dictation::Event>,
) -> Result<Recorder<CaptureSink>, MicrophoneError> {
    let open = |device: Option<String>| {
        let failures = events.clone();
        Recorder::open_with_failure_handler(
            RecorderConfig {
                device,
                ..RecorderConfig::speech()
            },
            move |error| {
                let error = error.into();
                let _ = failures.send(CaptureEvent::Failed { id, error }.into());
            },
        )
    };
    let recorder = match open(device.clone()) {
        Ok(recorder) => recorder,
        // Another microphone won't help when none may be used.
        Err(e) if device.is_some() && e.kind() != ErrorKind::PermissionDenied => {
            log::warn!("Saved microphone unavailable ({e}); using the default one");
            open(None)?
        }
        Err(e) => return Err(e.into()),
    };
    log::info!("Recording from {}", recorder.info().device.name);
    recorder
        .start(CaptureSink {
            id,
            app: app.clone(),
            events: events.clone(),
            samples: Vec::with_capacity(SAMPLE_RATE as usize * 30),
            watch: SignalWatch::default(),
        })
        .map_err(|e| e.error)?;
    Ok(recorder)
}

/// Whether the system lets Parler use the microphone, as far as it says
/// up front (macOS and Windows do; Linux doesn't).
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MicrophoneAccess {
    Granted,
    Denied,
    /// Never asked yet: macOS asks on the first recording.
    NotDetermined,
    Unknown,
}

/// Reads the privacy settings; never prompts.
pub fn microphone_access() -> MicrophoneAccess {
    match handy_recorder::permission_status() {
        Permission::Granted => MicrophoneAccess::Granted,
        Permission::Denied => MicrophoneAccess::Denied,
        Permission::NotDetermined => MicrophoneAccess::NotDetermined,
        _ => MicrophoneAccess::Unknown,
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Microphone {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub fn list_microphones() -> Vec<Microphone> {
    match handy_recorder::list_input_devices() {
        Ok(devices) => devices
            .into_iter()
            .filter(|d| !d.is_monitor)
            .map(|d| Microphone {
                id: d.id,
                name: d.name,
                is_default: d.is_default,
            })
            .collect(),
        Err(e) => {
            log::warn!("Could not list microphones: {e}");
            Vec::new()
        }
    }
}

/// The loudest sample, as a fraction of full scale.
fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |loudest, s| loudest.max(s.abs()))
}

/// Whether a recording is digital silence from end to end: a muted
/// microphone, or one the system lets Parler open but not hear. A moment of
/// zeros is normal while a microphone wakes up, so under a second never
/// counts.
pub fn is_digital_silence(samples: &[f32]) -> bool {
    samples.len() >= SAMPLE_RATE as usize && peak(samples) <= DIGITAL_SILENCE
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Loudness on a 0..1 scale that reads well as a meter: -55 dBFS and below
/// is silent, -10 dBFS is full.
fn level(samples: &[f32]) -> f32 {
    let db = 20.0 * rms(samples).max(1e-6).log10();
    ((db + 55.0) / 45.0).clamp(0.0, 1.0)
}

/// The part of a recording that holds speech, with a little air around it, or
/// `None` when nobody spoke. Whisper invents text on silence ("Thank you.",
/// "Sous-titres réalisés par..."), so silent recordings are never sent.
pub fn speech(samples: &[f32]) -> Option<&[f32]> {
    const MIN_SPEECH_FRAMES: usize = 8; // 240 ms
    const LEAD_FRAMES: usize = 10; // 300 ms kept before speech
    const TAIL_FRAMES: usize = 15; // 450 ms kept after it

    let energies: Vec<f32> = samples.chunks(FRAME).map(rms).collect();
    if energies.is_empty() {
        return None;
    }
    // Speech stands well above the room's own noise; the quietest tenth of
    // the recording estimates that floor. The cap keeps a recording that is
    // all speech, whose "floor" is itself speech, from discarding most of it.
    let mut sorted = energies.clone();
    sorted.sort_by(f32::total_cmp);
    let floor = sorted[sorted.len() / 10];
    let threshold = (floor * 3.0).clamp(0.006, 0.02);

    let voiced: Vec<usize> = energies
        .iter()
        .enumerate()
        .filter(|(_, &e)| e > threshold)
        .map(|(i, _)| i)
        .collect();
    if voiced.len() < MIN_SPEECH_FRAMES {
        return None;
    }
    let first = voiced[0].saturating_sub(LEAD_FRAMES);
    let last = (voiced[voiced.len() - 1] + TAIL_FRAMES + 1).min(energies.len());
    Some(&samples[first * FRAME..(last * FRAME).min(samples.len())])
}

/// 16-bit PCM mono WAV.
pub fn encode_wav(samples: &[f32]) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::with_capacity(44 + samples.len() * 2));
    {
        let mut writer =
            hound::WavWriter::new(&mut cursor, spec).expect("writing to memory cannot fail");
        for sample in samples {
            let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer
                .write_sample(value)
                .expect("writing to memory cannot fail");
        }
        writer.finalize().expect("writing to memory cannot fail");
    }
    cursor.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frames: usize, amplitude: f32) -> Vec<f32> {
        (0..frames * FRAME)
            .map(|i| amplitude * (i as f32 * 0.07).sin())
            .collect()
    }

    #[test]
    fn silence_holds_no_speech() {
        assert!(speech(&vec![0.0; FRAME * 100]).is_none());
        assert!(speech(&tone(100, 0.002)).is_none());
        assert!(speech(&[]).is_none());
    }

    #[test]
    fn speech_is_trimmed_with_air_around_it() {
        let mut samples = tone(60, 0.001);
        samples.extend(tone(30, 0.3));
        samples.extend(tone(60, 0.001));
        let kept = speech(&samples).unwrap();
        assert_eq!(kept.len(), (10 + 30 + 15) * FRAME);
    }

    #[test]
    fn a_blip_is_not_speech() {
        let mut samples = tone(50, 0.0);
        samples.extend(tone(3, 0.5));
        samples.extend(tone(50, 0.0));
        assert!(speech(&samples).is_none());
    }

    /// Feeds `samples` to a watch chunk by chunk, keeping what it said.
    fn watch(samples: &[f32]) -> Vec<SignalChange> {
        let mut watch = SignalWatch::default();
        samples
            .chunks(FRAME)
            .filter_map(|chunk| watch.chunk(chunk))
            .collect()
    }

    #[test]
    fn a_microphone_sending_nothing_is_noticed_after_three_seconds() {
        let seconds = |s: usize| s * SAMPLE_RATE as usize / FRAME;
        assert!(watch(&tone(seconds(3) - 1, 0.0)).is_empty());
        assert_eq!(watch(&tone(seconds(3), 0.0)), [SignalChange::Missing]);
        // A few bits of noise, as from an unplugged input, are no signal.
        assert_eq!(watch(&tone(seconds(4), 0.0003)), [SignalChange::Missing]);
    }

    /// Seconds of noise averaging `db` dBFS.
    fn noise(seconds: usize, db: f32) -> Vec<f32> {
        let rms = 10f32.powf(db / 20.0);
        let mut seed = 0x2545_f491_u32;
        let mut uniform = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed as f32 / u32::MAX as f32 - 0.5
        };
        // Twelve uniforms add up to nearly normal noise of variance one.
        (0..seconds * SAMPLE_RATE as usize)
            .map(|_| rms * (0..12).map(|_| uniform()).sum::<f32>())
            .collect()
    }

    #[test]
    fn a_silent_room_through_a_working_microphone_is_signal() {
        assert!(watch(&noise(20, -75.0)).is_empty());
        assert_eq!(watch(&noise(4, -90.0)), [SignalChange::Missing]);
    }

    #[test]
    fn signal_coming_back_is_said_once_and_pauses_are_not_watched() {
        let mut samples = tone(150, 0.0);
        samples.extend(tone(10, 0.2));
        samples.extend(tone(300, 0.0));
        assert_eq!(watch(&samples), [SignalChange::Missing, SignalChange::Back]);

        let mut speaks_first = tone(10, 0.2);
        speaks_first.extend(tone(300, 0.0));
        assert!(watch(&speaks_first).is_empty());
    }

    #[test]
    fn digital_silence_is_zeros_from_end_to_end() {
        let second = SAMPLE_RATE as usize;
        assert!(is_digital_silence(&vec![0.0; 5 * second]));
        // Dither of a bit or so is still nothing.
        assert!(is_digital_silence(&tone(5 * second / FRAME, 0.00002)));
        // Too short to tell from a microphone waking up.
        assert!(!is_digital_silence(&vec![0.0; second / 2]));
        // A quiet room is not digital silence, only too quiet to send.
        assert!(!is_digital_silence(&tone(5 * second / FRAME, 0.0005)));
        let mut late = vec![0.0; 5 * second];
        late.extend(tone(3, 0.1));
        assert!(!is_digital_silence(&late));
    }

    #[test]
    fn levels_span_zero_to_one() {
        assert_eq!(level(&[0.0; FRAME]), 0.0);
        assert_eq!(level(&[0.9; FRAME]), 1.0);
    }

    #[test]
    fn wav_reads_back_as_16khz_mono() {
        let wav = encode_wav(&[0.0, 0.5, -0.5]);
        let reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
        assert_eq!(reader.spec().sample_rate, SAMPLE_RATE);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.len(), 3);
    }
}
