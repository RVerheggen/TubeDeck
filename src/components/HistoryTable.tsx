import { History, Trash2 } from "lucide-react";
import { useState } from "react";
import { formatTime, shortUrl } from "../lib/format";
import type { JobStatus } from "../lib/types";
import { useAppStore } from "../store/useAppStore";
import { EmptyState, Panel } from "./ui";

const FINISHED: JobStatus[] = ["completed", "failed", "canceled"];

export function HistoryTable() {
  const jobs = useAppStore((s) => s.jobs);
  const clearFinished = useAppStore((s) => s.clearFinished);
  const [filter, setFilter] = useState<"all" | JobStatus>("all");
  const [query, setQuery] = useState("");

  const finished = jobs
    .filter((j) => FINISHED.includes(j.status))
    .filter((j) => filter === "all" || j.status === filter)
    .filter((j) => {
      const q = query.trim().toLowerCase();
      return !q || j.url.toLowerCase().includes(q) || (j.title ?? "").toLowerCase().includes(q);
    })
    .sort((a, b) => (b.finishedAt ?? b.createdAt) - (a.finishedAt ?? a.createdAt));

  return (
    <Panel
      title="Download history"
      icon={<History size={17} />}
      actions={
        <>
          <input className="compact" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search…" aria-label="Search history" />
          <select className="compact" value={filter} onChange={(e) => setFilter(e.target.value as typeof filter)} aria-label="Filter by status">
            <option value="all">All</option>
            <option value="completed">Completed</option>
            <option value="failed">Failed</option>
            <option value="canceled">Canceled</option>
          </select>
          <button className="secondary-button" onClick={clearFinished} disabled={finished.length === 0}>
            <Trash2 size={15} /> Clear
          </button>
        </>
      }
    >
      {finished.length === 0 ? (
        <EmptyState
          title="No finished downloads yet"
          text="Completed, failed and canceled downloads show up here. History is saved between sessions once the engine stores it."
        />
      ) : (
        <table className="history-table">
          <thead>
            <tr>
              <th>Title</th>
              <th>Status</th>
              <th>Preset</th>
              <th>Finished</th>
            </tr>
          </thead>
          <tbody>
            {finished.map((j) => (
              <tr key={j.id}>
                <td title={j.url}>{j.title ?? shortUrl(j.url)}</td>
                <td>
                  <span className={`status-pill ${j.status}`}>{j.status}</span>
                </td>
                <td>{j.presetName ?? "Custom"}</td>
                <td>{formatTime(j.finishedAt ?? j.createdAt)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Panel>
  );
}
