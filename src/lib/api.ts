// Typed wrappers around Tauri commands. Outside the desktop shell (plain `npm run dev`
// in a browser) calls fall back to a small mock so the UI can still be developed.

import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  AppLog,
  AppPaths,
  BuiltCommand,
  DownloadOptions,
  EngineStatus,
  ParsedUrl,
  Preset,
  Settings,
} from "./types";
import { mock } from "./mock";

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
};

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
