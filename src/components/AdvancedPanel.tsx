import { FolderOpen, Wrench } from "lucide-react";
import { pickFolder } from "../lib/api";
import type { Browser } from "../lib/types";
import { useAppStore, useOptions } from "../store/useAppStore";
import { CustomArgsEditor } from "./CustomArgsEditor";
import { Field, Panel, Select } from "./ui";

const BROWSERS: { value: Browser | ""; label: string }[] = [
  { value: "", label: "Don't use cookies" },
  { value: "firefox", label: "Firefox (most reliable)" },
  { value: "edge", label: "Edge" },
  { value: "chrome", label: "Chrome" },
  { value: "brave", label: "Brave" },
];

export function AdvancedPanel() {
  const o = useOptions();
  const set = useAppStore((s) => s.setOptions);
  const defaultFolder = useAppStore((s) => s.settings!.downloadFolder);

  const browse = async () => {
    const folder = await pickFolder(o.outputFolder || defaultFolder);
    if (folder) set({ outputFolder: folder });
  };

  return (
    <Panel title="Advanced" icon={<Wrench size={17} />}>
      <div className="config-grid">
        <Field label="Output folder" hint={o.outputFolder ? undefined : "Empty = default folder from Settings"} wide>
          <div className="input-with-button">
            <input
              value={o.outputFolder}
              placeholder={defaultFolder}
              onChange={(e) => set({ outputFolder: e.target.value })}
              spellCheck={false}
            />
            <button type="button" className="secondary-button" onClick={browse}>
              <FolderOpen size={15} /> Browse
            </button>
          </div>
        </Field>
        <Field
          label="Filename template"
          hint={
            <>
              yt-dlp output template. Use <code>/</code> for subfolders, e.g. <code>%(uploader)s/%(title)s.%(ext)s</code>
            </>
          }
          wide
        >
          <input
            value={o.filenameTemplate}
            onChange={(e) => set({ filenameTemplate: e.target.value })}
            spellCheck={false}
            className="mono"
          />
        </Field>
        <Field label={`Concurrent fragments: ${o.fragments}`} hint="Parallel connections per download (HLS/DASH).">
          <input
            type="range"
            min={1}
            max={16}
            value={o.fragments}
            onChange={(e) => set({ fragments: Number(e.target.value) })}
          />
        </Field>
        <Field label="Retries">
          <input
            type="number"
            min={0}
            max={100}
            value={o.retries}
            onChange={(e) => set({ retries: Math.max(0, Math.min(100, Number(e.target.value) || 0)) })}
          />
        </Field>
        <Field label="Rate limit" hint="Empty = unlimited. Examples: 500K, 4.2M">
          <input value={o.rateLimit} placeholder="Unlimited" onChange={(e) => set({ rateLimit: e.target.value })} />
        </Field>
        <Field label="Trim filenames" hint="Max filename length; avoids Windows path limits.">
          <input
            type="number"
            min={0}
            max={250}
            value={o.trimFilenames ?? 0}
            onChange={(e) => {
              const n = Number(e.target.value) || 0;
              set({ trimFilenames: n > 0 ? Math.min(250, n) : null });
            }}
          />
        </Field>
        <Field label="Cookies from browser" hint="Needed for age-restricted or members-only videos." wide>
          <Select
            value={o.cookiesFromBrowser ?? ""}
            onChange={(v) => set({ cookiesFromBrowser: v === "" ? null : v })}
            options={BROWSERS}
          />
        </Field>
      </div>
      <CustomArgsEditor />
    </Panel>
  );
}
