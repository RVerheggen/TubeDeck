// Read CHANGELOG.md ("## 0.1.4 (2026-11-24)" sections) for the update dialog.

export interface ChangelogSection {
  version: string;
  date: string | null;
  /** Markdown between this heading and the next "## ". */
  body: string;
}

export const REPO_URL = "https://github.com/RVerheggen/TubeDeck";
export const releaseUrl = (version: string) => `${REPO_URL}/releases/tag/v${version}`;
export const changelogUrl = (version: string) => `${REPO_URL}/blob/v${version}/CHANGELOG.md`;

const HEADING = /^## v?(\d+(?:\.\d+)*)(?:\s+\(([^)]*)\))?\s*$/;

export function parseChangelog(markdown: string): ChangelogSection[] {
  const sections: ChangelogSection[] = [];
  let current: ChangelogSection | null = null;
  const lines: string[] = [];
  const flush = () => {
    if (current) sections.push({ ...current, body: lines.join("\n").trim() });
    lines.length = 0;
  };
  for (const line of markdown.split(/\r?\n/)) {
    if (line.startsWith("## ")) {
      flush();
      const m = HEADING.exec(line);
      // "## Unreleased" and other non-version headings end the previous section only.
      current = m ? { version: m[1], date: m[2] ?? null, body: "" } : null;
    } else if (current) {
      lines.push(line);
    }
  }
  flush();
  return sections;
}

/** Numeric version comparison ("0.1.10" > "0.1.9"). */
export function compareVersions(a: string, b: string): number {
  const pa = a.replace(/^v/, "").split(".").map(Number);
  const pb = b.replace(/^v/, "").split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** Sections newer than `installed`, up to and including `available`, newest first. */
export function changesSince(markdown: string, installed: string, available: string): ChangelogSection[] {
  return parseChangelog(markdown)
    .filter((s) => compareVersions(s.version, installed) > 0 && compareVersions(s.version, available) <= 0)
    .sort((a, b) => compareVersions(b.version, a.version));
}
