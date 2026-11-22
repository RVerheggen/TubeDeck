# Changelog

## 0.1.0 (unreleased)

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

### Known limitations
- The installer is not code-signed, so Windows SmartScreen warns on first run.
- No automatic updates for TubeDeck itself yet (yt-dlp does update automatically).
