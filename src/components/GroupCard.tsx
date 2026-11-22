import { ChevronDown, ChevronRight, ListVideo, Pause, Play, Square, Trash2 } from "lucide-react";
import { useState } from "react";
import type { Job } from "../lib/types";
import { useAppStore } from "../store/useAppStore";
import { DownloadCard } from "./DownloadCard";

const RUNNING = ["starting", "downloading", "processing"];
const FINISHED = ["completed", "failed", "canceled"];

/** A playlist added through the picker: summary header plus its items. */
export function GroupCard({ jobs, collapsed, onToggle }: { jobs: Job[]; collapsed: boolean; onToggle: () => void }) {
  const { pauseJob, resumeJob, cancelJob, removeJob } = useAppStore.getState();
  const [confirmCancel, setConfirmCancel] = useState(false);
  const group = jobs[0].group!;

  const done = jobs.filter((j) => j.status === "completed").length;
  const failed = jobs.filter((j) => j.status === "failed").length;
  const running = jobs.filter((j) => RUNNING.includes(j.status)).length;
  const paused = jobs.filter((j) => j.status === "paused");
  const active = jobs.filter((j) => j.status === "queued" || RUNNING.includes(j.status));
  const finished = jobs.filter((j) => FINISHED.includes(j.status));
  const percent =
    jobs.reduce((sum, j) => sum + (j.status === "completed" ? 100 : FINISHED.includes(j.status) ? 0 : (j.progress.percent ?? 0)), 0) /
    jobs.length;

  const parts = [`${done}/${jobs.length} done`];
  if (running) parts.push(`${running} downloading`);
  if (paused.length) parts.push(`${paused.length} paused`);
  if (failed) parts.push(`${failed} failed`);

  const all = (ids: Job[], fn: (id: string) => Promise<void>) => Promise.all(ids.map((j) => fn(j.id)));

  return (
    <section className={`group-card${collapsed ? " collapsed" : ""}`} aria-label={`Playlist ${group.title}`}>
      <header className="group-head">
        <button className="icon-button ghost" onClick={onToggle} aria-expanded={!collapsed} title={collapsed ? "Show items" : "Hide items"}>
          {collapsed ? <ChevronRight size={16} /> : <ChevronDown size={16} />}
        </button>
        <div className="group-icon" aria-hidden="true">
          <ListVideo size={18} />
        </div>
        <div className="group-info">
          <div className="download-heading">
            <strong title={group.title}>{group.title}</strong>
            <span className="muted small">
              {jobs.length} of {group.count} item{group.count === 1 ? "" : "s"}
            </span>
          </div>
          <div className="progress-track" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(percent)}>
            <div className="progress-fill" style={{ width: `${percent}%` }} />
          </div>
          <div className="download-meta">
            <span>{parts.join(" · ")}</span>
            <span>{percent.toFixed(0)}%</span>
          </div>
        </div>
        <div className="card-actions">
          {active.length > 0 && (
            <button className="icon-button" title="Pause all items" aria-label="Pause all items" onClick={() => all(active, pauseJob)}>
              <Pause size={15} />
            </button>
          )}
          {paused.length > 0 && (
            <button className="icon-button accent" title="Resume all items" aria-label="Resume all items" onClick={() => all(paused, resumeJob)}>
              <Play size={15} />
            </button>
          )}
          {active.length + paused.length > 0 &&
            (confirmCancel ? (
              <button
                className="danger-button"
                onClick={() => all([...active, ...paused], cancelJob).then(() => setConfirmCancel(false))}
                onBlur={() => setConfirmCancel(false)}
                autoFocus
              >
                Cancel {active.length + paused.length}?
              </button>
            ) : (
              <button className="icon-button danger" title="Cancel remaining items" aria-label="Cancel remaining items" onClick={() => setConfirmCancel(true)}>
                <Square size={13} />
              </button>
            ))}
          {finished.length > 0 && active.length + paused.length === 0 && (
            <button className="icon-button" title="Remove playlist from queue (stays in History)" aria-label="Remove playlist" onClick={() => all(finished, removeJob)}>
              <Trash2 size={15} />
            </button>
          )}
        </div>
      </header>
      {!collapsed && (
        <div className="group-items">
          {jobs.map((j) => (
            <DownloadCard key={j.id} job={j} />
          ))}
        </div>
      )}
    </section>
  );
}
