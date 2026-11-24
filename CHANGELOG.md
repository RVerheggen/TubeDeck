# Changelog

## Unreleased

### Releases
- Publishing a release no longer starts a second, failing release run. Releases are started only with Actions → Release → Run workflow.

## 0.1.5 (2026-11-24)

### Changed
- The update dialog lists what changed since your version (from the changelog), with a link to the full changelog, instead of the generic release text.

### Releases
- Release pages show that version's changes. A release is refused when CHANGELOG.md has no section for its version.

## 0.1.4 (2026-11-24)

### Fixed
- The preset list on the Downloads page kept showing a preset such as "Audio MP3" after its settings were changed (for example to Video in Configure), so downloads came out as MP4. It now shows "Custom (changed from Audio MP3)", and choosing the preset again re-applies it. Such downloads are labelled "Audio MP3 (modified)".

## 0.1.3 (2026-11-23)

### Fixed
- Titles with characters such as "–", accents or emoji were saved correctly, but TubeDeck recorded a garbled path, so "Show in folder" failed with "path doesn't exist". yt-dlp now reports paths in UTF-8.
- "Show in folder" finds files recorded with such garbled names by earlier versions, and opens the folder when a file was moved or deleted instead of showing an error.

## 0.1.2 (2026-11-22)

### Changed
- TubeDeck's own version is shown in the bottom-left corner (click it for Settings → About).
- After an update, TubeDeck comes to the front instead of opening behind other windows, and says which version it was updated to.

### Releases
- Releases can be started with Actions → Release → Run workflow on `main`; those builds reuse the Rust build cache.

## 0.1.1 (2026-11-22)

### Fixed
- "Install everything" could fail for one component with "Could not write …\.downloads\….part (os error 3)": the parallel installs shared a temporary folder that the first one to finish deleted. Each install now has its own.
- Long error messages in the engine setup cards now wrap inside the card, and a failed install has a "Try again" button on its card.

## 0.1.0 (2026-11-22)

The first complete version of TubeDeck: a Windows download manager for yt-dlp.

### Downloading
- Queue with parallel downloads up to a limit you set (changes apply immediately).
- Live progress per download: percent, speed, ETA, video/audio stream and post-processing stage, with thumbnails and titles.
- Pause and resume (continues from the partial file), cancel (stops yt-dlp, its helper process and FFmpeg, and removes partial files), retry, and drag-and-drop reordering.
- Automatic retry for temporary failures (network, HTTP 403/429/5xx) after 5s, 20s, 60s…; permanent errors explain what to do (cookies for bot checks, update yt-dlp, and so on).

### Playlists
- Playlist and channel URLs open a picker with every item; select with filter, ranges or shift+click; nested playlists open in place.
- Each item downloads separately, grouped under a collapsible playlist header, into `<Playlist>\NN - <title>`.

### Settings and presets
- Video/audio mode, quality cap, container, preferred codec, audio format and quality, subtitles, SponsorBlock, embedding, output folder and filename template, fragments, retries, rate limit, browser cookies and custom yt-dlp arguments.
- Live command preview (the exact command TubeDeck runs) with copy and warnings.
- 5 built-in presets plus your own.
- Dark, light or system theme.

### Saved between sessions
- Queue, history, settings, presets and per-download logs are stored in SQLite.
- Downloads interrupted by quitting resume on the next start (setting); closing while downloading asks first.
- History with search, filters, open folder, download again and delete.

### Engine
- One-click install of yt-dlp, FFmpeg and Deno from their official GitHub releases, verified against published SHA-256 checksums.
- yt-dlp updates itself daily (setting); stable or nightly channel.
- Export diagnostics for bug reports (user folder redacted).

### Updates
- TubeDeck updates itself: "Update available" appears in the bottom-left corner when a new release is published. Updates are verified with TubeDeck's signing key before installing, and running downloads continue after the restart.
- Settings → About TubeDeck shows the version and checks for updates on demand.

### Known limitations
- The installer is not code-signed, so Windows SmartScreen warns on first run.
