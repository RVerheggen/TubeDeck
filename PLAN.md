# TubeDeck — Implementation Plan

Windows desktop download manager: **React + TypeScript (Vite) · Tauri 2 · Rust · yt-dlp · FFmpeg**.

This plan is based on `TubeDeck-instructions.txt`. Part A builds the v1 application described there (filling the gaps in it).
Part B is the follow-up milestone: the real download engine (streaming progress, pause/cancel, playlists, persistence, binary management).

---

## 0. Review of the instructions — gaps and fixes

The instructions are a good starting point but not complete or fully correct. These are resolved in the plan below.

| # | Issue in the instructions | Resolution |
|---|---|---|
| 1 | `App.tsx`, `Sidebar.tsx`, `CommandPreview.tsx`, `DownloadCard.tsx` are referenced but **no code is given** | Write them from scratch (A3) |
| 2 | Semaphore is created once with the first job's limit; it can't be changed later | Replace with a scheduler that reads the current limit (B2) |
| 3 | `Command::output()` blocks until yt-dlp exits, so there's no live progress | Spawn with `tokio::process`, stream stdout/stderr line by line (B3) |
| 4 | Every spawn opens a console window on Windows | Set `CREATE_NO_WINDOW` (`0x08000000`) via `CommandExt::creation_flags` |
| 5 | `--no-warnings` hides useful diagnostics | Keep warnings and send them to the job log |
| 6 | Audio vs. video mode is inferred from `audio != "best"` | Use an explicit `mode: "video" \| "audio"` enum |
| 7 | Output path built by string concatenation; name collisions; MAX_PATH | `-P home:<dir> -P temp:<jobTmp> -o "%(title)s [%(id)s].%(ext)s" --trim-filenames 180` |
| 8 | Quality fallback `/b` ignores the height cap | Use sorting: `-f "bv*+ba/b" -S "res:1080"` (codec preference also via `-S`, e.g. `vcodec:h264`) |
| 9 | Command preview built in the frontend, execution in Rust: they will drift apart | **One** argument builder in Rust, used for both preview and execution |
| 10 | Google Fonts import + CSP entry conflicts with "no external assets" and breaks offline | Bundle Inter locally (`@fontsource-variable/inter`), fall back to Segoe UI, remove Google from CSP |
| 11 | `tokio` features (`sync, rt, macros`) can't spawn processes | Add `process`, `io-util`, `time`; run on `tauri::async_runtime` |
| 12 | URL passed directly as an argument | Validate http(s) **and** put `--` before the URL |
| 13 | The app assumes `yt-dlp`/`ffmpeg` on PATH | Managed binaries in app data with PATH fallback (B6) |
| 14 | yt-dlp needs an external JS runtime for full YouTube support (**confirmed** in the yt-dlp README and EJS wiki, Nov 2026) | Manage Deno ≥ 2.3.0 alongside yt-dlp (B6a) |

Decisions so far:
- Rows 1–14 are adopted as written. All UI code is written from scratch; the instructions file is only a reference for intent and styling.
- The four engine problems (rows 2, 3, 4 and the orphaned ffmpeg) are all fixable and are fixed in **M2** (streaming, no console window, kill the whole process tree) and **M3** (adjustable concurrency). See B2/B3.

---

## Part A — v1 application

### A1. Prerequisites (one-time)
- Node.js LTS, Rust (stable, `x86_64-pc-windows-msvc`), Microsoft C++ Build Tools ("Desktop development with C++"), WebView2 Runtime (already on Windows 11).
- yt-dlp + FFmpeg on PATH **for development only**. Part B replaces this with managed binaries.
- `git init` in the repo before scaffolding.

### A2. Scaffold
```powershell
npm create tauri-app@latest .
```
Pick: TypeScript → npm → React → TypeScript. Scaffold **into the existing `TubeDeck` folder** (it only contains the instructions and this plan).

