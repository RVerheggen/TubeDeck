# TubeDeck

A Windows desktop download manager for [yt-dlp](https://github.com/yt-dlp/yt-dlp), built with React + TypeScript, Tauri 2 and Rust.

Paste a video, playlist or channel URL, pick a preset, and TubeDeck downloads in parallel with live progress, pause/resume and automatic retries. It sets up yt-dlp, FFmpeg and Deno for you and keeps yt-dlp current.

**Status:** feature-complete for 0.1.0 (milestones M0–M7 in [PLAN.md](PLAN.md)). See [CHANGELOG.md](CHANGELOG.md).

## Install

1. Download `TubeDeck_<version>_x64-setup.exe` from the repository's GitHub releases (or build it yourself, see below).
2. Run it. It installs for your user only and needs no administrator rights. The installer isn't code-signed yet, so Windows SmartScreen warns the first time: click **More info → Run anyway**.
3. On first start, click **Install everything** to set up yt-dlp, FFmpeg and Deno (about 155 MB, verified against their published checksums).

## Features

**Downloading**
- Parallel downloads up to a limit you choose (changes apply immediately), with live percent, speed, ETA, stream (video/audio) and post-processing stage.
- Pause and resume (continues from the partial file), cancel (stops yt-dlp, its helper process and FFmpeg, and removes partial files), retry, and drag-and-drop reordering. Pause all / Resume all.
- Automatic retry for temporary failures (network errors, HTTP 403/429/5xx) after 5s, 20s, 60s… (0–5 attempts). Permanent errors fail right away with a short explanation of what to do.
- Each download keeps a log of yt-dlp's output.

**Playlists**
- Playlist and channel URLs open a picker listing every item (thumbnail, title, duration). Select with a filter, ranges like `1-10, 15, 20-`, or shift+click, and open nested playlists.
- Each item becomes its own download, grouped under a collapsible playlist header, saved as `<Playlist>\NN - <title>`. "Pick items…" inspects any URL first.

**Configuration**
- Video or audio mode, quality cap, container, preferred codec, audio format and quality, subtitles, SponsorBlock, embedding, output folder and filename template, fragments, retries, rate limit, browser cookies, and free-form custom arguments.
- Live command preview: the exact yt-dlp command TubeDeck runs (built by the same Rust code), with copy and warnings for incompatible or dangerous options such as `--exec`.
- 5 built-in presets plus your own. Dark, light or system theme. Fonts are bundled, so the app needs no network to render.

**Saved between sessions**
- Queue, history, settings, presets and per-download logs live in a local SQLite database.
- Downloads running when you quit resume on the next start (setting). Closing the window while downloading asks first.
- History with search, status filter, open folder, download again, log and delete. "Clear finished" in the queue moves downloads there.

**Engine**
- One-click install of yt-dlp, FFmpeg and Deno from their official GitHub releases. Files are refused unless they match the published SHA-256 checksums.
- Settings → Engine: versions, update check, stable/nightly yt-dlp channel, daily yt-dlp auto-update. Lookup order: configured path → TubeDeck's managed folder → PATH.
- Logs view and **Export diagnostics**: a text report for bug reports, with your user folder replaced by `%USERPROFILE%`.

## Develop

Prerequisites (Windows):

| Tool | Notes |
|---|---|
| Node.js LTS (22+) | `winget install OpenJS.NodeJS.LTS` |
| Rust (stable, MSVC) | `winget install Rustlang.Rustup` |
| Microsoft C++ Build Tools | Visual Studio Installer → "Desktop development with C++" |
| WebView2 Runtime | Already included in Windows 11 |

```powershell
npm install
npm run tauri dev
```

`npm run dev` on its own serves the UI in a normal browser with a simulated backend, which is handy for styling work.

## Test

```powershell
npm test; npm run build
```

```powershell
cd src-tauri; cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test
```

CI (`.github/workflows/ci.yml`) runs the frontend checks, rustfmt and the Rust tests on Linux. It runs automatically as the first step of every release; to run it any other time, use **Actions → CI → Run workflow** on GitHub. Clippy isn't part of CI; run it locally with `cargo clippy --all-targets -- -D warnings`. The manual release checklist is in [docs/TESTING.md](docs/TESTING.md).

## Build and release

```powershell
npm run tauri build
```

The installers are written to `src-tauri/target/release/bundle/nsis/` (`-setup.exe`, per-user) and `.../msi/`. A local build without the signing key works but skips the updater signature.

To publish a release:
1. Bump `version` in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`, and update `CHANGELOG.md`.
2. Commit and push to `main`, then on GitHub open **Actions → Release → Run workflow** (branch `main`). The workflow creates the `v<version>` tag itself. Pushing a tag (`git tag v0.1.2; git push origin v0.1.2`) also works, but GitHub keeps build caches per tag, so tag-started builds always compile from scratch. Releases started from `main` reuse the previous build's cache.
3. `.github/workflows/release.yml` runs the CI tests, then builds and signs the installers. It attaches them and `latest.json` to a **draft** GitHub release.
4. Review the draft and click **Publish**. Installed copies of TubeDeck then show "Update available" in the bottom-left corner within 6 hours, or right away on their next start. Nobody sees a draft.

### App updates and the signing key

TubeDeck updates itself through `tauri-plugin-updater`. It reads `latest.json` from the newest published release, downloads the installer, and installs it **only if its signature matches** the public key in `src-tauri/tauri.conf.json`. Then it restarts, and running downloads continue afterwards.

- The **public key** is in `tauri.conf.json` and is meant to be public: it can only check signatures.
- The **private key** and its password are stored as the encrypted repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (Settings → Secrets and variables → Actions). They are not in the code or git history, can't be read back from GitHub, and aren't given to workflows from forks.
- The original files are in `%USERPROFILE%\.tubedeck-signing\` on the developer machine. **Back up** `tubedeck.key` and `tubedeck.key.password.txt` (a password manager is ideal). If they're lost, existing installs can't accept future updates and everyone has to reinstall once by hand.

## Where things are stored

| What | Location |
|---|---|
| Database (queue, history, settings, presets) | `%LOCALAPPDATA%\com.tubedeck.desktop\tubedeck.db` |
| Application log / per-download logs | `%LOCALAPPDATA%\com.tubedeck.desktop\logs\tubedeck.log` / `logs\jobs\<id>.log` |
| Managed binaries (`yt-dlp.exe`, `deno.exe`, `ffmpeg\`) | `%LOCALAPPDATA%\com.tubedeck.desktop\bin\` |
| Partial downloads (kept for queued/paused jobs; removed on completion, cancel or removal) | `%LOCALAPPDATA%\com.tubedeck.desktop\tmp\` |

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
  presets.rs, settings.rs
  db.rs                  SQLite: jobs (queue + history), settings, presets, migrations
  diagnostics.rs         text report for bug reports (profile path redacted)
  installer.rs           download + SHA-256 verify + install yt-dlp/FFmpeg/Deno, update checks
  jsonfile.rs            one-time import of the pre-M4 JSON files
  binaries.rs            locate yt-dlp / FFmpeg / Deno, read versions
  commands.rs            Tauri commands
  engine/
    mod.rs               queue, scheduler, pause/resume/cancel/retry, reorder, batched events
    errors.rs            classify yt-dlp failures, hints, auto-retry backoff
    probe.rs             inspect URLs (yt-dlp -J --flat-playlist), playlist folder/numbering
    job.rs               Job model and queue ordering
    runner.rs            spawn yt-dlp, merge stdout/stderr, apply parsed lines
    progress.rs          parse TubeDeck's progress templates (+ real-output fixture test)
    winjob.rs            Windows Job Object (kill the whole process tree)
```
