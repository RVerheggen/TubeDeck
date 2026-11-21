import { FileText, Film, FolderOpen, Music, RotateCcw, Square, X } from "lucide-react";
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
  completed: "Completed",
  failed: "Failed",
  canceled: "Canceled",
};

const RUNNING: JobStatus[] = ["starting", "downloading", "processing"];

function JobLog({ job }: { job: Job }) {
  const [lines, setLines] = useState<string[]>([]);
  const box = useRef<HTMLDivElement>(null);
  const running = RUNNING.includes(job.status);

  useEffect(() => {
    let alive = true;
    const load = () =>
      api
        .getJobLog(job.id)
        .then((l) => alive && setLines(l))
        .catch(() => {});
    void load();
    // Refresh while the job runs; a finished job's log no longer changes.
    const timer = running ? setInterval(load, 1000) : undefined;
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [job.id, running]);

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
          <div key={i} className={`log-line${l.startsWith("ERROR") ? " error" : l.startsWith("WARNING") ? " warn" : l.startsWith("[TubeDeck]") ? " debug" : ""}`}>
            {l}
          </div>
        ))
      )}
    </div>
  );
}

export function DownloadCard({ job }: { job: Job }) {
  const cancelJob = useAppStore((s) => s.cancelJob);
  const retryJob = useAppStore((s) => s.retryJob);
  const removeJob = useAppStore((s) => s.removeJob);
  const toast = useAppStore((s) => s.toast);
  const [showLog, setShowLog] = useState(false);
  const [thumbFailed, setThumbFailed] = useState(false);

  const { progress: p } = job;
  const running = RUNNING.includes(job.status);
  const indeterminate = running && p.percent == null;
  const percent = job.status === "completed" ? 100 : (p.percent ?? 0);
  const canRetry = job.status === "failed" || job.status === "canceled";

  let detail: string;
  if (job.error) detail = job.error;
  else if (job.status === "queued") detail = "Waiting for a free download slot";
  else if (job.status === "canceled") detail = "Canceled";
  else if (job.status === "completed") detail = job.outputPath ?? "Done";
  else if (p.totalBytes != null || p.downloadedBytes != null)
    detail = `${p.percent != null ? `${p.percent.toFixed(1)}% · ` : ""}${formatBytes(p.downloadedBytes)} of ${formatBytes(p.totalBytes)}`;
  else detail = job.stage ?? "Starting…";

  return (
    <article className={`download-card status-${job.status}`}>
      <div className="download-row">
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
              {job.stage && running && <span className="stage">{job.stage}</span>}
              <span className={`status-pill ${job.status}`}>{STATUS_LABEL[job.status]}</span>
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
            <span className={job.error ? "error-text" : "detail"} title={detail}>
              {detail}
            </span>
            <span>
              {job.status === "downloading" ? `${formatSpeed(p.speed)} · ETA ${formatEta(p.eta)} · ` : ""}
              {job.presetName ?? "Custom settings"}
            </span>
          </div>
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
          {running || job.status === "queued" ? (
            <button className="icon-button danger" title="Cancel" aria-label="Cancel" onClick={() => cancelJob(job.id)}>
              <Square size={13} />
            </button>
          ) : null}
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
