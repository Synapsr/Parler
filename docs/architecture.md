# Architecture

Parler is a [Tauri 2](https://tauri.app) app: a Rust backend that records,
transcribes and pastes, and a React interface in two windows.

```
 global shortcut ─┐
 tray menu ───────┼──► dictation loop ──► microphone (handy-recorder, 16 kHz mono)
 parler --toggle ─┘     (one thread)  ──► speech check, WAV encoding
                              │        ──► provider API (multipart POST /audio/transcriptions)
                              │        ──► paste: clipboard + Cmd/Ctrl+V, clipboard restored
                              ▼
                   events ──► main window (status, history)
                          ──► overlay pill (phase, voice level)
```

## Backend — `src-tauri/src`

| Module | Role |
| --- | --- |
| `lib.rs` | App setup: plugins, state, tray, overlay, shortcut; single instance and `--toggle` / `--hidden`. |
| `dictation.rs` | The dictation loop. One thread owns the microphone and the state; the shortcut, tray, microphone and finished transcriptions all send it events, so press/release/cancel never race. |
| `audio.rs` | Microphone capture, the voice level for the pill, the speech check that keeps silence from being sent, WAV encoding. |
| `providers.rs` | The provider list and the request to a Whisper-compatible API, with errors the interface can explain. |
| `paste.rs` | Clipboard and simulated paste; macOS resolves Cmd+V for the active keyboard layout. |
| `overlay.rs` | The pill window: a non-activating panel on macOS, a no-activate topmost window on Windows, so it never takes focus. |
| `shortcut.rs` | Registers the dictation shortcut, and Escape only while dictating. |
| `settings.rs`, `history.rs` | JSON files in the app's config and data directories, readable by the user only. |
| `tray.rs`, `commands.rs` | Tray menu and the commands the windows call. |

### Shortcut behaviour

In **smart** mode a press starts recording; releasing after 350 ms or more
stops it (push-to-talk), while a quicker tap keeps recording until the next
press. **Hold** and **toggle** are the two halves on their own.

### Why silence is never sent

Whisper models invent text when given silence. Before uploading, `audio::speech`
estimates the room's noise floor, keeps the part of the recording that rises
clearly above it, with a little air around it, and drops recordings with less
than a quarter of a second of speech.

## Interface — `src`

| Path | Role |
| --- | --- |
| `App.tsx`, `state.tsx` | Loads settings, status and history once, then follows backend events. |
| `components/Onboarding.tsx` | Provider → key (tested) → macOS permissions → try it. |
| `components/Home.tsx` | The shortcut, live status, problems with their fix, recent dictations. |
| `components/SettingsView.tsx` | Everything else. |
| `overlay/` | The pill: level bars, phases, synthesized sound cues. |
| `i18n/` | English and French, kept in sync by a test. |
