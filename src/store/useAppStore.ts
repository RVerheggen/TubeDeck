import { create } from "zustand";
import { api, errorMessage, onJobEvents } from "../lib/api";
import type {
  DownloadOptions,
  EngineStatus,
  Job,
  Preset,
  Settings,
  ViewId,
} from "../lib/types";

export interface Toast {
  id: number;
  kind: "info" | "success" | "error";
  message: string;
}

interface AppState {
  ready: boolean;
  loadError: string | null;
  view: ViewId;
  settings: Settings | null;
  presets: Preset[];
  engine: EngineStatus | null;
  engineLoading: boolean;
  urlDraft: string;
  jobs: Job[];
  toasts: Toast[];

  init: () => Promise<void>;
  setView: (view: ViewId) => void;
  setUrlDraft: (text: string) => void;
  updateSettings: (patch: Partial<Settings>) => void;
  setOptions: (patch: Partial<DownloadOptions>) => void;
  applyPreset: (id: string) => void;
  savePresetAs: (name: string, description: string) => Promise<void>;
  updatePreset: (preset: Preset) => Promise<void>;
  deletePreset: (id: string) => Promise<void>;
  refreshEngine: () => Promise<void>;
  enqueue: (urls: string[]) => Promise<void>;
  applyJobUpdates: (jobs: Job[]) => void;
  applyJobRemovals: (ids: string[]) => void;
  cancelJob: (id: string) => Promise<void>;
  retryJob: (id: string) => Promise<void>;
  removeJob: (id: string) => Promise<void>;
  clearFinished: () => Promise<void>;
  toast: (kind: Toast["kind"], message: string) => void;
  dismissToast: (id: number) => void;
}

const SAVE_DELAY_MS = 400;
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let toastSeq = 0;

export function sameOptions(a: DownloadOptions, b: DownloadOptions): boolean {
  return (Object.keys(a) as (keyof DownloadOptions)[]).every((k) => a[k] === b[k]);
}

let initStarted = false;

export const useAppStore = create<AppState>((set, get) => {
  /** Persist settings after a short pause so typing doesn't write on every keystroke. */
  const scheduleSave = () => {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      const s = get().settings;
      if (!s) return;
      try {
        await api.saveSettings(s);
      } catch (e) {
        get().toast("error", `Could not save settings: ${errorMessage(e)}`);
      }
    }, SAVE_DELAY_MS);
  };

  /** Run a job command; the engine reports the resulting state through events. */
  const jobAction = async (fn: () => Promise<void>) => {
    try {
      await fn();
    } catch (e) {
      get().toast("error", errorMessage(e));
    }
  };

  return {
    ready: false,
    loadError: null,
    view: "downloads",
    settings: null,
    presets: [],
    engine: null,
    engineLoading: false,
    urlDraft: "",
    jobs: [],
    toasts: [],

    init: async () => {
      // React StrictMode runs effects twice in development; subscribe only once.
      if (initStarted) return;
      initStarted = true;
      try {
        // Subscribe before listing so no update between the two is missed.
        await onJobEvents({
          updated: (jobs) => get().applyJobUpdates(jobs),
          removed: (ids) => get().applyJobRemovals(ids),
        });
        const [settings, presets, jobs] = await Promise.all([api.getSettings(), api.listPresets(), api.listJobs()]);
        set({ settings, presets, jobs, ready: true });
        void get().refreshEngine();
      } catch (e) {
        set({ loadError: errorMessage(e), ready: true });
      }
    },

    setView: (view) => set({ view }),
    setUrlDraft: (urlDraft) => set({ urlDraft }),

    updateSettings: (patch) => {
      const s = get().settings;
      if (!s) return;
      set({ settings: { ...s, ...patch } });
      scheduleSave();
    },

    setOptions: (patch) => {
      const s = get().settings;
      if (!s) return;
      set({ settings: { ...s, currentOptions: { ...s.currentOptions, ...patch } } });
      scheduleSave();
    },

    applyPreset: (id) => {
      const s = get().settings;
      const p = get().presets.find((x) => x.id === id);
      if (!s || !p) return;
      set({ settings: { ...s, currentOptions: { ...p.options }, activePresetId: p.id } });
      scheduleSave();
    },

    savePresetAs: async (name, description) => {
      const s = get().settings;
      if (!s) return;
      try {
        const saved = await api.savePreset({
          id: "",
          name,
          description,
          builtin: false,
          options: s.currentOptions,
        });
        set({ presets: await api.listPresets() });
        get().updateSettings({ activePresetId: saved.id });
        get().toast("success", `Preset "${saved.name}" saved`);
      } catch (e) {
        get().toast("error", errorMessage(e));
      }
    },

    updatePreset: async (preset) => {
      try {
        await api.savePreset(preset);
        set({ presets: await api.listPresets() });
        get().toast("success", `Preset "${preset.name}" updated`);
      } catch (e) {
        get().toast("error", errorMessage(e));
      }
    },

    deletePreset: async (id) => {
      try {
        await api.deletePreset(id);
        const presets = await api.listPresets();
        set({ presets });
        if (get().settings?.activePresetId === id) get().updateSettings({ activePresetId: null });
      } catch (e) {
        get().toast("error", errorMessage(e));
      }
    },

    refreshEngine: async () => {
      set({ engineLoading: true });
      try {
        set({ engine: await api.engineStatus() });
      } catch (e) {
        get().toast("error", `Engine check failed: ${errorMessage(e)}`);
      } finally {
        set({ engineLoading: false });
      }
    },

    enqueue: async (urls) => {
      const s = get().settings;
      if (!s || urls.length === 0) return;
      const presetName = get().presets.find((p) => p.id === s.activePresetId)?.name ?? null;
      try {
        const created = await api.enqueueJobs(urls, s.currentOptions, presetName);
        get().applyJobUpdates(created);
        get().toast("success", `Added ${created.length} item${created.length > 1 ? "s" : ""} to the queue`);
      } catch (e) {
        get().toast("error", errorMessage(e));
      }
    },

    applyJobUpdates: (updated) => {
      if (updated.length === 0) return;
      const byId = new Map(updated.map((j) => [j.id, j]));
      const jobs = get().jobs.map((j) => byId.get(j.id) ?? j);
      const known = new Set(jobs.map((j) => j.id));
      set({ jobs: [...jobs, ...updated.filter((j) => !known.has(j.id))] });
    },

    applyJobRemovals: (ids) => {
      const gone = new Set(ids);
      set({ jobs: get().jobs.filter((j) => !gone.has(j.id)) });
    },

    cancelJob: (id) => jobAction(() => api.cancelJob(id)),
    retryJob: (id) => jobAction(() => api.retryJob(id)),
    removeJob: (id) => jobAction(() => api.removeJob(id)),
    clearFinished: () => jobAction(() => api.clearFinishedJobs()),

    toast: (kind, message) => {
      const id = ++toastSeq;
      set({ toasts: [...get().toasts, { id, kind, message }] });
      setTimeout(() => get().dismissToast(id), kind === "error" ? 7000 : 3500);
    },
    dismissToast: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
  };
});

/** Convenience selector: the currently edited options (non-null after init). */
export const useOptions = () => useAppStore((s) => s.settings!.currentOptions);