Then:
```powershell
npm install lucide-react zustand @fontsource-variable/inter
npm install @tauri-apps/plugin-dialog @tauri-apps/plugin-opener @tauri-apps/plugin-clipboard-manager
```
Rust plugins: `tauri-plugin-dialog` (folder picker), `tauri-plugin-opener` (open file / reveal in Explorer), `tauri-plugin-clipboard-manager`, `tauri-plugin-log`, `tauri-plugin-single-instance`.

Keep whatever React/Vite versions the template generates (React 19 is fine). Don't downgrade to the versions pinned in the instructions.

### A3. Frontend structure
```
src/
├── main.tsx
├── App.tsx                    # layout shell + view routing (simple state, no router needed)
├── styles.css                 # from instructions, minus Google Fonts import
├── lib/
│   ├── types.ts               # DownloadOptions, Job, Preset, Settings… (mirror of Rust types)
│   ├── api.ts                 # typed wrappers around invoke()
│   ├── events.ts              # listen() subscriptions → store
│   └── format.ts              # bytes/speed/ETA formatting
├── store/
│   └── useAppStore.ts         # Zustand: jobs, options, presets, settings, theme
├── components/
│   ├── Sidebar.tsx            # nav + queue counts + engine status
│   ├── AddUrlPanel.tsx        # multi-line URL input, quick-setting chips, "Add to queue"
│   ├── StatsGrid.tsx          # active / queued / completed / total speed
│   ├── DownloadCard.tsx       # title, progress bar, speed, ETA, size, actions
│   ├── ConfigPanel.tsx        # mode, quality, container, audio codec, toggles
│   ├── AdvancedPanel.tsx      # fragments, retries, rate limit, cookies, proxy, templates
│   ├── CustomArgsEditor.tsx   # free-form extra args with validation
│   ├── CommandPreview.tsx     # live command (from Rust), copy button
│   ├── PresetList.tsx
│   ├── HistoryTable.tsx
│   ├── LogViewer.tsx
│   └── EngineStatus.tsx
└── views/
    ├── DownloadsView.tsx
    ├── ConfigureView.tsx
    ├── PresetsView.tsx
    ├── HistoryView.tsx
    ├── LogsView.tsx
    └── SettingsView.tsx       # theme, default folder, concurrency, binary paths
```

### A4. Option model (shared by UI, presets, and the engine)
```ts
type DownloadOptions = {
  mode: "video" | "audio";
  quality: "best" | "2160" | "1440" | "1080" | "720" | "480" | "360";
  container: "mp4" | "mkv" | "webm";                 // video merge format
  videoCodec: "any" | "h264" | "vp9" | "av1";        // → -S vcodec:…
  audioFormat: "best" | "mp3" | "m4a" | "opus" | "flac" | "wav";
  audioQuality: number;                              // 0 (best) … 10
  embedThumbnail: boolean; embedMetadata: boolean; embedChapters: boolean;
  subtitles: "none" | "embed" | "write"; subLangs: string;   // e.g. "en.*,nl"
  sponsorBlock: "off" | "mark" | "remove";
  fragments: number; retries: number; rateLimit?: string;    // "5M"
  cookiesFromBrowser?: "firefox" | "edge" | "chrome" | "brave";
  outputFolder: string; filenameTemplate: string;
  extraArgs: string;                                         // advanced, validated
};
```

