// Browser-only stand-in for the Rust backend so `npm run dev` renders without Tauri.
// It is intentionally simple: the real command builder lives in Rust (args.rs).

import type { BuiltCommand, DownloadOptions, Preset, Settings } from "./types";

export const defaultOptions: DownloadOptions = {
  mode: "video",
  quality: "best",
  container: "mp4",
  videoCodec: "any",
  audioFormat: "best",
  audioQuality: 0,
  embedThumbnail: true,
  embedMetadata: true,
  embedChapters: true,
  subtitles: "none",
  autoSubs: false,
  subLangs: "en.*",
  sponsorBlock: "off",
  noPlaylist: false,
  fragments: 4,
  retries: 10,
  rateLimit: "",
  cookiesFromBrowser: null,
  outputFolder: "",
  filenameTemplate: "%(title)s [%(id)s].%(ext)s",
  trimFilenames: 180,
  extraArgs: "",
};

let settings: Settings = {
  theme: "system",
  downloadFolder: "C:\\Users\\you\\Downloads\\TubeDeck",
  maxConcurrent: 3,
  ytdlpPath: "",
  ffmpegPath: "",
  denoPath: "",
  currentOptions: defaultOptions,
  activePresetId: "builtin-best-video",
};

let presets: Preset[] = [
  {
    id: "builtin-best-video",
    name: "Best video (MP4)",
    description: "Highest available quality, merged to MP4.",
    builtin: true,
    options: defaultOptions,
  },
  {
    id: "builtin-audio-mp3",
    name: "Audio MP3",
    description: "Extract audio as high-quality MP3 with cover art.",
    builtin: true,
    options: { ...defaultOptions, mode: "audio", audioFormat: "mp3" },
  },
];

function mockCommand(url: string): BuiltCommand {
  const args = ["-f", "bv*+ba/b", "--", url || "<URL>"];
  return {
    program: "yt-dlp",
    args,
    display: `yt-dlp -f 'bv*+ba/b' -- ${url || "<URL>"}`,
    warnings: [{ level: "info", message: "Browser preview: run the desktop app for the real command." }],
  };
}

export async function mock<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const a = args ?? {};
  const r = (v: unknown) => Promise.resolve(structuredClone(v) as T);
  switch (cmd) {
    case "get_settings":
      return r(settings);
    case "save_settings":
      settings = a.settings as Settings;
      return r(settings);
    case "build_command":
      return r(mockCommand(String(a.url ?? "")));
    case "parse_urls":
      return r(
        [...new Set(String(a.text).split(/\s+/).filter(Boolean))].map((url) => ({
          url,
          error: /^https?:\/\//i.test(url) ? null : "Only http:// and https:// URLs are supported",
        })),
      );
    case "list_presets":
      return r(presets);
    case "save_preset": {
      const p = { ...(a.preset as Preset), builtin: false };
      if (!p.id) p.id = `user-${Date.now().toString(16)}`;
      presets = [...presets.filter((x) => x.id !== p.id), p];
      return r(p);
    }
    case "delete_preset":
      presets = presets.filter((x) => x.id !== a.id);
      return r(undefined);
    case "engine_status": {
      const missing = { found: false, path: null, source: null, version: null, ok: false, note: "Browser preview" };
      return r({
        ytdlp: { name: "yt-dlp", ...missing },
        ffmpeg: { name: "FFmpeg", ...missing },
        deno: { name: "Deno", ...missing },
        managedDir: "",
      });
    }
    case "read_app_log":
      return r({ path: "(browser preview)", lines: [] });
    case "app_paths":
      return r({ configDir: "", logDir: "", managedBinDir: "", defaultDownloadFolder: settings.downloadFolder });
    default:
      return Promise.reject(`mock: unknown command ${cmd}`);
  }
}
