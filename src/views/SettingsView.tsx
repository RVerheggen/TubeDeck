import { Cpu, FolderOpen, Monitor, Moon, RefreshCw, Settings as SettingsIcon, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { api, inDesktop, pickFolder, revealInExplorer } from "../lib/api";
import type { AppPaths, BinaryStatus, Settings } from "../lib/types";
import { Field, Panel, Segmented } from "../components/ui";
import { useAppStore } from "../store/useAppStore";

function BinaryRow({
  status,
  value,
  onChange,
  hint,
}: {
  status: BinaryStatus | undefined;
  value: string;
  onChange: (v: string) => void;
  hint: string;
}) {
  const state = !status ? "unknown" : status.ok ? "ok" : status.found ? "degraded" : "missing";
  return (
    <div className="binary-row">
      <div className="binary-head">
        <span className={`status-dot ${state}`} />
        <strong>{status?.name ?? "…"}</strong>
        <span className="muted">
          {status?.version ? `v${status.version}` : (status?.note ?? "Checking…")}
          {status?.source ? ` · ${status.source}` : ""}
        </span>
      </div>
      {status?.path && <code className="binary-path">{status.path}</code>}
      {status?.ok === false && status.found && status.note && <small className="error-text">{status.note}</small>}
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Auto-detect (managed folder, then PATH)"
        spellCheck={false}
        aria-label={`${status?.name ?? "Binary"} path`}
      />
      <small className="field-hint">{hint}</small>
    </div>
  );
}

export function SettingsView() {
  const settings = useAppStore((s) => s.settings!);
  const update = useAppStore((s) => s.updateSettings);
  const engine = useAppStore((s) => s.engine);
  const loading = useAppStore((s) => s.engineLoading);
  const refreshEngine = useAppStore((s) => s.refreshEngine);
  const toast = useAppStore((s) => s.toast);
  const [paths, setPaths] = useState<AppPaths | null>(null);

  useEffect(() => {
    void api.appPaths().then(setPaths);
  }, []);

  const browse = async () => {
    const folder = await pickFolder(settings.downloadFolder);
    if (folder) update({ downloadFolder: folder });
  };

  const setPath = (key: keyof Pick<Settings, "ytdlpPath" | "ffmpegPath" | "denoPath">) => (v: string) => update({ [key]: v });

  return (
    <>
      <Panel title="General" icon={<SettingsIcon size={17} />}>
        <div className="config-grid">
          <Field label="Theme">
            <Segmented
              label="Theme"
              value={settings.theme}
              onChange={(theme) => update({ theme })}
              options={[
                { value: "system", label: "System", icon: <Monitor size={15} /> },
                { value: "dark", label: "Dark", icon: <Moon size={15} /> },
                { value: "light", label: "Light", icon: <Sun size={15} /> },
              ]}
            />
          </Field>
          <Field label={`Simultaneous downloads: ${settings.maxConcurrent}`} hint="Used by the download engine (M2/M3).">
            <input
              type="range"
              min={1}
              max={10}
              value={settings.maxConcurrent}
              onChange={(e) => update({ maxConcurrent: Number(e.target.value) })}
            />
          </Field>
          <Field label="Default download folder" wide>
            <div className="input-with-button">
              <input
                value={settings.downloadFolder}
                onChange={(e) => update({ downloadFolder: e.target.value })}
                placeholder={paths?.defaultDownloadFolder}
                spellCheck={false}
              />
              <button type="button" className="secondary-button" onClick={browse}>
                <FolderOpen size={15} /> Browse
              </button>
            </div>
          </Field>
        </div>
      </Panel>

      <Panel
        title="Engine"
        icon={<Cpu size={17} />}
        actions={
          <button className="secondary-button" onClick={() => refreshEngine()} disabled={loading}>
            <RefreshCw size={15} className={loading ? "spin" : ""} /> Re-check
          </button>
        }
      >
        <p className="muted">
          TubeDeck looks for each program in this order: the path below, the managed folder
          {engine?.managedDir ? (
            <>
              {" "}
              (<code>{engine.managedDir}</code>)
            </>
          ) : null}
          , then your PATH. One-click install and updates arrive in milestone M6.
        </p>
        <div className="binary-grid">
          <BinaryRow
            status={engine?.ytdlp}
            value={settings.ytdlpPath}
            onChange={setPath("ytdlpPath")}
            hint="Required. Official yt-dlp.exe from github.com/yt-dlp/yt-dlp/releases."
          />
          <BinaryRow
            status={engine?.ffmpeg}
            value={settings.ffmpegPath}
            onChange={setPath("ffmpegPath")}
            hint="Needed to merge video + audio, convert audio and embed thumbnails."
          />
          <BinaryRow
            status={engine?.deno}
            value={settings.denoPath}
            onChange={setPath("denoPath")}
            hint="Deno 2.3+ is needed for full YouTube support (yt-dlp's JavaScript challenge solver)."
          />
        </div>
        <small className="field-hint">Path changes apply after Re-check.</small>
      </Panel>

      {paths && inDesktop && (
        <Panel title="App data">
          <div className="path-list">
            {[
              ["Settings & presets", paths.configDir],
              ["Logs", paths.logDir],
            ].map(([label, p]) => (
              <div key={label} className="path-row">
                <span>{label}</span>
                <code>{p}</code>
                <button
                  className="icon-button"
                  title="Show in Explorer"
                  onClick={() => revealInExplorer(p).catch(() => toast("info", "This folder is created the first time something is saved."))}
                >
                  <FolderOpen size={15} />
                </button>
              </div>
            ))}
          </div>
        </Panel>
      )}
    </>
  );
}
