import { describe, expect, it } from "vitest";
import { defaultOptions } from "./mock";
import { jobPresetName, presetState } from "./presets";
import type { Preset, Settings } from "./types";

const mp3: Preset = {
  id: "builtin-audio-mp3",
  name: "Audio MP3",
  description: "",
  builtin: true,
  options: { ...defaultOptions, mode: "audio", audioFormat: "mp3" },
};
const settings = (over: Partial<Settings>) => ({ activePresetId: mp3.id, currentOptions: mp3.options, ...over }) as Settings;

describe("preset state", () => {
  it("names the preset while its options are unchanged", () => {
    const s = settings({});
    expect(presetState(s, [mp3]).modified).toBe(false);
    expect(jobPresetName(s, [mp3])).toBe("Audio MP3");
  });

  it("marks the preset modified after switching to video (the mp3-downloads-mp4 bug)", () => {
    const s = settings({ currentOptions: { ...mp3.options, mode: "video" } });
    expect(presetState(s, [mp3])).toEqual({ preset: mp3, modified: true });
    expect(jobPresetName(s, [mp3])).toBe("Audio MP3 (modified)");
  });

  it("has no name without an active preset", () => {
    expect(jobPresetName(settings({ activePresetId: null }), [mp3])).toBeNull();
    expect(jobPresetName(settings({ activePresetId: "deleted" }), [mp3])).toBeNull();
  });
});
