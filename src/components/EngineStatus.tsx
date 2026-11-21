import { useAppStore } from "../store/useAppStore";
import type { EngineStatus as Engine } from "../lib/types";

export type EngineHealth = "ok" | "degraded" | "missing" | "unknown";

export function engineHealth(e: Engine | null): EngineHealth {
  if (!e) return "unknown";
  if (!e.ytdlp.ok) return "missing";
  if (!e.ffmpeg.ok || !e.deno.ok) return "degraded";
  return "ok";
}

const LABEL: Record<EngineHealth, string> = {
  ok: "Engine ready",
  degraded: "Engine limited",
  missing: "Setup needed",
  unknown: "Checking engine…",
};

export function EngineStatus() {
  const engine = useAppStore((s) => s.engine);
  const loading = useAppStore((s) => s.engineLoading);
  const setView = useAppStore((s) => s.setView);
  const health = loading && !engine ? "unknown" : engineHealth(engine);

  const detail = engine
    ? engine.ytdlp.ok
      ? `yt-dlp ${engine.ytdlp.version}`
      : "yt-dlp not found"
    : "Detecting binaries";

  return (
    <button className="engine-status" onClick={() => setView("settings")} title="Open engine settings">
      <span className={`status-dot ${health}`} />
      <span className="engine-text">
        <strong>{LABEL[health]}</strong>
        <small>{detail}</small>
      </span>
    </button>
  );
}
