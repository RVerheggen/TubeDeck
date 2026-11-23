import type { DownloadOptions, Preset, Settings } from "./types";

export function sameOptions(a: DownloadOptions, b: DownloadOptions): boolean {
  return (Object.keys(a) as (keyof DownloadOptions)[]).every((k) => a[k] === b[k]);
}

/** The preset last applied, and whether the current options were changed since. */
export function presetState(settings: Settings, presets: Preset[]): { preset: Preset | null; modified: boolean } {
  const preset = presets.find((p) => p.id === settings.activePresetId) ?? null;
  return { preset, modified: !!preset && !sameOptions(settings.currentOptions, preset.options) };
}

/** Name recorded on new downloads: only the plain preset name when its options are unchanged. */
export function jobPresetName(settings: Settings, presets: Preset[]): string | null {
  const { preset, modified } = presetState(settings, presets);
  if (!preset) return null;
  return modified ? `${preset.name} (modified)` : preset.name;
}
