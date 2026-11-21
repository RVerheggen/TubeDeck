import { Film, Music, X } from "lucide-react";
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

export function DownloadCard({ job }: { job: Job }) {
  const removeJob = useAppStore((s) => s.removeJob);
  const { progress: p } = job;
  const running = job.status === "downloading" || job.status === "starting" || job.status === "processing";
  const indeterminate = running && p.percent == null;
  const percent = job.status === "completed" ? 100 : (p.percent ?? 0);

  return (
    <article className={`download-card status-${job.status}`}>
      <div className="file-icon" aria-hidden="true">
        {job.options.mode === "audio" ? <Music size={18} /> : <Film size={18} />}
      </div>
      <div className="download-info">
        <div className="download-heading">
          <strong title={job.url}>{job.title ?? shortUrl(job.url)}</strong>
          <span className={`status-pill ${job.status}`}>{STATUS_LABEL[job.status]}</span>
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
          {job.error ? (
            <span className="error-text">{job.error}</span>
          ) : job.status === "queued" ? (
            <span>Waiting for the download engine</span>
          ) : (
            <span>
              {p.percent != null ? `${p.percent.toFixed(1)}%` : "—"} · {formatBytes(p.downloadedBytes)} of{" "}
              {formatBytes(p.totalBytes)}
            </span>
          )}
          <span>
            {running ? `${formatSpeed(p.speed)} · ETA ${formatEta(p.eta)} · ` : ""}
            {job.presetName ?? "Custom settings"}
          </span>
        </div>
      </div>
      <div className="card-actions">
        <button className="icon-button" onClick={() => removeJob(job.id)} title="Remove from queue" aria-label="Remove">
          <X size={15} />
        </button>
      </div>
    </article>
  );
}
