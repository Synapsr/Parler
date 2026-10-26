# Architecture

Parler is a [Tauri 2](https://tauri.app) app: a Rust backend that records,
transcribes and pastes, and a React interface in two windows.

```
 global shortcut ─┐
 tray menu ───────┼──► dictation loop ──► microphone (handy-recorder, 16 kHz mono)
 parler --toggle ─┘     (one thread)  ──► speech check, WAV encoding
                              │        ──► provider API (multipart POST /audio/transcriptions)
                              │        ──► not sent? kept on disk to send again (pending)
                              │        ──► paste: clipboard + Cmd/Ctrl+V, clipboard restored
                              ▼
                   events ──► main window (status, history)
                          ──► overlay pill (phase, voice level)
```

## Backend — `src-tauri/src`

| Module | Role |
| --- | --- |
| `lib.rs` | App setup: plugins, state, tray, overlay, shortcut; single instance and `--toggle` / `--hidden`; panics into the log. On macOS Parler leaves the Dock while its window is closed, like other menu bar apps. |
| `dictation.rs` | The dictation loop. One thread owns the microphone and the state; the shortcut, tray, microphone and finished transcriptions all send it events, so press/release/cancel never race. It sends a passing failure once more, keeps what still fails in `pending`, and adds a space after a pasted sentence. A panic in one event is logged and the loop goes back to idle, freeing the microphone, instead of leaving dictation dead until a restart. |
| `audio.rs` | Microphone capture, the voice level for the pill, noticing a microphone that sends nothing, the speech check that keeps silence from being sent, WAV encoding. |
| `pending.rs` | Dictations that couldn't be sent: their speech and a few facts on disk, readable by the user only, until sent, deleted or 7 days old (see below). |
| `providers.rs` | The provider list and the request to a Whisper-compatible API, with errors the interface can explain. |
| `cloud.rs` | Parler Cloud: linking a computer with a device code, the account, signing out. |
| `updates.rs` | Checks for updates, downloads and installs them, then restarts Parler. |
| `diagnostics.rs` | Settings → About → "Send the log": version, system, a few settings, the microphone permission (macOS, Windows) and Accessibility (macOS), and the end of the log (keys masked) to `POST /api/diagnostics` on Parler Cloud, which emails support and keeps nothing. |
| `paste.rs` | Clipboard and simulated paste: the transcript passes through the clipboard unseen by clipboard histories, and what was there comes back (see below); macOS resolves Cmd+V for the active keyboard layout. |
| `overlay.rs` | The pill window: a non-activating panel on macOS, a no-activate topmost window on Windows, so it never takes focus. |
| `shortcut.rs` | Registers the dictation shortcut, and Escape only while dictating (twice cancels; once only says so on the pill). |
| `modifier_key.rs` | macOS: one key alone as the shortcut (fn, right ⌘, ⌥ or ⌃), followed by an event tap (it needs Accessibility). |
| `settings.rs`, `history.rs` | JSON files in the app's config and data directories, readable by the user only, written whole (a temporary file, then a rename). A settings file that can't be read is kept as `settings.json.broken` rather than overwritten. |
| `tray.rs`, `commands.rs` | Tray menu and the commands the windows call. |

### Shortcut behaviour

On macOS the shortcut can be one key alone, fn by default. Such a key is
also half of every chord (fn + ←, right ⌘ + C), so it counts as a press only
once held alone for 150 ms, or tapped cleanly; a key or click in between and
nothing starts. Once a dictation has started, nothing cuts it: people click,
type or switch windows while they speak, and a slow chord only records a
moment with no speech, which is never sent. If macOS pauses the key's
listener, Parler asks where the key is rather than assume it came up. A
press that would end a hands-free dictation ends it when the key comes back
up. Only Esc pressed twice throws a recording away: the first press only
says so on the pill, so an Esc meant for another app loses nothing.

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

### Updates

Parler updates itself with Tauri's updater plugin, driven from Rust:

```
 launch + 8 s, then every 6 h ── GET releases/latest/download/latest.json
   newer version? ──► Home: "Parler 0.1.3 is available. [Update]"
 Update ── download the bundle for this install (progress on Home)
        ── check its signature against the public key in tauri.conf.json
        ── wait until no dictation is under way
        ── install, restart
```

- Background checks say nothing unless there is news; a failure is logged
  and the next check tries again. *Check for updates* in Settings shows its
  outcome, failures included, in plain words.
- `latest.json` is written by the release workflow and names one bundle per
  platform and installer (`darwin-aarch64`, `windows-x86_64-nsis`,
  `linux-x86_64-appimage`…). The plugin picks the one matching how Parler was
  installed, so an MSI install stays an MSI install.
- The signature must come from Parler's key and name the announced version,
  or nothing is installed. On Windows the installer runs in passive mode.
- `PARLER_DISABLE_UPDATER=1` turns updates off entirely, for packagers who
  ship Parler through their own channel (Nix, distributions).
- The status (`idle`, `checking`, `upToDate`, `available`, `downloading`,
  `restarting`, `failed`, `disabled`) reaches the window as
  `parler://update` events.

### Why silence is never sent

Whisper models invent text when given silence. Before uploading, `audio::speech`
estimates the room's noise floor, keeps the part of the recording that rises
clearly above it, with a little air around it, and drops recordings with less
than a quarter of a second of speech.

### When the microphone sends nothing

A muted microphone, the wrong one, or one macOS lets Parler open without
hearing records silence; nothing should make the person speak for a minute
to find out.

- **While recording**, `audio::SignalWatch` follows the 30 ms chunks from the
  moment the microphone is live. If none peaks above -65 dBFS for 3 s, the
  pill says *No sound from the microphone* until one does, and recording goes
  on. It reads the peak, not the average: noise peaks about 10 dB above its
  own average, so a working microphone in a silent room (even around
  -75 dBFS) counts, while a muted or unplugged one (zeros, or a few bits)
  doesn't. Once anything has been heard it stops watching, so pauses never
  set it off.
- **At the end**, a recording of a second or more that never rises above one
  step of 16-bit audio (-90 dBFS) is digital silence: instead of *Nothing
  heard*, Parler says it got no sound from the microphone, with a button to
  the system's microphone privacy settings (macOS, Windows).
- **Access** is read again whenever the window comes to the front
  (`microphone_access`, from handy-recorder: macOS and Windows say, Linux
  doesn't). Taken away, Home says so with the same button; a recorder refused
  when it opens reports the same problem.

### Dictations that couldn't be sent

Parler transcribes in the cloud only, so a flaky network must never cost a
dictation.

- **One more try.** A passing failure (no connection, a 5xx) is sent again
  once, 1.5 s later, while the pill keeps writing. Not a timeout (two minutes
  have gone by already) nor a refusal (another 4xx, a plan or fair use limit,
  a missing or refused key): the same request would fail the same way.
- **Kept on disk.** What still fails is kept by `pending.rs` in
  `<app data>/pending/`: the speech as sent (`<id>.wav`, 16 kHz mono) and
  `<id>.json` (when, how long, why it failed), readable by the user only. It
  survives a restart and leaves as soon as it is transcribed or deleted,
  after 7 days at most; past 20 dictations or 50 MB the oldest go first (the
  newest always stays). If the disk refuses it, it lasts in memory until
  Parler quits. The audio only ever goes to the service.
- **Sending it again.** History lists unsent dictations first, each with *Try
  again* and *Delete*; Home's problem banner retries the one that just
  failed. Text sent again from the window is copied (pasting would type into
  Parler) and added to the history.

### A space after each dictation

A pasted dictation that ends a sentence or clause (`. ! ? … : ; ,`, Arabic's
and Hindi's marks, perhaps followed by a closing quote or bracket) gets one
space after it, so the next one doesn't stick to it ("…today.Next"). Text
with Han, kana or Thai is typed as it is: those scripts put no space between
sentences. Only the paste gets it; copy mode, the history and *Copy last
transcript* keep the text as transcribed.

### The clipboard around a paste

Parler pastes through the clipboard, so it saves what is there first and puts
it back once the target app has read the transcript.

- **What is saved.** On macOS, every item with every type, as raw data: a
  screenshot, files copied in Finder or rich text come back as they were.
  File promises (`com.apple.pasteboard.promised-*` and the like) are the
  exception: reading one makes Photos or Mail write the file out, so they are
  skipped, and such an item comes back without them (it can no longer be
  pasted as a file). On Windows and Linux, what arboard reads: a file list, text with its HTML, or
  an image, written back as the same kind. Other formats (RTF, an app's own)
  are lost there, and a cut in a file manager comes back as a copy.
- **Unseen by histories.** The transcript is marked so clipboard managers skip
  it: `org.nspasteboard.TransientType` and `AutoGeneratedType` on macOS, where
  it also stays off Universal Clipboard; `x-kde-passwordManagerHint` on Linux;
  on Windows, out of the clipboard history, the cloud clipboard and other
  monitors. Copy mode and *Copy last transcript* make a plain copy, which
  histories keep.
- **The restore rule.** If the clipboard no longer holds the transcript (on
  macOS, the pasteboard's change count moved), nothing is touched: what was
  copied meanwhile is the user's. Otherwise what was saved is written back,
  and a clipboard that was empty is cleared. Content Parler couldn't read is
  gone either way, so the transcript stays rather than leave the clipboard
  empty. Linux can't tell an empty clipboard from an unreadable one, so the
  transcript stays there too.
- If the keystroke can't be sent, the transcript stays on the clipboard, to
  paste by hand.

## Interface — `src`

Built on the [design system](design-system.md).

| Path | Role |
| --- | --- |
| `App.tsx`, `state.tsx` | Loads settings, status, history and totals once, then follows backend events. |
| `ui/` | The design system's components. |
| `onboarding/` | Welcome → service (Parler Cloud suggested first; your own key or server behind a link) → Parler Cloud sign-in, or the key (checked on paste) → macOS permissions → shortcut → first dictation, each with its animated scene. |
| `pages/` | Home (shortcut, totals, recent dictations), History (unsent dictations first, then every dictation, searchable), Vocabulary, Settings. |
| `components/` | The sidebar, the history list and the unsent dictations, the shortcut recorder, the service form logic, the Parler Cloud sign-in and account, sending the log to support. |
| `overlay/` | The pill: level bars, phases, synthesized sound cues. |
| `i18n/` | 12 languages (`languages.ts`), kept in sync by a test. |
| `preview.tsx` | Dev only: every window in a browser against a simulated backend. |
