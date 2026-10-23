//! Microphone capture and the little audio processing Parler needs before
//! sending a recording: dropping silence and encoding WAV.

use handy_recorder::{AudioChunk, Recorder, RecorderConfig, Sink};
use serde::Serialize;
use std::io::Cursor;
use std::sync::mpsc::Sender;
use tauri::{AppHandle, Emitter};

pub const SAMPLE_RATE: u32 = 16_000;
/// 30 ms at 16 kHz: the chunk size the recorder delivers, and the frame size
/// of the speech detection below.
const FRAME: usize = 480;

/// Messages the capture sends back to the dictation loop.
pub enum CaptureEvent {
    /// The first audio arrived: the microphone is live.
    Ready {
        id: u64,
    },
    Failed {
        id: u64,
        error: String,
    },
}

pub struct CaptureSink {
    id: u64,
    app: AppHandle,
    events: Sender<crate::dictation::Event>,
    samples: Vec<f32>,
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
    }
}

/// Opens the microphone and starts recording into a fresh sink. A saved
/// device that is gone falls back to the system default.
pub fn start(
    app: &AppHandle,
    id: u64,
    device: Option<String>,
    events: &Sender<crate::dictation::Event>,
) -> Result<Recorder<CaptureSink>, String> {
    let open = |device: Option<String>| {
        let failures = events.clone();
        Recorder::open_with_failure_handler(
            RecorderConfig {
                device,
                ..RecorderConfig::speech()
            },
            move |error| {
                let _ = failures.send(
                    CaptureEvent::Failed {
                        id,
                        error: error.to_string(),
                    }
                    .into(),
                );
            },
        )
    };
    let recorder = match open(device.clone()) {
        Ok(recorder) => recorder,
        Err(e) if device.is_some() => {
            log::warn!("Saved microphone unavailable ({e}); using the default one");
            open(None).map_err(|e| e.to_string())?
        }
        Err(e) => return Err(e.to_string()),
    };
    log::info!("Recording from {}", recorder.info().device.name);
    recorder
        .start(CaptureSink {
            id,
            app: app.clone(),
            events: events.clone(),
            samples: Vec::with_capacity(SAMPLE_RATE as usize * 30),
        })
        .map_err(|e| e.to_string())?;
    Ok(recorder)
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
