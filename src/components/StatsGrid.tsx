import { CircleCheckBig, Clock, Gauge, LoaderCircle } from "lucide-react";
import { formatSpeed } from "../lib/format";
import { useAppStore } from "../store/useAppStore";

export function StatsGrid() {
  const jobs = useAppStore((s) => s.jobs);
  const active = jobs.filter((j) => ["starting", "downloading", "processing"].includes(j.status));
  const queued = jobs.filter((j) => j.status === "queued" || j.status === "paused").length;
  const done = jobs.filter((j) => j.status === "completed").length;
  const speed = active.reduce((sum, j) => sum + (j.progress.speed ?? 0), 0);

  const stats = [
    { label: "Active", value: active.length, icon: <LoaderCircle size={18} /> },
    { label: "Queued", value: queued, icon: <Clock size={18} /> },
    { label: "Completed", value: done, icon: <CircleCheckBig size={18} /> },
    { label: "Total speed", value: formatSpeed(speed), icon: <Gauge size={18} /> },
  ];

  return (
    <div className="stats-grid">
      {stats.map((s) => (
        <div className="stat-card" key={s.label}>
          <div className="stat-icon">{s.icon}</div>
          <div>
            <small>{s.label}</small>
            <strong>{s.value}</strong>
          </div>
        </div>
      ))}
    </div>
  );
}
