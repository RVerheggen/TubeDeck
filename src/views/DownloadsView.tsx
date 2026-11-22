import { useState } from "react";
import { ListVideo, Pause, Play, Trash2 } from "lucide-react";
import { AddUrlPanel } from "../components/AddUrlPanel";
import { CommandPreview } from "../components/CommandPreview";
import { DownloadCard } from "../components/DownloadCard";
import { GroupCard } from "../components/GroupCard";
import { SetupPanel } from "../components/SetupPanel";
import { StatsGrid } from "../components/StatsGrid";
import { EmptyState, Panel } from "../components/ui";
import type { Job } from "../lib/types";
import { useAppStore } from "../store/useAppStore";

type QueueEntry = { job: Job } | { groupId: string; jobs: Job[] };

/** Playlist items are shown together under one header, at the position of their first item. */
function groupJobs(jobs: Job[]): QueueEntry[] {
  const groups = new Map<string, Job[]>();
  for (const j of jobs) if (j.group) groups.set(j.group.id, [...(groups.get(j.group.id) ?? []), j]);
  const out: QueueEntry[] = [];
  const seen = new Set<string>();
  for (const j of jobs) {
    if (!j.group) out.push({ job: j });
    else if (!seen.has(j.group.id)) {
      seen.add(j.group.id);
      out.push({ groupId: j.group.id, jobs: groups.get(j.group.id)! });
    }
  }
  return out;
}

export function DownloadsView() {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const toggleGroup = (id: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  const jobs = useAppStore((s) => s.jobs);
  const clearFinished = useAppStore((s) => s.clearFinished);
  const hasFinished = jobs.some((j) => ["completed", "failed", "canceled"].includes(j.status));
  const maxConcurrent = useAppStore((s) => s.settings!.maxConcurrent);
  const pauseAll = useAppStore((s) => s.pauseAll);
  const resumeAll = useAppStore((s) => s.resumeAll);
  const hasPaused = jobs.some((j) => j.status === "paused");
  const canPauseAll = jobs.some((j) => ["queued", "starting", "downloading", "processing"].includes(j.status));

  return (
    <>
      <SetupPanel />
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
              <button className="secondary-button" onClick={clearFinished} disabled={!hasFinished} title="Move finished downloads to History">
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
              {groupJobs(jobs).map((entry) =>
                "job" in entry ? (
                  <DownloadCard key={entry.job.id} job={entry.job} />
                ) : (
                  <GroupCard
                    key={entry.groupId}
                    jobs={entry.jobs}
                    collapsed={collapsed.has(entry.groupId)}
                    onToggle={() => toggleGroup(entry.groupId)}
                  />
                ),
              )}
            </>
          )}
        </Panel>
        <CommandPreview compact />
      </div>
    </>
  );
}
