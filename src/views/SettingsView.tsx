import { ArrowUpCircle, CloudDownload, Cpu, FolderOpen, Info, Monitor, Moon, RefreshCw, Settings as SettingsIcon, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { api, inDesktop, pickFolder, revealInExplorer } from "../lib/api";
import { formatBytes, formatTime } from "../lib/format";
import type { AppPaths, BinaryStatus, Component, ComponentUpdate, Settings } from "../lib/types";
import { COMPONENTS, InstallButton, InstallStatus } from "../components/SetupPanel";
import { ExportDiagnosticsButton } from "../components/LogViewer";
import { Field, Panel, Segmented, Toggle } from "../components/ui";
import { useAppStore } from "../store/useAppStore";

function BinaryRow({
  id,
  status,
  update,
  value,
  onChange,
  hint,
}: {
  id: Component;
  status: BinaryStatus | undefined;
  update: ComponentUpdate | undefined;
  value: string;
  onChange: (v: string) => void;
  hint: string;
}) {
  const progress = useAppStore((s) => s.installs[id]);
  const state = !status ? "unknown" : status.ok ? "ok" : status.found ? "degraded" : "missing";
  const managed = status?.source === "managed";
  const size = COMPONENTS.find((c) => c.id === id)!.size;
  const action = !managed
    ? status?.ok
      ? `Install managed copy (${formatBytes(size)})`
      : `Install (${formatBytes(size)})`
    : update?.updateAvailable
      ? `Update to ${update.latest}`
      : "Reinstall";
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
      {update && (
        <small className={update.updateAvailable ? "update-text" : "muted"}>
          {update.error
            ? `Couldn't check: ${update.error}`
            : update.latest
              ? update.updateAvailable
                ? `Update available: ${update.latest}`
                : managed
                  ? `Up to date (latest ${update.latest})`
                  : `Latest is ${update.latest}${status?.source === "path" ? "; this copy is managed outside TubeDeck" : ""}`
              : id === "ffmpeg"
                ? "Rolling build; reinstall to get the newest"
                : ""}
        </small>
      )}
      {progress && progress.phase !== "done" ? <InstallStatus progress={progress} /> : null}
      <div className="binary-actions">
        <InstallButton id={id} label={action} primary={!status?.ok || !!update?.updateAvailable} />
      </div>
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

function AboutPanel() {
  const version = useAppStore((s) => s.appVersion);
  const update = useAppStore((s) => s.appUpdate);
  const phase = useAppStore((s) => s.appUpdatePhase);
  const error = useAppStore((s) => s.appUpdateError);
  const check = useAppStore((s) => s.checkAppUpdate);
  const openDialog = useAppStore((s) => s.setAppUpdateDialog);
  const status =
    phase === "checking"
      ? "Checking…"
      : update
        ? `Version ${update.version} is available.`
        : phase === "uptodate"
          ? "You have the latest version."
          : phase === "error"
            ? `Couldn't check: ${error}`
            : "TubeDeck checks for updates when it starts and every 6 hours.";
  return (
    <Panel
      title="About TubeDeck"
      icon={<Info size={17} />}
      actions={
        update ? (
          <button className="primary-button" onClick={() => openDialog(true)}>
            <ArrowUpCircle size={15} /> Update to {update.version}
          </button>
        ) : (
          <button className="secondary-button" onClick={() => check(false)} disabled={phase === "checking"}>
            <RefreshCw size={15} className={phase === "checking" ? "spin" : ""} /> Check for TubeDeck updates
          </button>
        )
      }
    >
      <p>
        <strong className="about-version">TubeDeck {version || "…"}</strong> · {status}
      </p>
      <p className="muted small">
        Updates come from the project's GitHub releases and are verified with TubeDeck's signing key before they're installed.
      </p>
    </Panel>
  );
}

export function SettingsView() {
  const settings = useAppStore((s) => s.settings!);
  const update = useAppStore((s) => s.updateSettings);
  const engine = useAppStore((s) => s.engine);
  const loading = useAppStore((s) => s.engineLoading);
  const refreshEngine = useAppStore((s) => s.refreshEngine);
  const toast = useAppStore((s) => s.toast);
  const updates = useAppStore((s) => s.updates);
  const checking = useAppStore((s) => s.checkingUpdates);
  const checkUpdates = useAppStore((s) => s.checkUpdates);
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
          <Field label={`Simultaneous downloads: ${settings.maxConcurrent}`} hint="Changes apply immediately.">
            <input
              type="range"
              min={1}
              max={10}
              value={settings.maxConcurrent}
              onChange={(e) => update({ maxConcurrent: Number(e.target.value) })}
            />
          </Field>
          <Field
            label={`Automatic retries: ${settings.autoRetry === 0 ? "off" : settings.autoRetry}`}
            hint="For temporary failures (network errors, HTTP 403/429/5xx). Waits 5s, 20s, 60s, … between attempts and resumes partial files."
          >
            <input
              type="range"
              min={0}
              max={5}
              value={settings.autoRetry}
              onChange={(e) => update({ autoRetry: Number(e.target.value) })}
            />
          </Field>
          <div className="field wide">
            <Toggle
              label="Resume downloads when TubeDeck starts"
              hint="Downloads that were running or queued when you closed TubeDeck continue automatically. When off, they wait as paused."
              checked={settings.resumeOnStartup}
              onChange={(resumeOnStartup) => update({ resumeOnStartup })}
            />
          </div>
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
          <>
            <button className="secondary-button" onClick={() => checkUpdates()} disabled={checking}>
              <CloudDownload size={15} className={checking ? "spin" : ""} /> Check for updates
            </button>
            <button className="secondary-button" onClick={() => refreshEngine()} disabled={loading}>
              <RefreshCw size={15} className={loading ? "spin" : ""} /> Re-check
            </button>
          </>
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
          , then your PATH. Managed copies come from the official GitHub releases and are checked against their published
          SHA-256 checksums.
        </p>
        <div className="config-grid engine-options">
          <Field label="yt-dlp release channel" hint="Nightly gets site fixes sooner; stable is tested longer.">
            <Segmented
              label="yt-dlp release channel"
              value={settings.ytdlpChannel}
              onChange={(ytdlpChannel) => update({ ytdlpChannel })}
              options={[
                { value: "stable", label: "Stable" },
                { value: "nightly", label: "Nightly" },
              ]}
            />
          </Field>
          <div className="field">
            <Toggle
              label="Keep yt-dlp up to date automatically"
              hint="Checks once a day at startup and updates the managed copy. Sites like YouTube break old versions quickly."
              checked={settings.autoUpdate}
              onChange={(autoUpdate) => update({ autoUpdate })}
            />
            {settings.lastUpdateCheck > 0 && <small className="field-hint">Last automatic check: {formatTime(settings.lastUpdateCheck)}</small>}
          </div>
        </div>
        <div className="binary-grid">
          <BinaryRow
            id="ytdlp"
            status={engine?.ytdlp}
            update={updates?.ytdlp}
            value={settings.ytdlpPath}
            onChange={setPath("ytdlpPath")}
            hint="Required. Official yt-dlp.exe from github.com/yt-dlp/yt-dlp/releases."
          />
          <BinaryRow
            id="ffmpeg"
            status={engine?.ffmpeg}
            update={updates?.ffmpeg}
            value={settings.ffmpegPath}
            onChange={setPath("ffmpegPath")}
            hint="Needed to merge video + audio, convert audio and embed thumbnails."
          />
          <BinaryRow
            id="deno"
            status={engine?.deno}
            update={updates?.deno}
            value={settings.denoPath}
            onChange={setPath("denoPath")}
            hint="Deno 2.3+ is needed for full YouTube support (yt-dlp's JavaScript challenge solver)."
          />
        </div>
        <small className="field-hint">Path changes apply after Re-check.</small>
      </Panel>

      <AboutPanel />

      {paths && inDesktop && (
        <Panel title="App data" actions={<ExportDiagnosticsButton />}>
          <div className="path-list">
            {[
              ["Database & binaries", paths.dataDir],
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
