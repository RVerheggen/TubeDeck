const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return "—";
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i === 0 || v >= 100 ? 0 : 1)} ${UNITS[i]}`;
}

export function formatSpeed(bytesPerSec: number | null | undefined): string {
  if (bytesPerSec == null || !Number.isFinite(bytesPerSec) || bytesPerSec <= 0) return "—";
  return `${formatBytes(bytesPerSec)}/s`;
}

export function formatEta(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return "—";
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) return `${h}h ${m.toString().padStart(2, "0")}m`;
  if (m > 0) return `${m}m ${sec.toString().padStart(2, "0")}s`;
  return `${sec}s`;
}

export function formatTime(ms: number): string {
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Short host + path for display, e.g. "youtube.com/watch?v=abc". */
export function shortUrl(url: string): string {
  try {
    const u = new URL(url);
    return `${u.hostname.replace(/^www\./, "")}${u.pathname === "/" ? "" : u.pathname}${u.search}`;
  } catch {
    return url;
  }
}
