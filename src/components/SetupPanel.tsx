import { CircleCheckBig, Download, LoaderCircle, PackageCheck, RefreshCw, TriangleAlert, X } from "lucide-react";
import { useState } from "react";
import { formatBytes } from "../lib/format";
import type { BinaryStatus, Component, InstallProgress } from "../lib/types";
import { useAppStore } from "../store/useAppStore";

export const COMPONENTS: { id: Component; name: string; size: number; why: string; required: boolean }[] = [
  { id: "ytdlp", name: "yt-dlp", size: 23_600_000, why: "Does the actual downloading.", required: true },
  { id: "ffmpeg", name: "FFmpeg", size: 91_000_000, why: "Merges video and audio, converts audio, embeds thumbnails.", required: false },
  { id: "deno", name: "Deno", size: 42_900_000, why: "Solves YouTube's JavaScript challenges for full format support.", required: false },
];

export function statusOf(engine: ReturnType<typeof useAppStore.getState>["engine"], id: Component): BinaryStatus | undefined {
  return engine?.[id];
}

const busy = (p?: InstallProgress) => !!p && p.phase !== "done" && p.phase !== "error";

/** Progress bar + label for a running install. */
export function InstallStatus({ progress }: { progress: InstallProgress }) {
  const pct = progress.total ? (progress.downloaded / progress.total) * 100 : null;
  const label =
    progress.phase === "downloading"
      ? `Downloading ${formatBytes(progress.downloaded)}${progress.total ? ` of ${formatBytes(progress.total)}` : ""}`
      : progress.phase === "verifying"
        ? "Verifying checksum…"
        : progress.phase === "installing"
          ? "Installing…"
          : progress.phase === "done"
            ? "Installed"
            : (progress.message ?? "Failed");
  return (
    <div className={`install-status ${progress.phase}`}>
      {busy(progress) && (
        <div className={`progress-track${pct == null ? " indeterminate" : ""}`}>
          <div className="progress-fill" style={{ width: pct == null ? undefined : `${progress.phase === "downloading" ? pct : 100}%` }} />
        </div>
      )}
      <small>{label}</small>
    </div>
  );
}

/** Install / Update / Reinstall button for one component. */
export function InstallButton({ id, label, primary }: { id: Component; label: string; primary?: boolean }) {
  const progress = useAppStore((s) => s.installs[id]);
  const install = useAppStore((s) => s.install);
  return (
    <button className={primary ? "primary-button" : "secondary-button"} onClick={() => install(id)} disabled={busy(progress)}>
      {busy(progress) ? <LoaderCircle size={14} className="spin" /> : label.startsWith("Update") ? <RefreshCw size={14} /> : <Download size={14} />}
      {label}
    </button>
  );
}

/** First-run / missing-binary setup shown on the Downloads view. */
export function SetupPanel() {
  const engine = useAppStore((s) => s.engine);
  const installs = useAppStore((s) => s.installs);
  const install = useAppStore((s) => s.install);
  const [dismissed, setDismissed] = useState(() => {
    try {
      return localStorage.getItem("tubedeck.setupDismissed") === "1";
    } catch {
      return false;
    }
  });
  if (!engine) return null;

  const missing = COMPONENTS.filter((c) => !engine[c.id].ok);
  const anyBusy = COMPONENTS.some((c) => busy(installs[c.id]));
  const ytdlpMissing = !engine.ytdlp.ok;
  // Optional components can be dismissed; a missing yt-dlp can't.
  if ((missing.length === 0 && !anyBusy) || (!ytdlpMissing && dismissed && !anyBusy)) return null;

  const todo = missing.filter((c) => !busy(installs[c.id]));
  const totalSize = todo.reduce((s, c) => s + c.size, 0);
  const installAll = () => todo.forEach((c) => void install(c.id));

  return (
    <section className={`panel setup-panel${ytdlpMissing ? " required" : ""}`} aria-label="Engine setup">
      <header className="panel-head">
        <h2>
          {ytdlpMissing ? <TriangleAlert size={17} /> : <PackageCheck size={17} />}
          {ytdlpMissing ? "Set up the download engine" : "Recommended components"}
        </h2>
        <div className="panel-actions">
          {todo.length > 0 && (
            <button className="primary-button" onClick={installAll}>
              <Download size={15} /> Install {todo.length === COMPONENTS.length ? "everything" : todo.length === 1 ? todo[0].name : "missing"} ({formatBytes(totalSize)})
            </button>
          )}
          {!ytdlpMissing && (
            <button
              className="icon-button ghost"
              title="Hide"
              aria-label="Hide"
              onClick={() => {
                setDismissed(true);
                try {
                  localStorage.setItem("tubedeck.setupDismissed", "1");
                } catch {
                  /* storage unavailable: hide for this session only */
                }
              }}
            >
              <X size={15} />
            </button>
          )}
        </div>
      </header>
      <p>
        {ytdlpMissing
          ? "TubeDeck needs yt-dlp to download anything. Install it with one click. Files come from the official GitHub releases and are checked against their published SHA-256 checksums."
          : "Everything works, but these make downloads more reliable. Files come from the official GitHub releases and are checksum-verified."}
      </p>
      <div className="setup-grid">
        {COMPONENTS.map((c) => {
          const st = engine[c.id];
          const p = installs[c.id];
          return (
            <div key={c.id} className={`setup-item${st.ok ? " ok" : ""}`}>
              <div className="setup-item-head">
                {st.ok ? <CircleCheckBig size={16} className="ok-icon" /> : <span className={`status-dot ${c.required ? "missing" : "degraded"}`} />}
                <strong>{c.name}</strong>
                <span className="muted small">{c.required ? "required" : "recommended"}</span>
              </div>
              <small className="muted">{c.why}</small>
              {p && busy(p) ? (
                <InstallStatus progress={p} />
              ) : p?.phase === "error" && !st.ok ? (
                <>
                  <InstallStatus progress={p} />
                  <InstallButton id={c.id} label="Try again" />
                </>
              ) : st.ok ? (
                <small className="ok-text">v{st.version} · {st.source}</small>
              ) : (
                <InstallButton id={c.id} label={`Install (${formatBytes(c.size)})`} />
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
