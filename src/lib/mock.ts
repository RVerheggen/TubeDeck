// Browser-only stand-in for the Rust backend so `npm run dev` renders without Tauri.
// It is intentionally simple: the real command builder lives in Rust (args.rs).

import type { BuiltCommand, DownloadOptions, Job, Preset, Settings } from "./types";

// ---- Simulated download engine (browser preview only) ------------------------------

type Handlers = { updated: (jobs: Job[]) => void; removed: (ids: string[]) => void };
const subscribers = new Set<Handlers>();
let jobs: Job[] = [];
let seq = 0;

export function mockSubscribe(h: Handlers): () => void {
  subscribers.add(h);
  return () => subscribers.delete(h);
}

const emit = (changed: Job[]) => changed.length && subscribers.forEach((s) => s.updated(structuredClone(changed)));

setInterval(() => {
  const changed: Job[] = [];
  const running = jobs.filter((j) => ["starting", "downloading", "processing"].includes(j.status));
  for (const j of jobs.filter((x) => x.status === "queued").slice(0, Math.max(0, settings.maxConcurrent - running.length))) {
    Object.assign(j, { status: "downloading", startedAt: Date.now(), title: `Sample video ${j.id}`, stage: "Stream 1/2" });
    changed.push(j);
  }
  for (const j of jobs.filter((x) => x.status === "downloading" || x.status === "processing")) {
    const pct = Math.min(100, (j.progress.percent ?? 0) + 4 + Math.random() * 6);
    const total = 48_000_000;
    j.progress = { percent: pct, downloadedBytes: (total * pct) / 100, totalBytes: total, speed: 2_500_000 + Math.random() * 1e6, eta: Math.round(((100 - pct) / 100) * 20) };
    j.stage = pct < 50 ? "Stream 1/2" : pct < 95 ? "Stream 2/2" : "Merging formats";
    j.status = pct >= 95 ? "processing" : "downloading";
    if (pct >= 100) {
      const failed = j.url.includes("fail");
      Object.assign(j, { status: failed ? "failed" : "completed", stage: null, finishedAt: Date.now() });
      if (failed) j.error = "[youtube] abc: Video unavailable (simulated)";
      else j.outputPath = `C:\\Users\\you\\Downloads\\TubeDeck\\${j.title}.mp4`;
    }
    changed.push(j);
  }
  emit(changed);
}, 500);

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
    case "enqueue_jobs": {
      const created: Job[] = (a.urls as string[]).map((url) => ({
        id: `m${++seq}`,
        url,
        title: null,
        thumbnail: null,
        status: "queued",
        stage: null,
        progress: { percent: null, downloadedBytes: null, totalBytes: null, speed: null, eta: null },
        options: a.options as DownloadOptions,
        presetName: (a.presetName as string | null) ?? null,
        outputPath: null,
        createdAt: Date.now(),
        startedAt: null,
        finishedAt: null,
        error: null,
      }));
      jobs.push(...created);
      return r(created);
    }
    case "list_jobs":
      return r(jobs);
    case "cancel_job":
    case "retry_job": {
      const j = jobs.find((x) => x.id === a.id);
      if (j) {
        if (cmd === "cancel_job") Object.assign(j, { status: "canceled", stage: null, finishedAt: Date.now() });
        else Object.assign(j, { status: "queued", error: null, finishedAt: null, progress: { percent: null, downloadedBytes: null, totalBytes: null, speed: null, eta: null } });
        emit([j]);
      }
      return r(undefined);
    }
    case "remove_job":
    case "clear_finished_jobs": {
      const ids = cmd === "remove_job" ? [a.id as string] : jobs.filter((j) => ["completed", "failed", "canceled"].includes(j.status)).map((j) => j.id);
      jobs = jobs.filter((j) => !ids.includes(j.id));
      subscribers.forEach((s) => s.removed(ids));
      return r(undefined);
    }
    case "get_job_log":
      return r([`[TubeDeck] yt-dlp … -- ${jobs.find((j) => j.id === a.id)?.url ?? ""}`, "[youtube] Extracting URL (simulated)"]);
    case "app_paths":
      return r({ configDir: "", logDir: "", managedBinDir: "", defaultDownloadFolder: settings.downloadFolder });
    default:
      return Promise.reject(`mock: unknown command ${cmd}`);
  }
}
