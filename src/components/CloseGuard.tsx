import { TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { inDesktop } from "../lib/api";
import { useAppStore } from "../store/useAppStore";

const RUNNING = ["starting", "downloading", "processing"];

/** Asks before closing the window while downloads are running. */
export function CloseGuard() {
  const [running, setRunning] = useState(0);
  const resumeOnStartup = useAppStore((s) => s.settings?.resumeOnStartup ?? true);

  useEffect(() => {
    if (!inDesktop) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
      const off = await getCurrentWindow().onCloseRequested((e) => {
        const n = useAppStore.getState().jobs.filter((j) => RUNNING.includes(j.status)).length;
        if (n > 0) {
          e.preventDefault();
          setRunning(n);
        }
      });
      if (cancelled) off();
      else unlisten = off;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  if (running === 0) return null;

  const quit = async () => {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().destroy();
  };

  return (
    <div className="modal-backdrop" role="presentation" onClick={() => setRunning(0)}>
      <div className="modal" role="alertdialog" aria-modal="true" aria-labelledby="close-title" onClick={(e) => e.stopPropagation()}>
        <h2 id="close-title">
          <TriangleAlert size={18} /> {running} download{running > 1 ? "s are" : " is"} still running
        </h2>
        <p>
          {resumeOnStartup
            ? "If you quit now, they're paused and continue automatically the next time you open TubeDeck."
            : "If you quit now, they're paused. You can resume them the next time you open TubeDeck."}{" "}
          Partial files are kept.
        </p>
        <div className="modal-actions">
          <button className="secondary-button" onClick={() => setRunning(0)} autoFocus>
            Keep downloading
          </button>
          <button className="primary-button" onClick={quit}>
            Quit TubeDeck
          </button>
        </div>
      </div>
    </div>
  );
}
