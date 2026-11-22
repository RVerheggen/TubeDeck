import { FileDown, FolderOpen, RefreshCw, ScrollText } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage, exportDiagnostics, inDesktop, revealInExplorer } from "../lib/api";
import type { AppLog } from "../lib/types";
import { useAppStore } from "../store/useAppStore";
import { Panel } from "./ui";

function levelOf(line: string): string {
  if (line.includes("[ERROR]")) return "error";
  if (line.includes("[WARN]")) return "warn";
  if (line.includes("[DEBUG]") || line.includes("[TRACE]")) return "debug";
  return "info";
}

export function ExportDiagnosticsButton() {
  const toast = useAppStore((s) => s.toast);
  const [busy, setBusy] = useState(false);
  const run = async () => {
    setBusy(true);
    try {
      const path = await exportDiagnostics();
      if (path) toast("success", `Diagnostics saved to ${path}`);
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <button className="secondary-button" onClick={run} disabled={busy} title="Save a text report (versions, settings, recent logs) for a bug report">
      <FileDown size={15} /> Export diagnostics
    </button>
  );
}

export function LogViewer() {
  const toast = useAppStore((s) => s.toast);
  const [log, setLog] = useState<AppLog | null>(null);
  const [filter, setFilter] = useState("");
  const [loading, setLoading] = useState(false);
  const bottom = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setLog(await api.readAppLog(1000));
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [log]);

  const q = filter.trim().toLowerCase();
  const lines = (log?.lines ?? []).filter((l) => !q || l.toLowerCase().includes(q));

  return (
    <Panel
      title="Application log"
      icon={<ScrollText size={17} />}
      actions={
        <>
          <input className="compact" value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Filter…" aria-label="Filter log" />
          <button className="secondary-button" onClick={load} disabled={loading}>
            <RefreshCw size={15} className={loading ? "spin" : ""} /> Refresh
          </button>
          {inDesktop && log?.path && (
            <button className="icon-button" title="Show log file in Explorer" onClick={() => revealInExplorer(log.path).catch((e) => toast("error", errorMessage(e)))}>
              <FolderOpen size={15} />
            </button>
          )}
          {inDesktop && <ExportDiagnosticsButton />}
        </>
      }
    >
      <p className="muted">
        {log?.path ? <code>{log.path}</code> : "Loading…"} · Each download's own log is under the log button on its card or History row.
      </p>
      <div className="log-box" role="log">
        {lines.length === 0 ? (
          <div className="log-line muted">{log ? "No log lines" : ""}</div>
        ) : (
          lines.map((l, i) => (
            <div key={i} className={`log-line ${levelOf(l)}`}>
              {l}
            </div>
          ))
        )}
        <div ref={bottom} />
      </div>
    </Panel>
  );
}
