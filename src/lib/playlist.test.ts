import { describe, expect, it } from "vitest";
import { formatDuration, isVideoInPlaylist, looksLikePlaylist, parseRange } from "./playlist";

describe("looksLikePlaylist", () => {
  it.each([
    "https://www.youtube.com/playlist?list=PLav47HAVZMjkxzzLDqYpTCosbsWjosUN0",
    "https://www.youtube.com/watch?v=abc&list=PLx",
    "https://www.youtube.com/@BlenderStudio",
    "https://www.youtube.com/@BlenderStudio/videos",
    "https://www.youtube.com/channel/UC123/videos",
    "https://soundcloud.com/artist/sets/album-name",
    "https://vimeo.com/showcase/123",
    "https://artist.bandcamp.com/album/name",
  ])("detects %s", (u) => expect(looksLikePlaylist(u)).toBe(true));

  it.each([
    "https://www.youtube.com/watch?v=jNQXAC9IVRw",
    "https://youtu.be/jNQXAC9IVRw",
    "https://www.youtube.com/shorts/abc",
    "https://vimeo.com/76979871",
    "https://soundcloud.com/artist/track",
  ])("ignores %s", (u) => expect(looksLikePlaylist(u)).toBe(false));

  it("detects a video inside a playlist", () => {
    expect(isVideoInPlaylist("https://www.youtube.com/watch?v=abc&list=PLx")).toBe(true);
    expect(isVideoInPlaylist("https://www.youtube.com/playlist?list=PLx")).toBe(false);
  });
});

describe("parseRange", () => {
  it("parses lists and open ranges", () => {
    expect([...parseRange("1-3, 5", 10)!]).toEqual([1, 2, 3, 5]);
    expect([...parseRange("8-", 10)!]).toEqual([8, 9, 10]);
    expect([...parseRange("-2", 10)!]).toEqual([1, 2]);
    expect([...parseRange("9-20", 10)!]).toEqual([9, 10]);
  });
  it("rejects nonsense", () => {
    expect(parseRange("", 10)).toBeNull();
    expect(parseRange("a-b", 10)).toBeNull();
    expect(parseRange("5-2", 10)).toBeNull();
    expect(parseRange("0", 10)).toBeNull();
    expect(parseRange("-", 10)).toBeNull();
  });
});

describe("formatDuration", () => {
  it("formats", () => {
    expect(formatDuration(19)).toBe("0:19");
    expect(formatDuration(855)).toBe("14:15");
    expect(formatDuration(3725)).toBe("1:02:05");
    expect(formatDuration(null)).toBe("");
  });
});