### A5. Features in v1
| Feature | v1 implementation |
|---|---|
| URL input (single/multiple/playlist) | One URL per line. Playlist URLs are enqueued as one job for now (item selection comes in B5) |
| Video/audio/codec/quality settings | `ConfigPanel` bound to store |
| Advanced yt-dlp config | `AdvancedPanel` + `CustomArgsEditor` (split with a `shell-words`-style parser in Rust; warn about `--exec`, `--netrc-cmd`, `-o`/`-P` overrides) |
| Live command preview + copy | `invoke("build_command", { url, options })` with a 150 ms debounce. Rust returns `argv: string[]` + a PowerShell-quoted display string |
| Queue with simultaneous downloads | Backed by the engine. In v1 it can be the simple streaming version (B3), not the full manager |
| Progress, speed, ETA | Comes from B3. If Part A ships before B3, show indeterminate progress |
| Saved presets | Built-in presets (Best video MP4, 1080p H.264, Audio MP3, Audio Opus, Archive w/ subs+metadata) plus user presets. Persisted to a JSON file in app data in v1; moved to SQLite in B4 |
| Dark/light theme | `system \| dark \| light`. Set `data-theme` on `<html>` and persist it |
| History + diagnostic logs | v1: in-memory history plus `tauri-plugin-log` file log. Full version in B4 |
| Build/installer docs | README section (A7) |

**Remove the demo/sample download cards.** The queue should only show real jobs. Use an empty-state illustration instead.

### A6. Tauri configuration changes vs. instructions
- CSP: drop `fonts.googleapis.com` / `fonts.gstatic.com`. Keep `img-src https:` for thumbnails (or proxy/cache them via Rust later).
- Add plugin permissions to `capabilities/default.json` (`dialog:allow-open`, `opener:allow-open-path`, `opener:allow-reveal-item-in-dir`, `clipboard-manager:allow-write-text`, `log:default`).
- Give the window an explicit `"label": "main"` so it matches the capability.
- Bundle targets: NSIS (primary) + MSI.

### A7. README: build and install
Prerequisites, `npm install`, `npm run tauri dev`, `npm run tauri build`, output in `src-tauri/target/release/bundle/{nsis,msi}`, first-run binary setup, and a note that unsigned installers trigger SmartScreen.

### A — Done when
- `npm run tauri dev` launches. All six views render in both themes at 850px+ width.
- The command preview updates live, matches what is executed, and copies to the clipboard.
- A single URL downloads to the chosen folder. Errors appear in the log view.
- `npm run tauri build` produces a working NSIS installer.

---

## Part B — Download engine (follow-up milestone)

### B1. Architecture
```
src-tauri/src/
├── lib.rs                 # builder, plugins, state, command registration
├── commands.rs            # #[tauri::command] thin wrappers
├── engine/
│   ├── mod.rs             # DownloadManager handle (cloneable; sends messages to actor)
│   ├── actor.rs           # single task owning all job state; scheduler loop
│   ├── job.rs             # Job, JobStatus, Progress types
│   ├── args.rs            # DownloadOptions → Vec<String>  (single source of truth)
│   ├── runner.rs          # spawn yt-dlp, read streams, kill tree
│   ├── progress.rs        # parse progress/print lines
│   ├── probe.rs           # yt-dlp -J for metadata & playlists
│   ├── errors.rs          # classify stderr into actionable error kinds
│   └── winjob.rs          # Windows Job Object (kill whole process tree)
├── binaries/
│   ├── mod.rs             # resolve yt-dlp / ffmpeg / deno paths, versions
│   └── installer.rs       # download + SHA-256 verify + update
└── db/
    ├── mod.rs             # rusqlite connection, migrations (PRAGMA user_version)
    └── repo.rs            # jobs, presets, settings, history queries
```

**Actor pattern.** One tokio task owns the queue and job map. Commands and runner callbacks send messages over an `mpsc` channel. This avoids lock juggling and makes the scheduling rules easy to follow.

### B2. Job lifecycle and scheduler
```
Queued ─▶ Starting ─▶ Downloading ─▶ Processing ─▶ Completed
   ▲          │             │              │
   │          └──────┬──────┴──────────────┘
   │                 ▼
   ├──── Paused ◀── (pause)         Failed ──(retry)──▶ Queued
   └──────────────── (resume)       Canceled
```
- The scheduler runs after every state change. While `running < settings.maxConcurrent`, it starts the next `Queued` job by `position`.
- `set_concurrency(n)` takes effect right away. Lowering it does not kill running jobs; it just stops new ones from starting.
- Reorder: drag and drop in the UI calls `move_job(id, newPosition)`.
- Auto-retry transient network errors with backoff (e.g. 3 attempts: 5s/20s/60s). Don't retry permanent errors (private video, geo-blocked, unsupported URL).

