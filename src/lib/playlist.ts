// Playlist helpers for the picker: URL detection, range parsing, durations.

const PATTERNS: RegExp[] = [
  /[?&]list=[\w-]+/i, // YouTube playlists and watch URLs inside a playlist
  /youtube\.com\/(@[^/?#]+|channel\/|c\/|user\/)[^?#]*$/i, // YouTube channels and their tabs
  /youtube\.com\/(playlist|browse)\b/i,
  /soundcloud\.com\/[^/]+\/sets\//i,
  /vimeo\.com\/(showcase|channels|album)\//i,
  /bandcamp\.com\/album\//i,
  /twitch\.tv\/[^/]+\/videos/i,
  /\/playlists?\b/i,
];

/** Best guess whether a URL lists several videos (so the picker should open). */
export function looksLikePlaylist(url: string): boolean {
  return PATTERNS.some((p) => p.test(url));
}

/** True for a watch URL that names one video inside a playlist (`?v=…&list=…`). */
export function isVideoInPlaylist(url: string): boolean {
  return /[?&]v=[\w-]+/i.test(url) && /[?&]list=[\w-]+/i.test(url);
}

/**
 * Parse "1-5, 8, 12-" into a set of 1-based indexes within 1..max.
 * Returns null when the text is not a valid range list.
 */
export function parseRange(text: string, max: number): Set<number> | null {
  const out = new Set<number>();
  const parts = text.split(",").map((p) => p.trim()).filter(Boolean);
  if (parts.length === 0) return null;
  for (const part of parts) {
    const m = /^(\d*)\s*(-)?\s*(\d*)$/.exec(part);
    if (!m || (!m[1] && !m[3])) return null;
    const from = m[1] ? Number(m[1]) : 1;
    const to = m[2] ? (m[3] ? Number(m[3]) : max) : from;
    if (from < 1 || to < from) return null;
    for (let i = from; i <= Math.min(to, max); i++) out.add(i);
  }
  return out;
}

export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) return "";
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = (s % 60).toString().padStart(2, "0");
  return h > 0 ? `${h}:${m.toString().padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}
