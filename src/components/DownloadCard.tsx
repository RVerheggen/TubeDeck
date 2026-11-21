import { FileText, Film, FolderOpen, GripVertical, Music, Pause, Play, RotateCcw, Square, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api, errorMessage, inDesktop, revealInExplorer } from "../lib/api";
import { formatBytes, formatEta, formatSpeed, shortUrl } from "../lib/format";
import type { Job, JobStatus } from "../lib/types";
import { useAppStore } from "../store/useAppStore";

const STATUS_LABEL: Record<JobStatus, string> = {
  queued: "Queued",
  starting: "Starting",
  downloading: "Downloading",
  processing: "Processing",
  paused: "Paused",
  completed: "Completed",
  failed: "Failed",
  canceled: "Canceled",
};

const RUNNING: JobStatus[] = ["starting", "downloading", "processing"];
export const DRAG_TYPE = "application/x-tubedeck-job";

/** Seconds until `at` (epoch ms), re-rendering every second while in the future. */
function useCountdown(at: number | null): number | null {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (at == null) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [at]);
  return at == null ? null : Math.max(0, Math.ceil((at - now) / 1000));
}

function JobLog({ job }: { job: Job }) {
  const [lines, setLines] = useState<string[]>([]);
  const box = useRef<HTMLDivElement>(null);
  const live = RUNNING.includes(job.status) || job.retryAt != null;

  useEffect(() => {
    let alive = true;
    const load = () =>
      api
        .getJobLog(job.id)
        .then((l) => alive && setLines(l))
        .catch(() => {});
    void load();
    // Refresh while the job runs; a finished job's log no longer changes.
    const timer = live ? setInterval(load, 1000) : undefined;
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [job.id, live, job.status]);

  useEffect(() => {
    const el = box.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);

  return (
    <div className="job-log" ref={box} role="log">
      {lines.length === 0 ? (
        <div className="log-line muted">No output yet</div>
      ) : (
        lines.map((l, i) => (
          <div
            key={i}
            className={`log-line${l.startsWith("ERROR") ? " error" : l.startsWith("WARNING") ? " warn" : l.startsWith("[TubeDeck]") ? " debug" : ""}`}
          >
            {l}
          </div>
        ))
      )}
    </div>
  );
}

export function DownloadCard({ job }: { job: Job }) {
  const { cancelJob, retryJob, removeJob, pauseJob, resumeJob, moveJob, toast } = useAppStore.getState();
  const [showLog, setShowLog] = useState(false);
  const [thumbFailed, setThumbFailed] = useState(false);
  const [dropTarget, setDropTarget] = useState(false);
  const retryIn = useCountdown(job.status === "queued" ? job.retryAt : null);

  const { progress: p } = job;
  const running = RUNNING.includes(job.status);
  const waiting = job.status === "queued" || job.status === "paused";
  const indeterminate = running && p.percent == null;
  const percent = job.status === "completed" ? 100 : (p.percent ?? 0);
  const canRetry = job.status === "failed" || job.status === "canceled";
  const pillLabel = retryIn != null ? "Retrying" : STATUS_LABEL[job.status];

  let detail: string;
  if (retryIn != null) detail = `Retrying in ${retryIn}s${job.error ? ` · ${job.error}` : ""}`;
  else if (job.error && !running && job.status !== "paused") detail = job.error;
  else if (job.status === "queued") detail = "Waiting for a free download slot";
  else if (job.status === "paused")
    detail = p.percent != null ? `Paused at ${p.percent.toFixed(1)}% · resumes where it stopped` : "Paused";
  else if (job.status === "canceled") detail = "Canceled";
  else if (job.status === "completed") detail = job.outputPath ?? "Done";
  else if (p.totalBytes != null || p.downloadedBytes != null)
    detail = `${p.percent != null ? `${p.percent.toFixed(1)}% · ` : ""}${formatBytes(p.downloadedBytes)} of ${formatBytes(p.totalBytes)}`;
  else detail = job.stage ?? "Starting…";

  const isError = job.status === "failed" || retryIn != null;

  return (
    <article
      className={`download-card status-${job.status}${dropTarget ? " drop-target" : ""}${retryIn != null ? " retrying" : ""}`}
      draggable={waiting}
      onDragStart={(e) => {
        e.dataTransfer.setData(DRAG_TYPE, job.id);
        e.dataTransfer.effectAllowed = "move";
      }}
      onDragOver={(e) => {
        if (e.dataTransfer.types.includes(DRAG_TYPE)) {
          e.preventDefault();
          setDropTarget(true);
        }
      }}
      onDragLeave={() => setDropTarget(false)}
      onDrop={(e) => {
        setDropTarget(false);
        const id = e.dataTransfer.getData(DRAG_TYPE);
        if (id && id !== job.id) {
          e.preventDefault();
          void moveJob(id, job.id);
        }
      }}
    >
      <div className="download-row">
        <span className={`drag-handle${waiting ? "" : " hidden"}`} title="Drag to reorder" aria-hidden="true">
          <GripVertical size={14} />
        </span>
        <div className="file-icon" aria-hidden="true">
          {job.thumbnail && !thumbFailed ? (
            <img src={job.thumbnail} alt="" loading="lazy" referrerPolicy="no-referrer" onError={() => setThumbFailed(true)} />
          ) : job.options.mode === "audio" ? (
            <Music size={18} />
          ) : (
            <Film size={18} />
          )}
        </div>
        <div className="download-info">
          <div className="download-heading">
            <strong title={job.url}>{job.title ?? shortUrl(job.url)}</strong>
            <span className="heading-right">
              {job.stage && (running || job.retryAt != null) && <span className="stage">{job.stage}</span>}
              <span className={`status-pill ${retryIn != null ? "retrying" : job.status}`}>{pillLabel}</span>
            </span>
          </div>
          <div
            className={`progress-track${indeterminate ? " indeterminate" : ""}`}
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={indeterminate ? undefined : Math.round(percent)}
          >
            <div className="progress-fill" style={{ width: indeterminate ? undefined : `${percent}%` }} />
          </div>
          <div className="download-meta">
            <span className={isError ? "error-text" : "detail"} title={detail}>
              {detail}
            </span>
            <span>
              {job.status === "downloading" ? `${formatSpeed(p.speed)} · ETA ${formatEta(p.eta)} · ` : ""}
              {job.presetName ?? "Custom settings"}
            </span>
          </div>
          {job.errorHint && (job.status === "failed" || retryIn != null) && <div className="error-hint">{job.errorHint}</div>}
        </div>
        <div className="card-actions">
          {job.status === "completed" && job.outputPath && inDesktop && (
            <button
              className="icon-button"
              title="Show in Explorer"
              aria-label="Show in Explorer"
              onClick={() => revealInExplorer(job.outputPath!).catch((e) => toast("error", errorMessage(e)))}
            >
              <FolderOpen size={15} />
            </button>
          )}
          {(running || job.status === "queued") && (
            <button className="icon-button" title="Pause" aria-label="Pause" onClick={() => pauseJob(job.id)}>
              <Pause size={15} />
            </button>
          )}
          {job.status === "paused" && (
            <button className="icon-button accent" title="Resume" aria-label="Resume" onClick={() => resumeJob(job.id)}>
              <Play size={15} />
            </button>
          )}
          {canRetry && (
            <button className="icon-button" title="Retry" aria-label="Retry" onClick={() => retryJob(job.id)}>
              <RotateCcw size={15} />
            </button>
          )}
          <button
            className={`icon-button${showLog ? " active" : ""}`}
            title="Show log"
            aria-label="Show log"
            aria-expanded={showLog}
            onClick={() => setShowLog((v) => !v)}
          >
            <FileText size={15} />
          </button>
          {(running || waiting) && (
            <button className="icon-button danger" title="Cancel" aria-label="Cancel" onClick={() => cancelJob(job.id)}>
              <Square size={13} />
            </button>
          )}
          {!running && (
            <button className="icon-button" title="Remove from list" aria-label="Remove" onClick={() => removeJob(job.id)}>
              <X size={15} />
            </button>
          )}
        </div>
      </div>
      {showLog && <JobLog job={job} />}
    </article>
  );
}
