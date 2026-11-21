import { Link2, Plus } from "lucide-react";
import { useState } from "react";
import { api, errorMessage } from "../lib/api";
import { useAppStore } from "../store/useAppStore";
import { optionsSummary } from "../lib/summary";
import { Panel } from "./ui";

export function AddUrlPanel() {
  const draft = useAppStore((s) => s.urlDraft);
  const setDraft = useAppStore((s) => s.setUrlDraft);
  const enqueue = useAppStore((s) => s.enqueue);
  const toast = useAppStore((s) => s.toast);
  const settings = useAppStore((s) => s.settings!);
  const presets = useAppStore((s) => s.presets);
  const applyPreset = useAppStore((s) => s.applyPreset);
  const [busy, setBusy] = useState(false);

  const lineCount = draft.split(/\s+/).filter(Boolean).length;

  const add = async () => {
    if (!draft.trim() || busy) return;
    setBusy(true);
    try {
      const parsed = await api.parseUrls(draft);
      const valid = parsed.filter((p) => !p.error).map((p) => p.url);
      const invalid = parsed.filter((p) => p.error);
      enqueue(valid);
      if (invalid.length) {
        setDraft(invalid.map((p) => p.url).join("\n"));
        toast("error", `${invalid.length} invalid URL${invalid.length > 1 ? "s" : ""} kept in the box: ${invalid[0].error}`);
      } else {
        setDraft("");
      }
      if (valid.length) toast("success", `Added ${valid.length} item${valid.length > 1 ? "s" : ""} to the queue`);
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel
      title="Add downloads"
      icon={<Link2 size={17} />}
      actions={<span className="pill">{lineCount ? `${lineCount} URL${lineCount > 1 ? "s" : ""}` : "Videos & playlists"}</span>}
    >
      <textarea
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
            e.preventDefault();
            void add();
          }
        }}
        placeholder={"Paste one or more video or playlist URLs, one per line\nhttps://www.youtube.com/watch?v=…"}
        spellCheck={false}
        aria-label="URLs to download"
      />
      <div className="add-footer">
        <div className="quick-settings">
          <select
            className="compact"
            value={settings.activePresetId ?? ""}
            onChange={(e) => e.target.value && applyPreset(e.target.value)}
            aria-label="Preset"
          >
            <option value="">Custom settings</option>
            {presets.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          {optionsSummary(settings.currentOptions).map((chip) => (
            <span key={chip} className="chip">
              {chip}
            </span>
          ))}
        </div>
        <button className="primary-button" onClick={add} disabled={!draft.trim() || busy} title="Ctrl+Enter">
          <Plus size={15} />
          Add to queue
        </button>
      </div>
    </Panel>
  );
}
