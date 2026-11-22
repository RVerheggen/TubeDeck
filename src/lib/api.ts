// Typed wrappers around Tauri commands. Outside the desktop shell (plain `npm run dev`
// in a browser) calls fall back to a small mock so the UI can still be developed.

import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  AppLog,
  AppPaths,
  BuiltCommand,
  DownloadOptions,
  EngineStatus,
  HistoryPage,
  HistoryQuery,
  Job,
  ParsedUrl,
  Preset,
  Settings,
} from "./types";
import { mock, mockSubscribe } from "./mock";

export const inDesktop = isTauri();

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inDesktop) return mock<T>(cmd, args);
  return invoke<T>(cmd, args);
}

export const api = {
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<Settings>("save_settings", { settings }),
  buildCommand: (url: string, options: DownloadOptions, defaultFolder?: string) =>
    call<BuiltCommand>("build_command", { url, options, defaultFolder }),
  parseUrls: (text: string) => call<ParsedUrl[]>("parse_urls", { text }),
  listPresets: () => call<Preset[]>("list_presets"),
  savePreset: (preset: Preset) => call<Preset>("save_preset", { preset }),
  deletePreset: (id: string) => call<void>("delete_preset", { id }),
  engineStatus: () => call<EngineStatus>("engine_status"),
  readAppLog: (maxLines = 500) => call<AppLog>("read_app_log", { maxLines }),
  appPaths: () => call<AppPaths>("app_paths"),

  enqueueJobs: (urls: string[], options: DownloadOptions, presetName: string | null) =>
    call<Job[]>("enqueue_jobs", { urls, options, presetName }),
  listJobs: () => call<Job[]>("list_jobs"),
  cancelJob: (id: string) => call<void>("cancel_job", { id }),
  pauseJob: (id: string) => call<void>("pause_job", { id }),
  resumeJob: (id: string) => call<void>("resume_job", { id }),
  pauseAllJobs: () => call<void>("pause_all_jobs"),
  resumeAllJobs: () => call<void>("resume_all_jobs"),
  moveJob: (id: string, to: number) => call<void>("move_job", { id, to }),
  retryJob: (id: string) => call<void>("retry_job", { id }),
  removeJob: (id: string) => call<void>("remove_job", { id }),
  clearFinishedJobs: () => call<void>("clear_finished_jobs"),
  getJobLog: (id: string) => call<string[]>("get_job_log", { id }),

  listHistory: (query: HistoryQuery) => call<HistoryPage>("list_history", { query }),
  deleteHistory: (ids: string[]) => call<void>("delete_history", { ids }),
  clearHistory: () => call<void>("clear_history"),
  redownload: (id: string) => call<Job>("redownload", { id }),
};

/** Ask where to save, then write the diagnostics report. Returns the path or null. */
export async function exportDiagnostics(): Promise<string | null> {
  if (!inDesktop) return null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  const stamp = new Date().toISOString().slice(0, 16).replace(/[:T]/g, "-");
  const path = await save({
    defaultPath: `tubedeck-diagnostics-${stamp}.txt`,
    filters: [{ name: "Text", extensions: ["txt"] }],
  });
  if (!path) return null;
  await invoke("export_diagnostics", { path });
  return path;
}

/** Subscribe to engine events. Returns an unsubscribe function. */
export async function onJobEvents(handlers: {
  updated: (jobs: Job[]) => void;
  removed: (ids: string[]) => void;
  order: (ids: string[]) => void;
}): Promise<() => void> {
  if (!inDesktop) return mockSubscribe(handlers);
  const { listen } = await import("@tauri-apps/api/event");
  const offs = await Promise.all([
    listen<Job[]>("jobs://updated", (e) => handlers.updated(e.payload)),
    listen<string[]>("jobs://removed", (e) => handlers.removed(e.payload)),
    listen<string[]>("jobs://order", (e) => handlers.order(e.payload)),
  ]);
  return () => offs.forEach((off) => off());
}

export async function copyText(text: string): Promise<void> {
  if (inDesktop) {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
  } else {
    await navigator.clipboard.writeText(text);
  }
}

export async function pickFolder(defaultPath?: string): Promise<string | null> {
  if (!inDesktop) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const result = await open({ directory: true, multiple: false, defaultPath: defaultPath || undefined });
  return typeof result === "string" ? result : null;
}

export async function revealInExplorer(path: string): Promise<void> {
  if (!inDesktop) return;
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
  await revealItemInDir(path);
}

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}
