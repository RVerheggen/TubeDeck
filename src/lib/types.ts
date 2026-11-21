// Mirrors the Rust types in src-tauri/src/{options,settings,presets,args,binaries}.rs.
// Keep field names identical (camelCase on the wire).

export type Mode = "video" | "audio";
export type Quality = "best" | "2160" | "1440" | "1080" | "720" | "480" | "360";
export type Container = "mp4" | "mkv" | "webm";
export type VideoCodec = "any" | "h264" | "vp9" | "av1";
export type AudioFormat = "best" | "mp3" | "m4a" | "opus" | "flac" | "wav";
export type Subtitles = "none" | "embed" | "write";
export type SponsorBlock = "off" | "mark" | "remove";
export type Browser = "firefox" | "edge" | "chrome" | "brave";

export interface DownloadOptions {
  mode: Mode;
  quality: Quality;
  container: Container;
  videoCodec: VideoCodec;
  audioFormat: AudioFormat;
  audioQuality: number;
  embedThumbnail: boolean;
  embedMetadata: boolean;
  embedChapters: boolean;
  subtitles: Subtitles;
  autoSubs: boolean;
  subLangs: string;
  sponsorBlock: SponsorBlock;
  noPlaylist: boolean;
  fragments: number;
  retries: number;
  rateLimit: string;
  cookiesFromBrowser: Browser | null;
  outputFolder: string;
  filenameTemplate: string;
  trimFilenames: number | null;
  extraArgs: string;
}

export type Theme = "system" | "dark" | "light";

export interface Settings {
  theme: Theme;
  downloadFolder: string;
  maxConcurrent: number;
  autoRetry: number;
  ytdlpPath: string;
  ffmpegPath: string;
  denoPath: string;
  currentOptions: DownloadOptions;
  activePresetId: string | null;
}

export interface Preset {
  id: string;
  name: string;
  description: string;
  builtin: boolean;
  options: DownloadOptions;
}

export type WarningLevel = "info" | "warn" | "danger";

export interface CommandWarning {
  level: WarningLevel;
  message: string;
}

export interface BuiltCommand {
  program: string;
  args: string[];
  display: string;
  warnings: CommandWarning[];
}

export interface ParsedUrl {
  url: string;
  error: string | null;
}

export type BinarySource = "configured" | "managed" | "path";

export interface BinaryStatus {
  name: string;
  found: boolean;
  path: string | null;
  source: BinarySource | null;
  version: string | null;
  ok: boolean;
  note: string | null;
}

export interface EngineStatus {
  ytdlp: BinaryStatus;
  ffmpeg: BinaryStatus;
  deno: BinaryStatus;
  managedDir: string;
}

export interface AppLog {
  path: string;
  lines: string[];
}

export interface AppPaths {
  configDir: string;
  logDir: string;
  managedBinDir: string;
  defaultDownloadFolder: string;
}

// ---- Queue (owned by the Rust engine, src-tauri/src/engine) ------------------------

export type JobStatus =
  | "queued"
  | "starting"
  | "downloading"
  | "processing"
  | "paused"
  | "completed"
  | "failed"
  | "canceled";

export interface JobProgress {
  percent: number | null;
  downloadedBytes: number | null;
  totalBytes: number | null;
  speed: number | null; // bytes/s
  eta: number | null; // seconds
}

export interface Job {
  id: string;
  url: string;
  title: string | null;
  thumbnail: string | null;
  status: JobStatus;
  /** Sub-state such as "Stream 1/2" or "Merging formats". */
  stage: string | null;
  progress: JobProgress;
  options: DownloadOptions;
  presetName: string | null;
  outputPath: string | null;
  createdAt: number;
  startedAt: number | null;
  finishedAt: number | null;
  error: string | null;
  errorKind: ErrorKind | null;
  errorHint: string | null;
  /** Automatic retries made so far. */
  attempts: number;
  /** Epoch ms when a queued job will be retried automatically. */
  retryAt: number | null;
}

export type ErrorKind =
  | "network"
  | "http403"
  | "rate_limited"
  | "server_error"
  | "not_found"
  | "bot_check"
  | "age_restricted"
  | "unavailable"
  | "geo_blocked"
  | "unsupported"
  | "ffmpeg_missing"
  | "js_runtime"
  | "disk_full"
  | "unknown";

export type ViewId = "downloads" | "configure" | "presets" | "history" | "logs" | "settings";
