# Säljtipset

Desktop app for macOS and Windows. A salesperson drops in a recording of a sales call; Säljtipset
transcribes it **on the computer** with [KB-Whisper](https://huggingface.co/KBLab) (KBLab's
Swedish Whisper), shows the transcript with timestamps, and sends **only the text** to an AI
(Anthropic API key, or the user's own Claude or ChatGPT subscription) for selling points and a chat
about the call. All UI copy is Swedish.

Tauri 2 (Rust) + React + TypeScript + Vite + Tailwind. whisper.cpp via `whisper-rs`
(Metal on Apple Silicon, Vulkan on Windows, CPU everywhere else).

## Run in development

```bash
pnpm install
```

```bash
pnpm tauri dev --features gpu-metal
```

On Windows use `--features gpu-vulkan`; on an Intel Mac leave the feature out (CPU only).
On first start the app asks you to download a speech model (see [Models](#models)).

### Build requirements

| | macOS | Windows |
|---|---|---|
| Rust | stable via [rustup](https://rustup.rs) | stable via rustup (MSVC toolchain) |
| Node | 22+ and pnpm (`corepack enable`) | same |
| C/C++ | Xcode Command Line Tools (`xcode-select --install`) | Visual Studio Build Tools 2022 with "Desktop development with C++" |
| CMake | `brew install cmake` | included with VS Build Tools, or `winget install Kitware.CMake` |
| GPU | nothing extra (Metal is built in) | [Vulkan SDK](https://vulkan.lunarg.com/sdk/home#windows) (sets `VULKAN_SDK`), then run `scripts\fetch-vulkan-loader.ps1` once before `tauri build` |
| WebView | built in | WebView2 (preinstalled on Windows 10/11) |

Release builds for other people's machines: set `GGML_NATIVE=OFF` on x86 (Windows, Intel Mac) so
whisper.cpp targets a portable AVX2 baseline instead of the build machine's CPU. CI does this.

### Tests

```bash
cd src-tauri
cargo test --features gpu-metal
```

Unit tests cover 16 kHz mono conversion and resampling, stereo detection/splitting and merging,
speech-turn detection, the copy format, tips JSON validation and the quote check, the chat prompt,
export and the database. `tests/pipeline.rs` decodes the Swedish fixtures in `tests/fixtures` (wav, mp3, m4a,
webm) and, when a model is available, runs the full transcription path on a mono and a stereo call:

```bash
SALJTIPSET_TEST_MODEL=/path/to/kb-whisper-small/ggml-model-q5_0.bin cargo test --features gpu-metal --test pipeline
```

The fixtures were generated with macOS's Swedish voice (Alva). `sv_stereo.wav` has the seller on the
left channel (0 s and 10.5 s) and the customer on the right (4.5 s and 16.5 s); the test checks
both the speaker labels and that each line lands at its real time.

To check speed and memory on a long file:

```bash
cargo run --release --features gpu-metal --example transcribe -- <model.bin> <audio> [--cpu]
```

## How it works

| Step | Where | Notes |
|---|---|---|
| Decode | `src-tauri/src/audio.rs` | symphonia (wav, mp3, m4a/mp4/aac, ogg/vorbis, flac), streamed and resampled to 16 kHz f32 on the fly. Anything symphonia can't read (typically **webm/opus**) goes through `ffmpeg` if one is found next to the app or on the system. |
| Stereo | `audio.rs` | Stereo files whose sides are copies of each other, or where one side is silent, are treated as mono. Otherwise left = "Talare 1", right = "Talare 2". |
| Speech turns | `src-tauri/src/vad.rs` | Each channel is cut into speech turns and whisper runs once per turn. Without this, whisper misplaces timestamps after long silences (each side of a stereo call is silent while the other talks) by up to several seconds, which breaks merging by time. It also stops whisper from hallucinating text into silence. |
| Transcribe | `src-tauri/src/transcribe.rs` | Language fixed to `sv`. Beam search (5) on GPU, greedy on CPU. Segments stream to the UI as Tauri events; cancel aborts within a second. |
| Tips | `src-tauri/src/claude.rs`, `tips.rs` | Messages API over `reqwest`, Swedish prompt, structured JSON output, validated with one retry. Every point's quote must be found in the transcript (ignoring case and punctuation), otherwise it is dropped; its timestamp is replaced by the real one. |
| Storage | `src-tauri/src/db.rs` | SQLite in the app data folder. The audio file is read where it is and never copied. The API key is in the OS keychain (`secrets.rs`). |

## Models

On first start the app shows "Ladda ner talmodellen". Files are the `ggml-model-q5_0.bin` files
from the KBLab repos (checked against the repo listings; there is also an unquantized
`ggml-model.bin` in each):

| Choice | Repo | Size | Default when |
|---|---|---|---|
| Bäst | `KBLab/kb-whisper-large` | 1.08 GB | Apple Silicon or a Vulkan GPU with ≥ 3 GB, and ≥ 8 GB RAM |
| Snabb | `KBLab/kb-whisper-medium` | 539 MB | – |
| Lätt | `KBLab/kb-whisper-small` | 175 MB | no usable GPU |

Downloads resume after a cancel or a dropped connection, are verified against the SHA256 in Hugging
Face's metadata, and are stored in the OS app data folder (`models/`). To switch models, open
**Inställningar → Talmodell** and download another one or press **Använd** on an installed one;
**Ta bort** deletes a model from disk.

App data folder: `~/Library/Application Support/com.saljtipset.app` on macOS,
`%APPDATA%\com.saljtipset.app` on Windows.

## AI: tips and chat

Säljtipset can use any of three AI sources. Under **Inställningar → AI för säljtips och chatt** the
user picks one, or leaves it on **Automatiskt** (API key first, then a Claude subscription, then
ChatGPT):

| Source | How | Text goes to |
|---|---|---|
| Anthropic API key | Pasted in Settings, tested, stored in the macOS Keychain / Windows Credential Manager. Messages API over `reqwest`. | Anthropic |
| Claude subscription (Pro/Max) | The **Claude Code** CLI, if installed and logged in (`claude auth status`). | Anthropic |
| ChatGPT subscription | The **Codex** CLI, if installed and logged in (`codex login status`). | OpenAI |

The CLIs are found in the usual install folders (native installer, Homebrew, npm, nvm, bun, volta;
`%APPDATA%\npm` on Windows) since GUI apps don't get the shell's PATH. They run headless in an empty
temporary folder with every tool, MCP server, setting file and slash command turned off
(`claude -p --tools "" --strict-mcp-config --setting-sources "" …`; `codex exec --sandbox read-only
--ephemeral --ignore-user-config …`), and the prompt goes in on stdin. They see only the text the
app pipes in. "Sök igen" in Settings re-runs detection after installing or logging in.

The model setting (default `claude-sonnet-5-5`) applies to the API key and Claude Code; Codex uses
its own default. With the API key, server-side refusal fallback (`fallbacks: "default"`) is on, and
the transcript is sent as a cached prompt block so follow-up chat questions don't pay for it again.

**Chat.** In a call, "Fråga om samtalet" opens a small chat window ("Hur var mitt samtal?", "Skulle
jag kunnat säga detta istället?"). The AI gets the transcript, the speaker roles and any tips; answers
are in Swedish and cite `[mm:ss]` timestamps that jump to the line when clicked. History is saved
per call in SQLite and deleted with the call.

**Subscriptions and terms.** Using a personal Claude or ChatGPT subscription through its CLI is
meant for that person's own use on their own computer. Before rolling this out to a sales team,
check Anthropic's and OpenAI's current terms for subscriptions used by another application; the API
key (or the Prospex endpoint below) is the route made for products.

Later, if salespeople shouldn't handle keys, point `claude.rs` at an endpoint in Prospex that holds
the key. The audio and transcription still stay on the computer; only the transcript text goes out.

## Network

The app only makes these requests:

1. `huggingface.co` – model metadata and the model download, when the user picks a model.
2. The chosen AI source, when tips are generated or the user chats: `api.anthropic.com` directly
   (API key), or Anthropic / OpenAI through the Claude Code / Codex CLI. Only transcript, tips and
   chat text are sent.
3. Only in builds with the updater enabled: the update manifest URL you configure.

## Profile, names and coaching

On first start Säljtipset asks for your name and gender (only used to pick "hon/han/hen" when the
AI writes about you), and optionally your company and what you sell. It's stored in the local
database and editable under **Inställningar → Om dig**.

- **Who's who.** Once tips exist, the transcript shows your first name instead of "Talare 1" or
  "Säljare", and the customer's name if it was said in the call (the AI's guess is dropped unless
  the name actually occurs in the transcript). Copy and export use the same names.
- **Call stats.** Översikt shows how much you talked, how many questions you and the customer asked,
  and your longest monologue, counted locally from the transcript (exact for stereo, based on the
  AI's guess for mono).
- **Sales playbook.** Tips and chat follow a built-in Swedish playbook (`src-tauri/src/sales.rs`):
  discovery from situation to problem to consequence to value, value before price, objections
  handled by acknowledging, exploring and answering with the customer's own goals, concrete next
  steps with a date, and never inventing proof. The objection approach is adapted from Anthropic's
  open-source sales plugin ([knowledge-work-plugins](https://github.com/anthropics/knowledge-work-plugins),
  Apache-2.0). Your own "Ditt erbjudande" text is included so the tips relate to what you sell.
- **Theme.** Inställningar → Utseende: Som datorn, Ljust or Mörkt.

## Download page (GitHub Pages)

`site/` is a one-page download site. `.github/workflows/pages.yml` publishes it on every push to
`main` that touches `site/`, and again whenever a release is published. The buttons look up the
newest release through the GitHub API and link straight to the `.dmg` / `setup.exe`, so nothing
needs editing per version.

To turn it on: in the repository settings, under **Pages**, set the source to **GitHub Actions**.
Releases from the build workflow start as drafts; the page only sees a release once you
**publish** it. Replace `site/screenshot.jpg` with a fresh screenshot whenever the app's look changes.

## Distribution

Push a tag like `v0.1.0`. `.github/workflows/build.yml` runs the tests on macOS and Windows, then
builds:

- `Säljtipset_<version>_aarch64.dmg` (Apple Silicon, Metal)
- `Säljtipset_<version>_x64.dmg` (Intel, CPU; built on an Intel runner)
- `Säljtipset_<version>_x64-setup.exe` (NSIS) and `Säljtipset_<version>_x64_sv-SE.msi` (Vulkan with CPU fallback, `vulkan-1.dll` bundled)

and attaches them to a **draft** GitHub release. Running the workflow by hand keeps them as workflow
artifacts instead.

### Code signing

Both platforms build fine unsigned. Unsigned apps get a warning the first time they are opened.

**macOS (Developer ID + notarization).** Requires an Apple Developer Program membership. Add these
repository secrets and tauri-action signs and notarizes automatically:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the exported "Developer ID Application" `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Ditt Företag AB (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Apple ID, an app-specific password, and the team ID, for notarization |

**Windows (Authenticode).** Add `WINDOWS_CERTIFICATE` (base64 of the `.pfx`) and
`WINDOWS_CERTIFICATE_PASSWORD`. The workflow imports it and signs the `.exe` and `.msi`. An EV
certificate gets past SmartScreen immediately; a standard OV certificate builds reputation over time.
Certificates on a hardware token or in a cloud HSM need a `signCommand` in `tauri.windows.conf.json`
instead.

**Till säljarna: öppna en osignerad version**

- *Mac:* Öppna `.dmg`-filen och dra Säljtipset till Program. Första gången: högerklicka på
  Säljtipset i Program och välj **Öppna**, och sedan **Öppna** igen. Om macOS säger att appen
  "är skadad" eller inte kan öppnas: gå till **Systeminställningar → Integritet och säkerhet**, scrolla
  ner och klicka **Öppna ändå**.
- *Windows:* Om "Windows skyddade datorn" (SmartScreen) visas: klicka **Mer information** och sedan
  **Kör ändå**.

### Updates (optional)

Off by default. To turn on in-app updates:

1. `pnpm tauri signer generate -w ~/.tauri/saljtipset.key` and keep the private key safe.
2. Put the public key and your release URL in `src-tauri/tauri.updater.conf.json`.
3. Add the secrets `TAURI_SIGNING_PRIVATE_KEY` (and `_PASSWORD`) and the repository variable
   `ENABLE_UPDATER=true`.

CI then builds with `--features updater`, signs the update bundles and uploads `latest.json`. The app
checks on start and shows a banner when a new version exists.

## Windows notes

- The Vulkan loader is delay-loaded and `vulkan-1.dll` ships next to the exe, so the app starts
  even on a PC with no Vulkan runtime. Without any GPU, ggml finds no Vulkan device and
  transcribes on the CPU.
- If a broken graphics driver crashes during GPU start-up, the next launch notices, switches to
  the CPU and says so. The GPU can be turned back on under **Inställningar → Använd grafikkortet**.
- `ffmpeg` is not bundled. Without it, webm/opus files show a message asking for mp3, m4a or wav.
  To bundle it, place `ffmpeg.exe` / `ffmpeg` next to the app binary (mind the LGPL/GPL terms of the
  build you choose).
