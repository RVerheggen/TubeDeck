// Browser-only stand-in for the Rust backend so `npm run dev` renders without Tauri.
// It is intentionally simple: the real command builder lives in Rust (args.rs).

import type { BuiltCommand, DownloadOptions, Job, Preset, Settings } from "./types";

// ---- Simulated download engine (browser preview only) ------------------------------

type Handlers = { updated: (jobs: Job[]) => void; removed: (ids: string[]) => void; order: (ids: string[]) => void };
const subscribers = new Set<Handlers>();
let jobs: Job[] = [];
let archived: Job[] = [];
let seq = 0;

type EngineHandlers = { install: (p: import("./types").InstallProgress) => void; notice: (m: string) => void };
const engineSubs = new Set<EngineHandlers>();
const installed = new Set<string>();
export function mockSubscribeEngine(h: EngineHandlers): () => void {
  engineSubs.add(h);
  return () => engineSubs.delete(h);
}

export function mockSubscribe(h: Handlers): () => void {
  subscribers.add(h);
  return () => subscribers.delete(h);
}

const emit = (changed: Job[]) => changed.length && subscribers.forEach((s) => s.updated(structuredClone(changed)));

setInterval(() => {
  const changed: Job[] = [];
  const running = jobs.filter((j) => ["starting", "downloading", "processing"].includes(j.status));
  const ready = jobs.filter((x) => x.status === "queued" && (x.retryAt == null || x.retryAt <= Date.now()));
  for (const j of ready.slice(0, Math.max(0, settings.maxConcurrent - running.length))) {
    Object.assign(j, { status: "downloading", startedAt: Date.now(), retryAt: null, title: j.title ?? `Sample video ${j.id}`, stage: "Stream 1/2" });
    changed.push(j);
  }
  for (const j of jobs.filter((x) => x.status === "downloading" || x.status === "processing")) {
    const pct = Math.min(100, (j.progress.percent ?? 0) + 4 + Math.random() * 6);
    const total = 48_000_000;
    j.progress = { percent: pct, downloadedBytes: (total * pct) / 100, totalBytes: total, speed: 2_500_000 + Math.random() * 1e6, eta: Math.round(((100 - pct) / 100) * 20) };
    j.stage = pct < 50 ? "Stream 1/2" : pct < 95 ? "Stream 2/2" : "Merging formats";
    j.status = pct >= 95 ? "processing" : "downloading";
    if (pct >= 40 && j.url.includes("retry") && j.attempts < settings.autoRetry) {
      Object.assign(j, {
        status: "queued", attempts: j.attempts + 1, retryAt: Date.now() + 5000, stage: `Retry ${j.attempts + 1}/${settings.autoRetry}`,
        error: "unable to download video data: HTTP Error 403: Forbidden (simulated)", errorKind: "http403",
        errorHint: "The site refused the download. This is usually temporary; if it keeps happening, update yt-dlp.",
      });
    } else if (pct >= 100) {
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
  autoRetry: 3,
  resumeOnStartup: true,
  ytdlpChannel: "stable",
  autoUpdate: true,
  lastUpdateCheck: 0,
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
      const st = (name: string, id: string, version: string) =>
        installed.has(id)
          ? { name, found: true, path: "C:\\Users\\you\\AppData\\Local\\com.tubedeck.desktop\\bin\\" + id + ".exe", source: "managed", version, ok: true, note: null }
          : { name, found: false, path: null, source: null, version: null, ok: false, note: "Not found" };
      return r({ ytdlp: st("yt-dlp", "ytdlp", "2026.10.26"), ffmpeg: st("FFmpeg", "ffmpeg", "N-121000-gabc"), deno: st("Deno", "deno", "2.9.8"), managedDir: "" });
    }
    case "install_binary": {
      const component = a.component as "ytdlp" | "ffmpeg" | "deno";
      const total = { ytdlp: 23_600_000, ffmpeg: 91_000_000, deno: 42_900_000 }[component];
      const send = (p: Partial<import("./types").InstallProgress>) =>
        engineSubs.forEach((s) => s.install({ component, phase: "downloading", downloaded: 0, total, message: null, ...p }));
      for (let d = 0; d < total; d += total / 12) {
        send({ downloaded: d });
        await new Promise((res) => setTimeout(res, 150));
      }
      send({ phase: "verifying", downloaded: total });
      await new Promise((res) => setTimeout(res, 300));
      installed.add(component);
      send({ phase: "done", downloaded: total });
      return r(undefined);
    }
    case "check_updates": {
      const u = (id: string, inst: string, latest: string | null) => ({ installed: installed.has(id) ? inst : null, latest, managed: installed.has(id), updateAvailable: installed.has(id) && id === "ytdlp", error: null });
      return r({ ytdlp: u("ytdlp", "2026.10.26", "2026.11.02"), deno: u("deno", "2.9.8", "2.9.8"), ffmpeg: u("ffmpeg", "N-121000-gabc", null), checkedAt: Date.now() });
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
        errorKind: null,
        errorHint: null,
        attempts: 0,
        retryAt: null,
        group: null,
      }));
      jobs.push(...created);
      return r(created);
    }
    case "list_jobs":
      return r(jobs);
    case "cancel_job":
    case "pause_job":
    case "resume_job":
    case "retry_job": {
      const j = jobs.find((x) => x.id === a.id);
      if (j) {
        if (cmd === "cancel_job") Object.assign(j, { status: "canceled", stage: null, retryAt: null, finishedAt: Date.now() });
        else if (cmd === "pause_job") Object.assign(j, { status: "paused", stage: "Paused", retryAt: null });
        else if (cmd === "resume_job") Object.assign(j, { status: "queued", stage: null });
        else Object.assign(j, { status: "queued", error: null, errorKind: null, errorHint: null, attempts: 0, finishedAt: null, progress: { percent: null, downloadedBytes: null, totalBytes: null, speed: null, eta: null } });
        emit([j]);
      }
      return r(undefined);
    }
    case "pause_all_jobs":
    case "resume_all_jobs": {
      const from = cmd === "pause_all_jobs" ? ["queued", "downloading", "processing", "starting"] : ["paused"];
      const changed = jobs.filter((j) => from.includes(j.status));
      changed.forEach((j) => Object.assign(j, cmd === "pause_all_jobs" ? { status: "paused", stage: "Paused", retryAt: null } : { status: "queued", stage: null }));
      emit(changed);
      return r(undefined);
    }
    case "move_job": {
      const i = jobs.findIndex((j) => j.id === a.id);
      if (i >= 0) {
        const [j] = jobs.splice(i, 1);
        jobs.splice(Math.min(Number(a.to), jobs.length), 0, j);
        subscribers.forEach((s) => s.order(jobs.map((x) => x.id)));
      }
      return r(undefined);
    }
    case "remove_job":
    case "clear_finished_jobs": {
      const ids = cmd === "remove_job" ? [a.id as string] : jobs.filter((j) => ["completed", "failed", "canceled"].includes(j.status)).map((j) => j.id);
      archived.push(...jobs.filter((j) => ids.includes(j.id) && ["completed", "failed", "canceled"].includes(j.status)));
      jobs = jobs.filter((j) => !ids.includes(j.id));
      subscribers.forEach((s) => s.removed(ids));
      return r(undefined);
    }
    case "get_job_log":
      return r([`[TubeDeck] yt-dlp … -- ${jobs.find((j) => j.id === a.id)?.url ?? ""}`, "[youtube] Extracting URL (simulated)"]);
    case "probe_url": {
      await new Promise((res) => setTimeout(res, 700));
      const u = String(a.url);
      if (!/list=|@|playlist/i.test(u)) {
        return r({ kind: "video", id: "abc", title: "Sample single video", url: u, duration: 212, thumbnail: null, uploader: "Sample channel" });
      }
      const entries = Array.from({ length: 24 }, (_, i) => ({
        index: i + 1, id: `v${i + 1}`, title: i === 6 ? "[Private video]" : `Sample episode ${i + 1}: something interesting happens`,
        url: i === 6 ? null : `https://www.youtube.com/watch?v=sample${i + 1}`, duration: 300 + i * 37, thumbnail: null,
        isPlaylist: false, unavailable: i === 6,
      }));
      return r({ kind: "playlist", id: "PLsample", title: "Sample Playlist", url: u, uploader: "Sample channel", entries });
    }
    case "cancel_probe":
      return r(undefined);
    case "enqueue_playlist": {
      const req = a.request as import("./types").PlaylistRequest;
      const gid = `g${Date.now().toString(36)}`;
      const created: Job[] = [];
      for (const it of req.items) {
        const [j] = (await mock<Job[]>("enqueue_jobs", { urls: [it.url], options: req.options, presetName: req.presetName }));
        const real = jobs.find((x) => x.id === j.id)!;
        Object.assign(real, { title: it.title, group: { id: gid, playlistId: req.playlistId, title: req.title, index: it.index, count: req.count } });
        created.push(structuredClone(real));
      }
      return r(created);
    }
    case "list_history": {
      const q = a.query as { search: string; status: string; limit: number; offset: number };
      const all = [...jobs.filter((j) => ["completed", "failed", "canceled"].includes(j.status)), ...archived]
        .filter((j) => q.status === "all" || j.status === q.status)
        .filter((j) => !q.search || `${j.title} ${j.url}`.toLowerCase().includes(q.search.toLowerCase()))
        .sort((x, y) => (y.finishedAt ?? 0) - (x.finishedAt ?? 0));
      return r({ items: all.slice(q.offset, q.offset + (q.limit || 50)), total: all.length });
    }
    case "delete_history":
    case "clear_history": {
      const ids = cmd === "clear_history" ? null : (a.ids as string[]);
      const gone = (j: Job) => ["completed", "failed", "canceled"].includes(j.status) && (!ids || ids.includes(j.id));
      const removed = jobs.filter(gone).map((j) => j.id);
      jobs = jobs.filter((j) => !gone(j));
      archived = archived.filter((j) => !gone(j));
      if (removed.length) subscribers.forEach((s) => s.removed(removed));
      return r(undefined);
    }
    case "redownload": {
      const old = [...jobs, ...archived].find((j) => j.id === a.id)!;
      return mock<T>("enqueue_jobs", { urls: [old.url], options: old.options, presetName: old.presetName }).then((c) => (c as unknown as Job[])[0] as T);
    }
    case "app_paths":
      return r({ dataDir: "", configDir: "", logDir: "", managedBinDir: "", defaultDownloadFolder: settings.downloadFolder });
    default:
      return Promise.reject(`mock: unknown command ${cmd}`);
  }
}
