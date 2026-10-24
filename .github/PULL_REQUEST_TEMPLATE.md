## What and why

<!-- What does this change, and why? Link the issue it closes, e.g. "Closes #12". -->

## How it was tested

<!-- Platforms you tried it on (macOS, Windows, Linux X11/Wayland), provider, and what you checked by hand. -->

## Screenshots

<!-- For interface changes: before and after, light and dark if it matters. Delete this section otherwise. -->

## Checklist

- [ ] The title follows [Conventional Commits](https://www.conventionalcommits.org) (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`).
- [ ] `pnpm typecheck && pnpm test && pnpm build` pass.
- [ ] `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test` pass in `src-tauri`.
- [ ] New interface strings are in all 12 files of `src/i18n/locales` (`en.json` is the source).
- [ ] No new setting where a better default would do.
- [ ] Docs updated if behaviour changed (`README.md`, `README.fr.md`, `docs/`).
