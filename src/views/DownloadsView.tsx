import { ListVideo, Pause, Play, Trash2, TriangleAlert } from "lucide-react";
import { AddUrlPanel } from "../components/AddUrlPanel";
import { CommandPreview } from "../components/CommandPreview";
import { DownloadCard } from "../components/DownloadCard";
import { StatsGrid } from "../components/StatsGrid";
import { EmptyState, Panel } from "../components/ui";
import { useAppStore } from "../store/useAppStore";

export function DownloadsView() {
  const jobs = useAppStore((s) => s.jobs);
  const clearFinished = useAppStore((s) => s.clearFinished);
  const hasFinished = jobs.some((j) => ["completed", "failed", "canceled"].includes(j.status));
  const engine = useAppStore((s) => s.engine);
  const maxConcurrent = useAppStore((s) => s.settings!.maxConcurrent);
  const setView = useAppStore((s) => s.setView);
  const pauseAll = useAppStore((s) => s.pauseAll);
  const resumeAll = useAppStore((s) => s.resumeAll);
  const hasPaused = jobs.some((j) => j.status === "paused");
  const canPauseAll = jobs.some((j) => ["queued", "starting", "downloading", "processing"].includes(j.status));

  return (
    <>
      {engine && !engine.ytdlp.ok && (
        <div className="notice warn">
          <TriangleAlert size={16} />
          <span>
            yt-dlp was not found, so downloads will fail. Install it or set its path in{" "}
            <button className="link-button" onClick={() => setView("settings")}>
              Settings → Engine
            </button>
            .
          </span>
        </div>
      )}
      <AddUrlPanel />
      <StatsGrid />
      <div className="two-col">
        <Panel
          title="Queue"
          icon={<ListVideo size={17} />}
          actions={
            <>
              <span className="muted small">Up to {maxConcurrent} at a time</span>
              {canPauseAll && (
                <button className="secondary-button" onClick={pauseAll} title="Pause all queued and running downloads">
                  <Pause size={15} /> Pause all
                </button>
              )}
              {hasPaused && (
                <button className="secondary-button" onClick={resumeAll}>
                  <Play size={15} /> Resume all
                </button>
              )}
              <button className="secondary-button" onClick={clearFinished} disabled={!hasFinished}>
                <Trash2 size={15} /> Clear finished
              </button>
            </>
          }
        >
          {jobs.length === 0 ? (
            <EmptyState
              title="Your queue is empty"
              text="Paste a video or playlist URL above and press Add to queue (Ctrl+Enter)."
            />
          ) : (
            <>
              {hasPaused || jobs.filter((j) => j.status === "queued").length > 1 ? (
                <p className="muted small queue-hint">Drag queued or paused items to change which one starts next.</p>
              ) : null}
              {jobs.map((j) => (
                <DownloadCard key={j.id} job={j} />
              ))}
            </>
          )}
        </Panel>
        <CommandPreview compact />
      </div>
    </>
  );
}
