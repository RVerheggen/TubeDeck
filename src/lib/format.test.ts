import { describe, expect, it } from "vitest";
import { formatBytes, formatEta, formatSpeed, shortUrl } from "./format";

describe("format", () => {
  it("formats bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(250 * 1024 * 1024)).toBe("250 MB");
    expect(formatBytes(null)).toBe("—");
    expect(formatBytes(-1)).toBe("—");
  });

  it("formats speed", () => {
    expect(formatSpeed(2 * 1024 * 1024)).toBe("2.0 MB/s");
    expect(formatSpeed(0)).toBe("—");
  });

  it("formats eta", () => {
    expect(formatEta(5)).toBe("5s");
    expect(formatEta(65)).toBe("1m 05s");
    expect(formatEta(3725)).toBe("1h 02m");
    expect(formatEta(null)).toBe("—");
  });

  it("shortens urls", () => {
    expect(shortUrl("https://www.youtube.com/watch?v=abc")).toBe("youtube.com/watch?v=abc");
    expect(shortUrl("not a url")).toBe("not a url");
  });
});
