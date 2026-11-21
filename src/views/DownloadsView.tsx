import { ListVideo, Trash2 } from "lucide-react";
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

  return (
    <>
      <AddUrlPanel />
      <StatsGrid />
      <div className="two-col">
        <Panel
          title="Queue"
          icon={<ListVideo size={17} />}
          actions={
            <button className="secondary-button" onClick={clearFinished} disabled={!hasFinished}>
              <Trash2 size={15} /> Clear finished
            </button>
          }
        >
          {jobs.length === 0 ? (
            <EmptyState
              title="Your queue is empty"
              text="Paste a video or playlist URL above and press Add to queue (Ctrl+Enter)."
            />
          ) : (
            <>
              <div className="notice">
                The download engine connects in the next milestone (M2). Until then, added items stay queued.
              </div>
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
