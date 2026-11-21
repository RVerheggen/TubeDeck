import { Check, CircleAlert, Copy, Info, Terminal, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { api, copyText, errorMessage } from "../lib/api";
import type { BuiltCommand } from "../lib/types";
import { useAppStore, useOptions } from "../store/useAppStore";
import { Panel } from "./ui";

const DEBOUNCE_MS = 150;

const ICONS = {
  info: <Info size={14} />,
  warn: <TriangleAlert size={14} />,
  danger: <CircleAlert size={14} />,
};

/** Live yt-dlp command built by the Rust backend (the same builder the engine uses). */
export function CommandPreview({ compact = false }: { compact?: boolean }) {
  const options = useOptions();
  const draft = useAppStore((s) => s.urlDraft);
  const downloadFolder = useAppStore((s) => s.settings!.downloadFolder);
  const toast = useAppStore((s) => s.toast);
  const [cmd, setCmd] = useState<BuiltCommand | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const urls = draft.split(/\s+/).filter(Boolean);
  const firstUrl = urls[0] ?? "";

  useEffect(() => {
    let cancelled = false;
    const t = setTimeout(async () => {
      try {
        const built = await api.buildCommand(firstUrl, options, downloadFolder);
        if (!cancelled) {
          setCmd(built);
          setError(null);
        }
      } catch (e) {
        if (!cancelled) setError(errorMessage(e));
      }
    }, DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [firstUrl, options, downloadFolder]);

  const copy = async () => {
    if (!cmd) return;
    try {
      await copyText(cmd.display);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      toast("error", `Copy failed: ${errorMessage(e)}`);
    }
  };

  return (
    <Panel
      className="command-panel"
      title="Command preview"
      icon={<Terminal size={17} />}
      actions={
        <button className="secondary-button" onClick={copy} disabled={!cmd}>
          {copied ? <Check size={15} /> : <Copy size={15} />}
          {copied ? "Copied" : "Copy"}
        </button>
      }
    >
      <pre className={`command-code${compact ? " compact" : ""}`} aria-live="polite">
        {error ? `Error: ${error}` : (cmd?.display ?? "Building…")}
      </pre>
      <div className="command-meta">
        <span>PowerShell · {cmd ? `${cmd.args.length} arguments` : "—"}</span>
        {urls.length > 1 && <span>Showing the first of {urls.length} URLs</span>}
      </div>
      {cmd && cmd.warnings.length > 0 && (
        <ul className="warnings">
          {cmd.warnings.map((w, i) => (
            <li key={i} className={`warning ${w.level}`}>
              {ICONS[w.level]}
              <span>{w.message}</span>
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}
