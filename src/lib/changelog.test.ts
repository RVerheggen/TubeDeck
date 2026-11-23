import { describe, expect, it } from "vitest";
import { changesSince, compareVersions, parseChangelog } from "./changelog";

const md = `# Changelog

## Unreleased

### Fixed
- Not released yet.

## 0.1.10 (2026-12-01)

### Fixed
- Ten.

## 0.1.4 (2026-11-24)

### Fixed
- Preset list.

## 0.1.3 (2026-11-23)

### Fixed
- Paths.

## 0.1.2 (2026-11-22)

### Changed
- Version in the corner.
`;

describe("changelog", () => {
  it("parses version sections and skips Unreleased", () => {
    const s = parseChangelog(md);
    expect(s.map((x) => x.version)).toEqual(["0.1.10", "0.1.4", "0.1.3", "0.1.2"]);
    expect(s[1]).toEqual({ version: "0.1.4", date: "2026-11-24", body: "### Fixed\n- Preset list." });
  });

  it("compares versions numerically", () => {
    expect(compareVersions("0.1.10", "0.1.9")).toBeGreaterThan(0);
    expect(compareVersions("v0.1.4", "0.1.4")).toBe(0);
    expect(compareVersions("0.2", "0.1.9")).toBeGreaterThan(0);
  });

  it("lists every version after the installed one, up to the available one", () => {
    expect(changesSince(md, "0.1.2", "0.1.4").map((x) => x.version)).toEqual(["0.1.4", "0.1.3"]);
    expect(changesSince(md, "0.1.3", "0.1.4").map((x) => x.version)).toEqual(["0.1.4"]);
    expect(changesSince(md, "0.1.4", "0.1.4")).toEqual([]);
  });

  it("handles Windows line endings", () => {
    expect(parseChangelog(md.replace(/\n/g, "\r\n"))[1].body).toBe("### Fixed\n- Preset list.");
  });
});
