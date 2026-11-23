// TubeDeck self-update via tauri-plugin-updater.
// The app reads latest.json from the newest *published* GitHub release, downloads the
// installer, verifies its signature against the public key in tauri.conf.json, installs it
// (passive NSIS, no admin) and restarts.

import { invoke } from "@tauri-apps/api/core";
import { inDesktop } from "./api";

export interface AppUpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
  date: string | null;
}

// The plugin's Update object holds a native resource; keep it out of the (serializable) store.
let pending: import("@tauri-apps/plugin-updater").Update | null = null;

/**
 * After an update, TubeDeck is started by the installer in the background, and Windows
 * opens it behind other windows. Returns the "updated" message (once) and brings the window
 * to the front; returns null on a normal start.
 */
export async function afterUpdate(): Promise<string | null> {
  if (!inDesktop) return null;
  const message = await invoke<string | null>("take_update_notice");
  if (!message) return null;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  try {
    await win.unminimize();
    await win.show();
    // Windows refuses a plain focus request from a background-started app; briefly making
    // the window topmost is the usual way to get it in front of the others.
    await win.setAlwaysOnTop(true);
    await win.setFocus();
    setTimeout(() => void win.setAlwaysOnTop(false), 400);
  } catch {
    /* focusing is best effort */
  }
  return message;
}

export async function appVersion(): Promise<string> {
  if (!inDesktop) return "0.1.2";
  const { getVersion } = await import("@tauri-apps/api/app");
  return getVersion();
}

/** CHANGELOG.md as published with release `version` (fetched by the backend). */
export async function fetchChangelog(version: string): Promise<string> {
  if (!inDesktop) return (await import("../../CHANGELOG.md?raw")).default;
  return invoke<string>("app_changelog", { version });
}

/** Returns the available update, or null when TubeDeck is up to date. */
export async function checkAppUpdate(): Promise<AppUpdateInfo | null> {
  if (!inDesktop) {
    // Browser preview: pretend an update exists so the UI can be designed.
    return { version: "0.1.4", currentVersion: "0.1.2", notes: null, date: "2026-11-24T00:00:00Z" };
  }
  const { check } = await import("@tauri-apps/plugin-updater");
  const update = await check({ timeout: 30_000 });
  pending = update;
  if (!update) return null;
  return { version: update.version, currentVersion: update.currentVersion, notes: update.body ?? null, date: update.date ?? null };
}

/**
 * Download and install the pending update, reporting 0–100 (null while the size is unknown).
 * Running downloads are saved first so they resume after the restart.
 */
export async function installAppUpdate(onProgress: (percent: number | null) => void): Promise<void> {
  if (!inDesktop) {
    for (let p = 0; p <= 100; p += 10) {
      onProgress(p);
      await new Promise((r) => setTimeout(r, 150));
    }
    return;
  }
  if (!pending) throw new Error("No update has been found yet; check again.");
  let total = 0;
  let done = 0;
  // Download (and verify) first; only then stop the engine, so a failed download leaves
  // TubeDeck fully working.
  await pending.download((event) => {
    if (event.event === "Started") {
      total = event.data.contentLength ?? 0;
      onProgress(total ? 0 : null);
    } else if (event.event === "Progress") {
      done += event.data.chunkLength;
      onProgress(total ? Math.min(100, (done / total) * 100) : null);
    } else if (event.event === "Finished") {
      onProgress(100);
    }
  });
  await invoke("prepare_for_update");
  await pending.install();
  // On Windows the installer usually closes TubeDeck itself; restart in case it didn't.
  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}
