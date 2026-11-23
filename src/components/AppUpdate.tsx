import { ArrowUpCircle, ExternalLink, LoaderCircle, TriangleAlert, X } from "lucide-react";
import { useEffect, useState } from "react";
import { errorMessage, openExternal } from "../lib/api";
import { fetchChangelog } from "../lib/appUpdate";
import { changelogUrl, changesSince, releaseUrl, type ChangelogSection } from "../lib/changelog";
import { useAppStore } from "../store/useAppStore";
import { Markdown } from "./Markdown";

const RUNNING = ["starting", "downloading", "processing"];

/** Bottom-left sidebar notice shown when a newer TubeDeck release exists. */
export function UpdateNotice() {
  const update = useAppStore((s) => s.appUpdate);
  const phase = useAppStore((s) => s.appUpdatePhase);
  const progress = useAppStore((s) => s.appUpdateProgress);
  const open = useAppStore((s) => s.setAppUpdateDialog);
  if (!update) return null;
  const busy = phase === "downloading" || phase === "installing";
  return (
    <button className="update-notice" onClick={() => open(true)} title={`TubeDeck ${update.version} is available`}>
      {busy ? <LoaderCircle size={17} className="spin" /> : <ArrowUpCircle size={17} />}
      <span className="update-text-wrap">
        <strong>{busy ? (phase === "installing" ? "Installing update…" : `Downloading ${progress != null ? `${Math.round(progress)}%` : "…"}`) : "Update available"}</strong>
        <small>
          v{update.currentVersion} → v{update.version}
        </small>
      </span>
    </button>
  );
}

/** "2026-11-24" in the user's date format, like the release date above it. */
const localDate = (iso: string) => {
  const d = new Date(`${iso}T00:00:00`);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString();
};

type Changes = { phase: "loading" } | { phase: "ready"; sections: ChangelogSection[] } | { phase: "error"; message: string };

/** What changed between the installed version and the available one, from CHANGELOG.md. */
function WhatsNew({ installed, available }: { installed: string; available: string }) {
  const [changes, setChanges] = useState<Changes>({ phase: "loading" });

  useEffect(() => {
    let live = true;
    setChanges({ phase: "loading" });
    fetchChangelog(available)
      .then((md) => live && setChanges({ phase: "ready", sections: changesSince(md, installed, available) }))
      .catch((e) => live && setChanges({ phase: "error", message: errorMessage(e) }));
    return () => {
      live = false;
    };
  }, [installed, available]);

  const link = (url: string, label: string) => (
    <button className="link-button" onClick={() => void openExternal(url)}>
      {label} <ExternalLink size={12} />
    </button>
  );

  return (
    <div className="whats-new">
      {changes.phase === "loading" && (
        <p className="muted">
          <LoaderCircle size={14} className="spin" /> Loading what's new…
        </p>
      )}
      {changes.phase === "ready" &&
        changes.sections.map((s) => (
          <section key={s.version}>
            <h3>
              {s.version}
              {s.date && <small>{localDate(s.date)}</small>}
            </h3>
            <Markdown text={s.body} />
          </section>
        ))}
      {(changes.phase === "error" || (changes.phase === "ready" && changes.sections.length === 0)) && (
        <p className="muted" title={changes.phase === "error" ? changes.message : undefined}>
          Couldn't load the list of changes. {link(releaseUrl(available), "See the release on GitHub")}
        </p>
      )}
      <div className="whats-new-links">{link(changelogUrl(available), "Full changelog")}</div>
    </div>
  );
}

/** What's new + install button. */
export function AppUpdateDialog() {
  const openState = useAppStore((s) => s.appUpdateDialog);
  const update = useAppStore((s) => s.appUpdate);
  const phase = useAppStore((s) => s.appUpdatePhase);
  const progress = useAppStore((s) => s.appUpdateProgress);
  const error = useAppStore((s) => s.appUpdateError);
  const install = useAppStore((s) => s.installAppUpdate);
  const setOpen = useAppStore((s) => s.setAppUpdateDialog);
  const running = useAppStore((s) => s.jobs.filter((j) => RUNNING.includes(j.status)).length);
  if (!openState || !update) return null;
  const busy = phase === "downloading" || phase === "installing";

  return (
    <div className="modal-backdrop" role="presentation" onClick={() => !busy && setOpen(false)}>
      <div className="modal update-modal" role="dialog" aria-modal="true" aria-labelledby="upd-title" onClick={(e) => e.stopPropagation()}>
        <header className="picker-head">
          <div className="picker-title">
            <h2 id="upd-title">
              <ArrowUpCircle size={18} /> TubeDeck {update.version} is available
            </h2>
            <small>
              You have {update.currentVersion}
              {update.date ? ` · released ${new Date(update.date).toLocaleDateString()}` : ""}
            </small>
          </div>
          {!busy && (
            <button className="icon-button" title="Close" aria-label="Close" onClick={() => setOpen(false)}>
              <X size={15} />
            </button>
          )}
        </header>
        <div className="update-body">
          <WhatsNew installed={update.currentVersion} available={update.version} />
          {running > 0 && !busy && (
            <p className="notice warn">
              <TriangleAlert size={15} /> {running} download{running > 1 ? "s are" : " is"} running. {running > 1 ? "They're" : "It's"} saved and
              continue{running > 1 ? "" : "s"} from where {running > 1 ? "they" : "it"} stopped after the restart.
            </p>
          )}
          {busy && (
            <div className="install-status downloading">
              <div className={`progress-track${progress == null ? " indeterminate" : ""}`}>
                <div className="progress-fill" style={{ width: progress == null ? undefined : `${progress}%` }} />
              </div>
              <small>{phase === "installing" ? "Installing… TubeDeck restarts by itself." : `Downloading ${progress != null ? `${Math.round(progress)}%` : "…"}`}</small>
            </div>
          )}
          {phase === "error" && error && <p className="error-text">Update failed: {error}</p>}
        </div>
        <footer className="modal-actions picker-foot">
          <span className="muted small">The update is verified with TubeDeck's signing key before installing.</span>
          {!busy && (
            <>
              <button className="secondary-button" onClick={() => setOpen(false)}>Later</button>
              <button className="primary-button" onClick={install}>
                {phase === "error" ? "Try again" : "Update and restart"}
              </button>
            </>
          )}
        </footer>
      </div>
    </div>
  );
}
