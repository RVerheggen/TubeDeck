import { FileText, Film, FolderOpen, History, Music, RotateCw, Trash2 } from "lucide-react";
import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage, inDesktop, revealInExplorer } from "../lib/api";
import { formatBytes, formatTime, shortUrl } from "../lib/format";
import type { HistoryQuery, Job } from "../lib/types";
import { useAppStore } from "../store/useAppStore";
import { JobLog } from "./DownloadCard";
import { EmptyState, Panel } from "./ui";

const PAGE = 50;

export function HistoryTable() {
  const toast = useAppStore((s) => s.toast);
  const setView = useAppStore((s) => s.setView);
  // Refetch when a download in the queue finishes or leaves the queue.
  const finishedKey = useAppStore((s) =>
    s.jobs
      .filter((j) => ["completed", "failed", "canceled"].includes(j.status))
      .map((j) => j.id)
      .join(","),
  );
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<HistoryQuery["status"]>("all");
  const [items, setItems] = useState<Job[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [openLog, setOpenLog] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);
  const requestSeq = useRef(0);

  const load = useCallback(
    async (offset = 0) => {
      const seq = ++requestSeq.current;
      setLoading(true);
      try {
        const page = await api.listHistory({ search, status, limit: PAGE, offset });
        if (seq !== requestSeq.current) return; // a newer search superseded this one
        setItems((prev) => (offset === 0 ? page.items : [...prev, ...page.items]));
        setTotal(page.total);
      } catch (e) {
        toast("error", errorMessage(e));
      } finally {
        if (seq === requestSeq.current) setLoading(false);
      }
    },
    [search, status, toast],
  );

  useEffect(() => {
    const t = setTimeout(() => void load(0), 200);
    return () => clearTimeout(t);
  }, [load, finishedKey]);

  const redownload = async (job: Job) => {
    try {
      await api.redownload(job.id);
      toast("success", `Queued "${job.title ?? shortUrl(job.url)}" again`);
      setView("downloads");
    } catch (e) {
      toast("error", errorMessage(e));
    }
  };

  const remove = async (ids: string[]) => {
    try {
      if (ids.length === 0) await api.clearHistory();
      else await api.deleteHistory(ids);
      setConfirm(null);
      await load(0);
    } catch (e) {
      toast("error", errorMessage(e));
    }
  };

  return (
    <Panel
      title="Download history"
      icon={<History size={17} />}
      actions={
        <>
          <input className="compact" value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Search title or URL…" aria-label="Search history" />
          <select className="compact" value={status} onChange={(e) => setStatus(e.target.value as HistoryQuery["status"])} aria-label="Filter by status">
            <option value="all">All</option>
            <option value="completed">Completed</option>
            <option value="failed">Failed</option>
            <option value="canceled">Canceled</option>
          </select>
          {confirm === "all" ? (
            <button className="danger-button" onClick={() => remove([])} onBlur={() => setConfirm(null)} autoFocus>
              Delete all history?
            </button>
          ) : (
            <button className="secondary-button" onClick={() => setConfirm("all")} disabled={total === 0}>
              <Trash2 size={15} /> Clear history
            </button>
          )}
        </>
      }
    >
      {items.length === 0 && !loading ? (
        <EmptyState
          title={search || status !== "all" ? "Nothing matches" : "No finished downloads yet"}
          text={
            search || status !== "all"
              ? "Try a different search or filter."
              : "Completed, failed and canceled downloads are kept here, also after you clear them from the queue or restart TubeDeck."
          }
        />
      ) : (
        <>
          <p className="muted small">
            {total} download{total === 1 ? "" : "s"}
            {search || status !== "all" ? " match" : ""}
          </p>
          <table className="history-table">
            <thead>
              <tr>
                <th>Title</th>
                <th>Status</th>
                <th>Size</th>
                <th>Preset</th>
                <th>Finished</th>
                <th aria-label="Actions" />
              </tr>
            </thead>
            <tbody>
              {items.map((j) => (
                <Fragment key={j.id}>
                  <tr>
                    <td className="title-cell" title={j.url}>
                      <span className="history-icon" aria-hidden="true">
                        {j.thumbnail ? <img src={j.thumbnail} alt="" loading="lazy" referrerPolicy="no-referrer" /> : j.options.mode === "audio" ? <Music size={14} /> : <Film size={14} />}
                      </span>
                      <span className="title-text">
                        {j.title ?? shortUrl(j.url)}
                        {j.error && <small className="error-text">{j.error}</small>}
                      </span>
                    </td>
                    <td>
                      <span className={`status-pill ${j.status}`}>{j.status}</span>
                    </td>
                    <td>{j.status === "completed" ? formatBytes(j.progress.totalBytes) : "—"}</td>
                    <td>{j.presetName ?? "Custom"}</td>
                    <td>{formatTime(j.finishedAt ?? j.createdAt)}</td>
                    <td className="row-actions">
                      {j.status === "completed" && j.outputPath && inDesktop && (
                        <button className="icon-button" title="Show in Explorer" aria-label="Show in Explorer" onClick={() => revealInExplorer(j.outputPath!).catch(() => toast("error", "The file is no longer there."))}>
                          <FolderOpen size={14} />
                        </button>
                      )}
                      <button className="icon-button" title="Download again" aria-label="Download again" onClick={() => redownload(j)}>
                        <RotateCw size={14} />
                      </button>
                      <button className={`icon-button${openLog === j.id ? " active" : ""}`} title="Show log" aria-label="Show log" onClick={() => setOpenLog(openLog === j.id ? null : j.id)}>
                        <FileText size={14} />
                      </button>
                      {confirm === j.id ? (
                        <button className="danger-button" onClick={() => remove([j.id])} onBlur={() => setConfirm(null)} autoFocus>
                          Delete?
                        </button>
                      ) : (
                        <button className="icon-button" title="Delete from history" aria-label="Delete from history" onClick={() => setConfirm(j.id)}>
                          <Trash2 size={14} />
                        </button>
                      )}
                    </td>
                  </tr>
                  {openLog === j.id && (
                    <tr className="log-row">
                      <td colSpan={6}>
                        <JobLog job={j} />
                      </td>
                    </tr>
                  )}
                </Fragment>
              ))}
            </tbody>
          </table>
          {items.length < total && (
            <div className="load-more">
              <button className="secondary-button" onClick={() => load(items.length)} disabled={loading}>
                {loading ? "Loading…" : `Load more (${total - items.length} left)`}
              </button>
            </div>
          )}
        </>
      )}
    </Panel>
  );
}
