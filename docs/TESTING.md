# Testing TubeDeck

## Automated

| What | Command | Covers |
|---|---|---|
| Rust unit tests | `cd src-tauri; cargo test` | option → yt-dlp argument builder, progress parser (real yt-dlp output fixture), error classification and retry policy, playlist probe parsing (real fixtures), folder names and numbering, SQLite storage and history queries, startup recovery, installer checksum parsing / version comparison / file swap, diagnostics redaction |
| Rust lints | `cargo fmt --check; cargo clippy --all-targets -- -D warnings` | formatting, common mistakes |
| Frontend tests | `npm test` | formatting helpers, playlist URL detection and range parsing, queue grouping, option summaries |
| Type-check + build | `npm run build` | TypeScript types (mirroring the Rust types) and the production bundle |

CI (`.github/workflows/ci.yml`) runs all of these on Windows for every push to `main` and every pull request.

## Browser preview

`npm run dev` serves the UI in a normal browser with a simulated backend (`src/lib/mock.ts`). It's useful for layout and styling: queue progress, retries, pause, playlist picker and engine setup are all simulated. It does not exercise Rust.

## Manual release checklist

Run this on the installed build (`npm run tauri build`, then run the `-setup.exe`) before publishing a release. Ideally use a Windows user account with no yt-dlp, FFmpeg or Deno on PATH.

**First run**
- [ ] SmartScreen warning appears (unsigned) and "Run anyway" works; installation needs no admin rights.
- [ ] The setup panel shows yt-dlp as required and "Install everything" installs all three with progress.
- [ ] Settings → Engine shows all three as `managed` with versions; "Check for updates" reports latest versions.

**Downloads**
- [ ] A single video downloads, merges and embeds the thumbnail (needs FFmpeg).
- [ ] Audio-only MP3 works.
- [ ] A 4K/long video can be paused (no yt-dlp/ffmpeg left in Task Manager), resumed (continues from the same %), and canceled (partial files removed).
- [ ] A non-existent video fails right away with a clear message and no retries.
- [ ] Unplugging the network mid-download → "Retrying in 5s", and it continues after reconnecting.
- [ ] Non-ASCII titles (e.g. Japanese) are saved with the right file name.
- [ ] A very long title doesn't fail (trimmed by `--trim-filenames`).

**Playlists**
- [ ] A playlist URL opens the picker; range "1-3" selects 3 items; files land in `<Playlist>\01 - …`.
- [ ] A 50+ item playlist lists quickly; scrolling the picker stays smooth.
- [ ] A channel's Playlists tab shows nested playlists that open in place.

**Persistence**
- [ ] Quit while downloading → the dialog appears → Quit; on next start the download resumes from its partial file.
- [ ] History keeps finished downloads across restarts; "Download again" works; delete works.
- [ ] Settings, presets and theme survive a restart.

**Misc**
- [ ] Light and dark theme, and resizing down to 850 px wide.
- [ ] A second launch focuses the running window instead of opening another.
- [ ] Logs → Export diagnostics writes a report without your Windows user name in it.
- [ ] Uninstall from Windows Settings removes the app (downloads and app data stay).
