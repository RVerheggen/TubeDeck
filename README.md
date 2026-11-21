# TubeDeck

A Windows desktop download manager for [yt-dlp](https://github.com/yt-dlp/yt-dlp), built with React + TypeScript, Tauri 2 and Rust.

> **Status: milestones M0–M2 done.** Real downloads work, with live progress, cancel and retry. The queue and history are not saved between sessions yet (M4). See [PLAN.md](PLAN.md) for the roadmap.

## Features (current)

- **Download engine** (Rust): runs yt-dlp with no console window and parallel downloads up to your limit (changes apply immediately). Shows live percent, speed, ETA, stream (video/audio) and post-processing stage. Cancel stops the whole process tree (yt-dlp, its Python child and ffmpeg) and deletes the job's temp files. Failed jobs can be retried, and each job keeps a log of yt-dlp's output.
- **Downloads view**: paste one or more URLs (Ctrl+Enter to add), cards with thumbnails, stats, and a live command preview.
- **Configure view**: video or audio mode, quality cap, container, preferred codec, audio format and quality, subtitles, SponsorBlock, embedding toggles, output folder and filename template, fragments, retries, rate limit, browser cookies, and free-form custom arguments.
- **Live command preview**: built by the same Rust function the engine will run (`src-tauri/src/args.rs`), shown as a copy-pasteable PowerShell command, with warnings for incompatible or dangerous options (e.g. `--exec`).
- **Presets**: 5 built-ins plus your own (save, rename, overwrite, delete).
- **Themes**: dark, light, or follow Windows. Fonts are bundled, so the app needs no network to render.
- **Engine detection**: finds yt-dlp, FFmpeg and Deno (configured path → managed folder → PATH) and shows their versions.
- **Logs view**: the application log, with a filter.

## Prerequisites (Windows)

| Tool | Notes |
|---|---|
| Node.js LTS (22+) | `winget install OpenJS.NodeJS.LTS` |
| Rust (stable, MSVC) | `winget install Rustlang.Rustup` |
| Microsoft C++ Build Tools | Visual Studio Installer → "Desktop development with C++" |
| WebView2 Runtime | Already included in Windows 11 |
| yt-dlp | `yt-dlp.exe` from the [releases page](https://github.com/yt-dlp/yt-dlp/releases), on PATH or set in Settings |
| FFmpeg | Needed for merging, audio conversion and embedding |
| Deno 2.3+ | Needed for full YouTube support (yt-dlp's [EJS](https://github.com/yt-dlp/yt-dlp/wiki/EJS) challenge solver) |

From M6 on, TubeDeck will download yt-dlp, FFmpeg and Deno for you.

## Develop

```powershell
npm install
npm run tauri dev
```

`npm run dev` on its own serves the UI in a normal browser with a mocked backend, which is handy for styling work.

## Test

```powershell
npm test
```

```powershell
cd src-tauri; cargo test; cargo clippy --all-targets -- -D warnings
```

## Build the installer

```powershell
npm run tauri build
```

Installers are written to `src-tauri/target/release/bundle/nsis/` and `.../msi/`. The installers are unsigned, so Windows SmartScreen will warn on first run.

## Where things are stored

| What | Location |
|---|---|
| Settings and user presets | `%APPDATA%\com.tubedeck.desktop\` |
| Application log | `%LOCALAPPDATA%\com.tubedeck.desktop\logs\tubedeck.log` |
| Managed binaries (put `yt-dlp.exe` / `deno.exe` here; auto-install in M6) | `%LOCALAPPDATA%\com.tubedeck.desktop\bin\` |
| Partial downloads (per job, cleared on cancel/completion and at startup) | `%LOCALAPPDATA%\com.tubedeck.desktop\tmp\` |

## Project layout

```
src/                     React UI
  components/            panels, cards, sidebar, command preview, …
  views/                 Downloads, Configure, Settings
  store/useAppStore.ts   Zustand store
  lib/                   types (mirror of Rust), api wrapper, browser mock, formatting
src-tauri/src/
  options.rs             DownloadOptions model
  args.rs                options → yt-dlp argv (single source of truth) + tests
  presets.rs, settings.rs, jsonfile.rs
  binaries.rs            locate yt-dlp / FFmpeg / Deno, read versions
  commands.rs            Tauri commands
  engine/
    mod.rs               queue, scheduler, cancel/retry, batched `jobs://updated` events
    runner.rs            spawn yt-dlp, merge stdout/stderr, apply parsed lines
    progress.rs          parse TubeDeck's progress templates (+ real-output fixture test)
    winjob.rs            Windows Job Object (kill the whole process tree)
```
