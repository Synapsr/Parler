# Contributing

Thanks for helping! Parler aims to stay small and obvious: before adding a
setting, ask whether a better default would do.

## Setup

- [Rust](https://rustup.rs) 1.88 or newer
- [Node.js](https://nodejs.org) 20+ and [pnpm](https://pnpm.io) 10
- Linux only: the [Tauri prerequisites](https://tauri.app/start/prerequisites/#linux) and

  ```sh
  sudo apt install libasound2-dev libpulse-dev libxtst-dev libxkbcommon-dev
  ```

```sh
pnpm install
pnpm tauri dev
```

## Working on the interface alone

`pnpm dev`, then open <http://localhost:1420/preview.html>: the windows run in
a browser against a simulated backend, no Rust build needed. Query parameters
pick the state: `onboarded=1`, `lang=en`, `platform=windows`, `problem=1`,
`state=recording`, `shortcut=Fn`, or `view=overlay&phase=recording`
(`transcribing`, `error`, `empty`).

Parler Cloud: `cloud=signed-out` (or `trial`, `active`, `canceling`,
`past-due`, `inactive`, `canceled`, `offline`) picks it in that state;
`signin=wait` keeps a sign-in waiting (`deny`, `expire`; it is approved by
default); `problem=subscription` (`fairday`, `fairmonth`, `signedout`) shows
its problems, and `view=overlay&phase=error&error=subscription` the pill. To
run the app against a local Parler Cloud service:
`PARLER_CLOUD_URL=http://localhost:3100 pnpm tauri dev`.

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
the `APPLE_*` secrets set, the macOS build is signed and notarized.