### B3. Running yt-dlp with live progress
Base arguments added by the engine, in addition to the user's options:
```
--newline --progress --no-simulate --no-colors --encoding utf-8
--progress-template "download:TD_PROG %(progress)j"
--progress-template "postprocess:TD_PP %(progress.postprocessor)s|%(progress.status)s"
--print "before_dl:TD_INFO %(.{id,title,ext,duration,thumbnail,playlist_index})j"
--print "after_move:TD_FILE %(filepath)s"
--ffmpeg-location <resolved> [--js-runtimes deno:<resolved>]
-P home:<outputFolder> -P temp:<appdata>/tmp/<jobId>
-o <filenameTemplate>
-- <url>
```
Notes:
- `--print` implies `--quiet`/`--simulate`. `--no-simulate` + `--progress` keep the download and progress output on.
- Environment: `PYTHONIOENCODING=utf-8`, `PYTHONUTF8=1`, so titles with non-ASCII characters survive on Windows.
- Spawn with `tokio::process::Command`: piped stdout/stderr, `kill_on_drop(true)`, `CREATE_NO_WINDOW`. Assign the process to a **Windows Job Object** with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, so killing yt-dlp also kills its ffmpeg children (use the `windows` crate, or `win32job`).
- **Parsing.** Lines starting with `TD_PROG` are JSON: `downloaded_bytes`, `total_bytes`/`total_bytes_estimate`, `speed`, `eta`, `fragment_index`/`fragment_count`, `status`. `TD_PP` lines mean the job is in Processing. `TD_FILE` gives the final path. Every other line goes to the job log.
- **Multiple streams.** Video and audio download one after the other, and progress resets for each. Track the stream index ("Video 1/2", "Audio 2/2") and show a combined percentage.
- **Throttling.** Parse every line, but send the UI a batched `jobs://updated` event with changed jobs only, at most every 250 ms. This keeps 10+ parallel downloads smooth.
- **Unit-test the parser** with captured real yt-dlp output in `tests/fixtures/`.

