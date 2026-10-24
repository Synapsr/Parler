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
| `cloud.rs` | Parler Cloud: linking a computer with a device code, the account, signing out. |
| `paste.rs` | Clipboard and simulated paste; macOS resolves Cmd+V for the active keyboard layout. |
| `overlay.rs` | The pill window: a non-activating panel on macOS, a no-activate topmost window on Windows, so it never takes focus. |
| `shortcut.rs` | Registers the dictation shortcut, and Escape only while dictating. |
| `settings.rs`, `history.rs` | JSON files in the app's config and data directories, readable by the user only. |
| `tray.rs`, `commands.rs` | Tray menu and the commands the windows call. |

### Shortcut behaviour

In **smart** mode a press starts recording; releasing after 350 ms or more
stops it (push-to-talk), while a quicker tap keeps recording until the next
press. **Hold** and **toggle** are the two halves on their own.

### Parler Cloud

Parler Cloud is the paid plan that removes the API key. To Parler it is one
more provider, `cloud`, speaking the same OpenAI audio API at
`https://cloud.parler.synapsr.io/v1`; its "API key" is a device token. The
app and the service share a contract (endpoints, payloads, error codes).

A computer is linked with the device code flow, like `gh auth login`, so
nothing depends on deep links or the OS:

```
 Parler                                   browser                cloud.parler.synapsr.io
 cloud_sign_in_start ── POST /api/device/start {deviceName, platform} ──────► code ABCD-EFGH
   opens verificationUrl ──────────────► sign in, start the trial,
   shows ABCD-EFGH                       "Authorize this computer"  ─────────► approved
 cloud_sign_in_poll, every `interval` s ── POST /api/device/poll ───────────► {token, account}
   token saved, provider = cloud, window brought back to the front
```

- **The token** is stored like any API key (`api_keys.cloud` in
  `settings.json`, readable by the user only, never logged). Only the
  backend writes it: `save_settings` keeps the stored one, so a window's
  older copy of the settings can't erase it, and the backend sends
  `parler://settings` whenever it changes them.
- **The account** (`GET /api/me`) is loaded when Settings or the window
  needs it, and again whenever the window comes back to the front, so a plan
  bought in the browser shows up. A 401 means the service no longer knows
  the token: it is forgotten and Settings offers to sign in again.
- **Signing out** revokes the token (best effort), forgets it and goes back
  to the service used before Parler Cloud, or Groq.
- **Errors** from Parler Cloud map to their own problems: 401 → signed out,
  402 → no active plan (with the page to choose one), 429 `daily_limit` /
  `monthly_limit` → fair use reached, until the next UTC day or month. Other
  providers keep the usual meaning of those codes.
- `PARLER_CLOUD_URL` (read once at startup) points the app at another
  service, for instance `PARLER_CLOUD_URL=http://localhost:3100 pnpm tauri dev`.

### Why silence is never sent

Whisper models invent text when given silence. Before uploading, `audio::speech`
estimates the room's noise floor, keeps the part of the recording that rises
clearly above it, with a little air around it, and drops recordings with less
than a quarter of a second of speech.

## Interface — `src`

Built on the [design system](design-system.md).

| Path | Role |
| --- | --- |
| `App.tsx`, `state.tsx` | Loads settings, status, history and totals once, then follows backend events. |
| `ui/` | The design system's components. |
| `onboarding/` | Welcome → service → key (checked on paste), or the Parler Cloud sign-in → macOS permissions → shortcut → first dictation, each with its animated scene. |
| `pages/` | Home (shortcut, totals, recent dictations), History (every dictation, searchable), Vocabulary, Settings. |
| `components/` | The sidebar, the history list, the shortcut recorder, the service form logic, the Parler Cloud sign-in and account. |
| `overlay/` | The pill: level bars, phases, synthesized sound cues. |
| `i18n/` | 12 languages (`languages.ts`), kept in sync by a test. |
| `preview.tsx` | Dev only: every window in a browser against a simulated backend. |
