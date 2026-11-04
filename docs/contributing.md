# Contributing

Thanks for helping! Parler aims to stay small and obvious: before adding a
setting, ask whether a better default would do.

## Setup

- [Rust](https://rustup.rs) 1.88 or newer
- [Node.js](https://nodejs.org) 20+ and [pnpm](https://pnpm.io) 10
- [CMake](https://cmake.org) and a C++ compiler (Xcode's command line tools,
  Visual Studio's C++ workload, or GCC): the speech engine for transcription
  on this computer is built from source
- Windows and Linux: the [Vulkan SDK](https://vulkan.lunarg.com) (its
  headers and shader compiler build the engine's GPU backend; Parler runs
  without it), and on Windows SPIRV-Headers
  (`vcpkg install spirv-headers:x64-windows`, its prefix in
  `CMAKE_PREFIX_PATH`)
- Linux only: the [Tauri prerequisites](https://tauri.app/start/prerequisites/#linux) and

  ```sh
  sudo apt install libasound2-dev libpulse-dev libxtst-dev libxkbcommon-dev
  ```

The first build compiles the engine (a few minutes; on Windows and Linux,
once per CPU instruction set); later ones reuse it.

```sh
pnpm install
pnpm tauri dev
```

## Working on the interface alone

`pnpm dev`, then open <http://localhost:1420/preview.html>: the windows run in
a browser against a simulated backend, no Rust build needed. Query parameters
pick the state: `onboarded=1`, `lang=en`, `platform=windows`, `problem=1`,
`state=recording`, `shortcut=RightCommand` (`Fn`, `RightOption`,
`RightControl`, or an accelerator), `problem=key` (a key alone waiting for
Accessibility), `update=available` (`downloading`, `waiting`, `restarting`,
`error`, `offline`, `uptodate`, `disabled`), or `view=overlay&phase=recording`
(`transcribing`, `error`, `empty`). `w=780` narrows the window to its
smallest width.

Parler Cloud: `cloud=signed-out` (or `trial`, `active`, `canceling`,
`past-due`, `inactive`, `canceled`, `offline`) picks it in that state;
`signin=wait` keeps a sign-in waiting (`deny`, `expire`; it is approved by
default); `problem=subscription` (`fairday`, `fairmonth`, `signedout`) shows
its problems, and `view=overlay&phase=error&error=subscription` the pill. To
run the app against a local Parler Cloud service:
`PARLER_CLOUD_URL=http://localhost:3100 pnpm tauri dev`.

The microphone: `problem=nosound` (a recording of digital silence) or
`problem=micdenied`, `mic=denied` (access taken away, as read when the window
comes to the front), and on the pill `view=overlay&phase=recording&hint=noSound`
or `view=overlay&phase=error&error=nosound`. Unsent dictations:
`onboarded=1&pending=3` lists them in History (up to 4); sending one again
succeeds unless `retry=fail`.

On this computer: `local=missing` (or `downloading`, `ready`, `failed`)
puts its model in that state, and once onboarded picks it as the service;
`localModel=whisper` uses Whisper Large v3 Turbo instead of the recommended
model (Parakeet Ultra for the default French); `localError=offline` (`disk`,
`checksum`, `other`) says why `failed`; `download=fail` stops a download a
third of the way; `backend=CPU` (or `Vulkan · AMD Radeon 780M`;
`Metal · Apple M4` by default) is what it runs on, and `loaded=0` leaves the
ready model unloaded, so Settings says where it will run. Downloads run in about eight seconds.
`problem=modelMissing` or `problem=localFailed` shows its problems, and
`view=overlay&phase=error&error=modelMissing` the pill. To reach the
download step of the first-run flow: `lang=en&clicks=Get started,on this computer`.

The speech models can be tried without the app: the ignored test
downloads one (into a folder it keeps) and transcribes 16 kHz mono WAV
files, printing load and run times and where it ran.

```sh
cd src-tauri
PARLER_MODELS_DIR=~/parler-models PARLER_TEST_WAVS=fr.wav,en.wav \
  cargo test --release --lib real_download_and_transcription -- --ignored --nocapture
```

`PARLER_TEST_MODEL=whisper-large-v3-turbo` picks Whisper, `PARLER_TEST_CPU=1`
keeps off the GPU and `PARLER_TEST_LANGUAGE=fr` passes a language.

## Checks

CI runs these on every pull request; run them before pushing:

```sh
pnpm typecheck && pnpm test && pnpm build
cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Conventions

- Commits follow [Conventional Commits](https://www.conventionalcommits.org):
  `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`. Say why, not only what.
- Every interface string lives in all 12 files of `src/i18n/locales`
  (`en.json` is the source); a test fails if a key or a `{placeholder}` is
  missing from one of them. A new language is a JSON file plus a line in
  `src/i18n/languages.ts`.
- New providers go in `PROVIDERS` in `src-tauri/src/providers.rs`, with a
  tagline under `providers` in every locale file.

## Releasing

Bump the version in `package.json`, `src-tauri/Cargo.toml` and
`src-tauri/tauri.conf.json`, then push a `vX.Y.Z` tag. The release workflow
builds macOS (universal), Windows and Linux bundles and publishes them. With
the `APPLE_*` secrets set, the macOS build is signed and notarized: Tauri
notarizes the app itself, then the workflow notarizes the DMG.

### In-app updates

Installed copies update themselves from GitHub Releases (see
[architecture](architecture.md#updates)). For that, every release carries
bundles signed with Parler's updater key and a `latest.json` describing them:

- The signing key lives in two repository secrets,
  `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; its
  public half is `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`. Keep
  the key: copies already installed only accept updates signed with it.
- Signatures record the version they were made for, and Parler refuses an
  update whose signature and announced version disagree
  (`requireSignedVersion`). Build releases with the Tauri CLI from this repo,
  which writes that version.
- The workflow uploads the update bundles (`Parler-macOS.app.tar.gz`, the
  Windows installers, the AppImage and, when signed, the `.deb`) with their
  `.sig` files, writes `latest.json` pointing at this release's assets, and
  covers them all in `SHA256SUMS` and the build attestation. Installed copies
  read it at `releases/latest/download/latest.json`, so only a full release
  (not a draft or prerelease) reaches them.
- A local release build (`pnpm tauri build`) signs the update bundles too, so
  it needs `TAURI_SIGNING_PRIVATE_KEY` (and its password) in the environment.
  Without the key, build without them:
  `pnpm tauri build --config '{"bundle":{"createUpdaterArtifacts":false}}'`.
  `pnpm tauri dev` and the preview never sign anything.
- Packagers who update Parler themselves (Nix, distributions) set
  `PARLER_DISABLE_UPDATER=1`: Parler then never checks for updates, and its
  settings say updates are handled outside it.
