import { describe, expect, it } from "vitest";
import { defaultOptions } from "./mock";
import { groupJobs } from "./queue";
import { optionsSummary } from "./summary";
import type { Job } from "./types";
import { sameOptions } from "../store/useAppStore";

function job(id: string, group?: string, index = 1): Job {
  return {
    id,
    url: `https://example.com/${id}`,
    title: id,
    thumbnail: null,
    status: "queued",
    stage: null,
    progress: { percent: null, downloadedBytes: null, totalBytes: null, speed: null, eta: null },
    options: defaultOptions,
    presetName: null,
    outputPath: null,
    createdAt: 0,
    startedAt: null,
    finishedAt: null,
    error: null,
    errorKind: null,
    errorHint: null,
    attempts: 0,
    retryAt: null,
    group: group ? { id: group, playlistId: null, title: group, index, count: 10 } : null,
  };
}

describe("groupJobs", () => {
  it("keeps singles in place and gathers group members at the first member", () => {
    const entries = groupJobs([job("a"), job("p1", "G", 1), job("b"), job("p2", "G", 2), job("c")]);
    expect(entries.map((e) => ("job" in e ? e.job.id : `[${e.jobs.map((j) => j.id).join(",")}]`))).toEqual([
      "a",
      "[p1,p2]",
      "b",
      "c",
    ]);
  });

  it("keeps two adds of the same playlist apart", () => {
    const entries = groupJobs([job("x1", "G1"), job("y1", "G2"), job("x2", "G1")]);
    expect(entries).toHaveLength(2);
  });

  it("handles an empty queue", () => {
    expect(groupJobs([])).toEqual([]);
  });
});

describe("optionsSummary", () => {
  it("describes video and audio settings", () => {
    expect(optionsSummary(defaultOptions)).toEqual(["Best quality", "MP4"]);
    expect(optionsSummary({ ...defaultOptions, quality: "1080", videoCodec: "av1", subtitles: "embed" })).toEqual([
      "≤ 1080p",
      "MP4",
      "AV1",
      "Subs embedded",
    ]);
    expect(optionsSummary({ ...defaultOptions, mode: "audio", audioFormat: "mp3", cookiesFromBrowser: "firefox" })).toEqual([
      "Audio only",
      "MP3",
      "Cookies: firefox",
    ]);
  });
});

describe("sameOptions", () => {
  it("compares every field", () => {
    expect(sameOptions(defaultOptions, { ...defaultOptions })).toBe(true);
    expect(sameOptions(defaultOptions, { ...defaultOptions, fragments: 8 })).toBe(false);
  });
});