### B4. Persistence (SQLite via `rusqlite` with `bundled`)
Database at `%LOCALAPPDATA%\com.tubedeck.desktop\tubedeck.db`.
```sql
jobs(id TEXT PK, url, title, thumbnail, status, position INTEGER,
     options_json, playlist_id NULL, playlist_index NULL,
     output_path NULL, bytes_total NULL, error_kind NULL, error_msg NULL,
     attempts INTEGER, created_at, started_at, finished_at)
presets(id TEXT PK, name, builtin INTEGER, options_json, updated_at)
settings(key TEXT PK, value_json)
```
- **History** = jobs in a terminal state. The History view adds search, filters (status/date), "open file", "show in folder", "re-download", and "clear history".
- **Logs.** App log via `tauri-plugin-log` (rotating files). Per-job log at `logs/jobs/<id>.log`, viewable from the card and from the Logs view. Add an "Export diagnostics" button that zips the logs, versions, and settings, with cookies/paths redacted.
- **Startup recovery.** Jobs that were `Downloading`/`Starting` at shutdown become `Paused` (or `Queued` if the "auto-resume on launch" setting is on).
- **Graceful shutdown.** On window close, ask "N downloads running: pause and quit?" Then kill the process trees (partial files stay in the job's temp folder) and write state.

### B5. Pause, resume, cancel, and playlists
- **Pause.** yt-dlp has no native pause, so pause = kill the process tree and set status to `Paused`. **Resume** re-runs the same argv: yt-dlp continues from the `.part`/`.ytdl` files in the job's own temp folder. Live streams and some HLS sources can't resume; the UI shows that.
- **Cancel.** Kill the tree and delete `tmp/<jobId>`. A per-job temp dir makes cleanup exact.
- **Playlist inspection.** `probe_url` runs `yt-dlp -J --flat-playlist -- <url>` (with a timeout and a cancel option) and returns entries (id, title, duration, url, index). The `PlaylistPickerDialog` offers select all/none, a range ("1-20"), a title filter, and the total count.
- Each selected entry becomes **its own job** (own progress, own pause/retry), grouped by `playlist_id` and collapsible in the queue. Output path: `<folder>/<sanitized playlist title>/<NNN> - %(title)s [%(id)s].%(ext)s`. Index and title come from the probe, so the playlist isn't fetched again per item.
- **Single-video probe (optional, nice to have).** `yt-dlp -J --no-playlist` shows title, thumbnail, duration, and the real available formats before enqueueing.

### B6. Binary management (yt-dlp, FFmpeg, Deno)
Resolution order: **user-configured path → managed `%LOCALAPPDATA%\…\bin\` → PATH**.
- **First-run setup screen.** Detects each binary and its version. Offers a one-click download from the official sources (yt-dlp GitHub releases `yt-dlp.exe`, verified against `SHA2-256SUMS`; FFmpeg from the yt-dlp FFmpeg-Builds win64 release; Deno from its official releases).
- **Updates.** Check at most once a day. Run `yt-dlp --update-to stable|nightly` (user setting). Show the version in `EngineStatus`, plus a "yt-dlp is outdated" hint when errors look like extractor breakage (e.g. HTTP 403, "Sign in to confirm you're not a bot").
- **Why managed instead of bundled as a Tauri sidecar.** See below.

#### B6a. JavaScript runtime (verified against the yt-dlp README + EJS wiki)
- YouTube serves JavaScript challenges. yt-dlp solves them with the **yt-dlp-ejs** scripts, which need an external JS runtime.
- **The official `yt-dlp.exe` already bundles yt-dlp-ejs.** With managed binaries, nothing extra is needed beyond the runtime. A pip-installed yt-dlp that the user points to may need `--remote-components ejs:github`; the engine status screen detects this.
- **Runtime choice.** Use **Deno** (recommended, the only runtime enabled by default, minimum **2.3.0**, runs the scripts without file/network access). Download the `deno` release asset, **not** `denort`.
- **Where to put it.** On Windows, yt-dlp finds `deno.exe` automatically when it sits next to `yt-dlp.exe`, so it goes in the managed `bin\` folder. The engine still passes `--js-runtimes deno:<path>` explicitly, so a user-configured yt-dlp path also works.
- **Fallback.** Node ≥ 22 works too, but only when enabled (`--no-js-runtimes --js-runtimes node:<path>`). Offer it as a setting for users who already have Node and don't want Deno. QuickJS and Bun are not offered (QuickJS is slow, Bun is deprecated).
- **Without a runtime**, YouTube still partly works, but yt-dlp drops some player clients, so fewer formats are available and more downloads fail. Show a persistent warning in `EngineStatus` and map the related stderr messages to `js_runtime_missing`.
- **Version check.** Run `deno --version` during the `engine_status()` check and warn below 2.3.0.

#### Why managed instead of bundled yt-dlp needs frequent updates, and a copy inside `Program Files` can't self-update without admin rights. Redistributing GPL FFmpeg builds inside the installer also brings licence obligations. Downloading on demand avoids both.

### B7. IPC surface
| Command | Purpose |
|---|---|
| `engine_status()` | binary paths + versions, Deno present, db ok |
| `build_command(url, options)` | `{ argv, display }` for preview |
| `probe_url(url)` | single metadata or playlist entries |
| `enqueue(requests[])` | create jobs → ids |
| `pause_job / resume_job / cancel_job / retry_job / remove_job(id)` | per-job control |
| `pause_all / resume_all / clear_completed()` | bulk |
| `move_job(id, position)`, `set_concurrency(n)` | queue control |
| `list_jobs()`, `list_history(query)`, `get_job_log(id)` | reads |
| `presets_*`, `settings_get/set` | config |
| `install_binary(name)`, `update_ytdlp(channel)` | binaries (progress via events) |

Events: `jobs://updated` (batched), `jobs://removed`, `engine://binaries`, `engine://install-progress`.
Optional: generate TS types from Rust (`ts-rs` or `tauri-specta`) so `lib/types.ts` never drifts.

### B8. Error classification (shown as friendly messages + raw log)
`ytdlp_missing`, `ffmpeg_missing`, `js_runtime_missing`, `network`, `http_403` (suggest updating yt-dlp), `bot_check` (suggest cookies from browser; Firefox is the most reliable on Windows), `private_or_removed`, `geo_blocked`, `unsupported_url`, `disk_full`, `path_too_long`, `unknown`.

### B — Done when
- 5+ parallel downloads show smooth live percent/speed/ETA. Changing concurrency takes effect without restarting.
- Pause → resume continues from the partial file. Cancel leaves no leftovers. No orphan `ffmpeg.exe` after killing.
- Playlist picker enqueues the selected items as grouped jobs.
- Closing and reopening the app restores the queue, history, presets, and settings.
- A fresh Windows machine with nothing on PATH gets working downloads through the first-run setup.

---

## Part C — Quality and release

- **Tests.** Rust unit tests for `args.rs` (option → argv snapshots), `progress.rs` (fixtures), `errors.rs`, and the scheduler (fake runner). Vitest for formatters and store reducers. A manual test checklist per release (single video, 4K merge, audio extract, playlist 50+, pause/resume, cancel, offline, missing ffmpeg, non-ASCII titles, long paths).
- **CI.** GitHub Actions on `windows-latest`: `cargo clippy -D warnings`, `cargo test`, `npm run build`, `tauri-action` build that uploads the NSIS/MSI artifacts.
- **Release.** Code signing (avoids SmartScreen warnings), app auto-update via `tauri-plugin-updater` (signed update manifests), versioning plus a changelog.

---

## Milestone order

1. ✅ **M0. Scaffold.** git, create-tauri-app, plugins, fonts, CSP, empty views.
2. ✅ **M1. UI shell.** All components and views, theme, option model, built-in presets, Rust `args.rs` + live command preview.
3. ✅ **M2. Minimal engine.** Streaming runner + progress parser + batched events → real cards (B3). Includes `CREATE_NO_WINDOW` and the Job Object from the first real download, so there are no console windows and no orphaned ffmpeg. *This is where v1 becomes useful.*
   Delivered earlier than planned: cancel (kills the process tree), retry, a per-job temp dir, a per-job log viewer, and concurrency limits that apply live. The queue uses a `Mutex` plus a schedule-on-change design rather than an actor; it is simple and has worked well so far.
   Found during E2E testing: yt-dlp writes `postprocess` progress lines to **stderr**, so the runner merges both streams through one parser.
4. **M3. Manager.** Pause/resume (kill and restart, keeping partial files), auto-retry with backoff for transient errors (e.g. YouTube HTTP 403), reordering (B2, B5 part 1).
5. **M4. Persistence.** SQLite, history, per-job logs, startup recovery, graceful shutdown (B4).
6. **M5. Playlists.** Probe + picker + grouped jobs (B5 part 2).
7. **M6. Binaries.** First-run setup, managed downloads, yt-dlp updates, error classification (B6, B8).
8. **M7. Release.** Tests, CI, installer testing, signing, README (Part C, A7).

## Open decisions
- **Binary strategy.** Recommended: managed download into app data with PATH fallback. The alternative is PATH-only (simpler, but worse for non-technical users).
- **Cookies support** (`--cookies-from-browser`) in v1, or later? It's needed for age-restricted and bot-check content.
- **Distribution.** Personal use (unsigned is fine) or public (budget for a code-signing certificate)?
