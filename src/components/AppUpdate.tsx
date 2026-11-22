import { ArrowUpCircle, LoaderCircle, TriangleAlert, X } from "lucide-react";
import { useAppStore } from "../store/useAppStore";

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

/** Release notes + install button. */
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
          {update.notes ? <pre className="release-notes">{update.notes}</pre> : <p className="muted">No release notes.</p>}
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
