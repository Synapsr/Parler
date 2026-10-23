<div align="right">

🌐 **Language**: [Français](README.fr.md) | **English**

</div>

<div align="center">

<img src="src-tauri/icons/icon.png" width="112" alt="Parler logo" />

# Parler

### Speak. Your words appear wherever you type.

**Free, open-source dictation for macOS, Windows and Linux — powered by the speech-to-text API you choose.**

[![Latest Release](https://img.shields.io/github/v/release/Synapsr/Parler?style=for-the-badge&logo=github&label=Release)](https://github.com/Synapsr/Parler/releases/latest)
[![License](https://img.shields.io/badge/License-MIT-blue?style=for-the-badge)](LICENSE)
[![Built with Tauri](https://img.shields.io/badge/Tauri-2-FFC131?style=for-the-badge&logo=tauri&logoColor=black)](https://tauri.app/)

[⬇️ Download](#️-download) • [✨ Features](#-features) • [🔌 Providers](#-providers) • [🏗️ Architecture](docs/architecture.md) • [🤝 Contributing](docs/contributing.md)

</div>

---

## 💡 What is Parler?

Hold a shortcut, speak, let go: Parler sends your voice to **Groq**, **OpenAI**, **Fireworks** or any **Whisper-compatible** server, and types the text where your cursor is. In your email, your code editor, your chat — anywhere.

No account, no subscription, no model to download. Bring your own API key; it stays on your computer.

```
   ⌥ Space held  ──►  🎙️ your voice  ──►  ☁️ Whisper API  ──►  ✍️ text in the focused app
                        (16 kHz WAV)       (Groq, OpenAI…)       (your clipboard is restored)
```

---

## ✨ Features

|            🎯 **One shortcut**            |          ⚡ **Fast**          |          🔒 **Yours**          |
| :--------------------------------------: | :--------------------------: | :---------------------------: |
| Hold to talk, or tap to start and stop.  | About a second with Groq.    | Your key, your provider. MIT. |

|          🧭 **Guided setup**           |      🌍 **Any language**      |      🖥️ **Everywhere**       |
| :-----------------------------------: | :--------------------------: | :-------------------------: |
| Pick a provider, paste a key, test it. | Auto-detect or pick yours.   | macOS, Windows and Linux.   |

- **Pastes where you type**, then puts your clipboard back the way it was.
- **Never sends silence**: Whisper invents text on silent audio ("Thank you.", "Sous-titres réalisés par…"), so empty recordings stay on your machine.
- **Vocabulary hints** so names, brands and jargon come out spelled right.
- **Recent dictations** to copy again if one landed in the wrong window, and **retry** when the network failed.
- **Esc cancels** a dictation in progress.
- A discreet **pill** at the bottom of the screen shows your voice level — and never steals focus.
- Interface in **English and French**.

---

## ⬇️ Download

Grab the latest build from the [Releases page](https://github.com/Synapsr/Parler/releases/latest):

| Platform | File |
| --- | --- |
| macOS (Apple Silicon & Intel) | `Parler_x.y.z_universal.dmg` |
| Windows 10/11 | `Parler_x.y.z_x64-setup.exe` or `.msi` |
| Linux | `.AppImage`, `.deb` or `.rpm` |

Then open Parler and follow the three-step setup.

<details>
<summary><b>macOS says the app can't be opened</b></summary>

Builds are notarized when the release pipeline has Apple credentials. If yours isn't, right-click **Parler.app → Open**, or run:

```sh
xattr -dr com.apple.quarantine /Applications/Parler.app
```

Parler asks for two permissions: **Microphone** (to hear you) and **Accessibility** (to paste for you).
</details>

<details>
<summary><b>Windows SmartScreen warns about an unknown publisher</b></summary>

Click **More info → Run anyway**. Every release ships a `SHA256SUMS` file and a [build attestation](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations) you can verify with `gh attestation verify <file> --repo Synapsr/Parler`.
</details>

<details>
<summary><b>Linux and Wayland</b></summary>

Global shortcuts work on X11. On Wayland, bind a shortcut in your desktop settings to the command:

```sh
parler --toggle
```

Pasting on Wayland uses `wtype` or `ydotool` when installed.
</details>

---

## 🔌 Providers

| Provider | Default model | Get a key |
| --- | --- | --- |
| **Groq** (recommended) | `whisper-large-v3-turbo` | [console.groq.com/keys](https://console.groq.com/keys) |
| **OpenAI** | `gpt-4o-mini-transcribe` | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) |
| **Fireworks** | `whisper-v3-turbo` | [app.fireworks.ai](https://app.fireworks.ai/settings/users/api-keys) |
| **Custom** | any | Your own server speaking the OpenAI audio API: [speaches](https://github.com/speaches-ai/speaches), [LocalAI](https://localai.io), … |

Any other model the provider serves can be typed in the settings. Prices and free tiers are set by each provider.

---

## 🔐 Privacy

- Audio is recorded only while you dictate, and sent only to the provider you picked.
- Your API key and settings live in a local file readable only by your user account.
- The last 50 transcripts are kept locally so you can copy them again; clear them anytime.
- No analytics, no telemetry, no Parler server.

---

## 🛠️ Build from source

Requires [Rust](https://rustup.rs) 1.88+, [Node.js](https://nodejs.org) 20+ and [pnpm](https://pnpm.io).

```sh
git clone https://github.com/Synapsr/Parler.git
cd Parler
pnpm install
pnpm tauri dev      # run in development
pnpm tauri build    # build an installer for your platform
```

On Linux, install the [Tauri prerequisites](https://tauri.app/start/prerequisites/#linux) plus `libasound2-dev libpulse-dev libxtst-dev libxkbcommon-dev`. See [docs/contributing.md](docs/contributing.md).

---

## 🙏 Credits

Parler stands on [Handy](https://github.com/cjpais/Handy), the open-source dictation app by CJ Pais: microphone capture comes from its [`handy-recorder`](https://crates.io/crates/handy-recorder) crate, and the focus-safe overlay and layout-aware paste are adapted from it. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## 📄 License

[MIT](LICENSE) © Synapsr
