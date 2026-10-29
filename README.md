# Bubbe

Desktop app for macOS and Windows. A salesperson drops in a recording of a sales call; Bubbe
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

App data folder: `~/Library/Application Support/se.bubbe.desktop` on macOS,
`%APPDATA%\se.bubbe.desktop` on Windows.

## AI: tips and chat

Bubbe can use any of three AI sources. Under **Inställningar → AI för säljtips och chatt** the
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

On first start Bubbe asks for your name and gender (only used to pick "hon/han/hen" when the
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

### Admin: reviewing other people's calls

Onboarding (and Inställningar → Om dig) asks what you use Bubbe for: **Jag säljer** (coaching, as
above) or **Jag granskar samtal** (admin). An admin gets a *review* instead of sales tips:

- **Verdict:** Godkänt, Bör ses över or Allvarligt, with a one-sentence reason, shown as a banner and as
  a coloured dot in the call list.
- **Flaggat:** breaks of the admin's rules ("Regler att flagga", one per line, numbered so a flag can
  say "regel 3"), each with severity, explanation and an exact, verified quote. Without rules the AI
  judges against general good practice.
- **Saknas i policyn:** risky things no rule covers, each with a suggested new rule.
- **Allmänna villkor:** an admin can upload the company's terms (PDF, Word or text; `src-tauri/src/terms.rs`).
  Only the text is kept, locally, capped at 60,000 characters. Reviews then flag anything the seller
  says that contradicts them (price, contract length, notice period, cancellation rights). Scanned
  PDFs contain no text and are refused with a message.
- **Fråga AI** drafts the rules from a short description of the business; the admin edits the result.

The admin's own name is never used for the seller; the seller is named only if the call says it.
Each stored analysis records its kind, so old calls keep their sales tips until reviewed again,
and the chat answers as a reviewer for reviewed calls.

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

- `Bubbe_<version>_aarch64.dmg` (Apple Silicon, Metal)
- `Bubbe_<version>_x64.dmg` (Intel, CPU; built on an Intel runner)
- `Bubbe_<version>_x64-setup.exe` (NSIS) and `Bubbe_<version>_x64_sv-SE.msi` (Vulkan with CPU fallback, `vulkan-1.dll` bundled)

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

- *Mac:* Öppna `.dmg`-filen och dra Bubbe till Program. Första gången: högerklicka på
  Bubbe i Program och välj **Öppna**, och sedan **Öppna** igen. Om macOS säger att appen
  "är skadad" eller inte kan öppnas: gå till **Systeminställningar → Integritet och säkerhet**, scrolla
  ner och klicka **Öppna ändå**.
- *Windows:* Om "Windows skyddade datorn" (SmartScreen) visas: klicka **Mer information** och sedan
  **Kör ändå**.

### Updates

In-app updates are built with the `updater` feature and `src-tauri/tauri.updater.conf.json`, which
holds the public key and points at `https://github.com/LuckyLBP/saljtipset/releases/latest/download/latest.json`.
The app checks at start and every six hours, shows a banner when a new version exists, and has
**Sök efter uppdateringar** under Inställningar. Updates are only installed if they are signed with
the private key.

- The key pair was made with `pnpm tauri signer generate`; the private key is in
  `~/.tauri/saljtipset.key` on the developer's Mac. Keep a backup somewhere safe: without it,
  installed apps can't be updated anymore.
- CI needs the secrets `TAURI_SIGNING_PRIVATE_KEY` (the key file's contents) and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty), and the repository variable `ENABLE_UPDATER=true`.
- To release: bump `version` in `src-tauri/tauri.conf.json`, `package.json` and `src-tauri/Cargo.toml`,
  push a tag `vX.Y.Z`, then **publish** the draft release. Apps pick it up from `latest.json`.
- A build made without the updater (like the very first 0.1.0) can't update itself; install the
  first updater-enabled version by hand once.

## Brand

The app was called Säljtipset before 0.2.0. On first start Bubbe moves the old data folder
(`com.saljtipset.app`) and the Keychain entry over, so models, calls and the API key carry on.

The brand direction is "Humlan": honey yellow `#FFC53D`, charcoal `#1F1A17`, raspberry
`#E5486B` on cream, with Baloo 2 (bundled, OFL) for the wordmark and headings. Honey is only
used as a fill; readable accent text uses a dark amber in light mode.

The mascot is **Bubbe the bumblebee** with a headset (`src/mascot.tsx`), drawn as SVG and
animated with CSS. Moods: idle, listening (nods while transcribing), thinking (antennae wiggle,
a spark circles), cheer (tips ready), hello, surprised (a file dragged over), oops (errors),
sleepy (after 90 s idle), flying (downloading) and love (you talked less than the customer).
The larger Bubbe follows the cursor with its eyes and giggles when clicked. Everything holds
still with the OS "Reduce motion" setting. The app icon is Bubbe on a charcoal tile.

## Windows notes

- The Visual C++ runtime is linked into `bubbe.exe` (`src-tauri/.cargo/config.toml`), so it starts
  on a PC without the VC++ Redistributable (no "MSVCP140.dll saknas"). The release build checks
  the exe's imports and fails if a VC++ DLL sneaks back in.
- Building on Windows yourself: set `CMAKE_C_FLAGS_RELEASE` and `CMAKE_CXX_FLAGS_RELEASE` to
  `-O2 -Ob2 -DNDEBUG` first (CI does this in the "Windows-byggmiljö" step). With Visual Studio the
  `cmake` crate otherwise replaces CMake's release flags without `/O2`, and whisper.cpp ends up
  unoptimized and many times slower.
- The Vulkan loader is delay-loaded and `vulkan-1.dll` ships next to the exe, so the app starts
  even on a PC with no Vulkan runtime. Without any GPU, ggml finds no Vulkan device and
  transcribes on the CPU.
- If a broken graphics driver crashes during GPU start-up, the next launch notices, switches to
  the CPU and says so. The GPU can be turned back on under **Inställningar → Använd grafikkortet**.
- `ffmpeg` is not bundled. Without it, webm/opus files show a message asking for mp3, m4a or wav.
  To bundle it, place `ffmpeg.exe` / `ffmpeg` next to the app binary (mind the LGPL/GPL terms of the
  build you choose).
