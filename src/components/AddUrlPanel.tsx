import { Link2, ListChecks, Plus } from "lucide-react";
import { useState } from "react";
import { api, errorMessage } from "../lib/api";
import { useAppStore } from "../store/useAppStore";
import { looksLikePlaylist } from "../lib/playlist";
import { presetState } from "../lib/presets";
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
  const openPicker = useAppStore((s) => s.openPicker);
  const [busy, setBusy] = useState(false);

  // When the options were changed after applying a preset (for example Audio MP3, then
  // "Video" in Configure), show "Custom" so the list doesn't claim the preset is in use and
  // choosing that preset again re-applies it.
  const { preset: activePreset, modified } = presetState(settings, presets);

  const lineCount = draft.split(/\s+/).filter(Boolean).length;

  /** `pickAll`: open the picker for every URL instead of only playlist-looking ones. */
  const add = async (pickAll = false) => {
    if (!draft.trim() || busy) return;
    setBusy(true);
    try {
      const parsed = await api.parseUrls(draft);
      const valid = parsed.filter((p) => !p.error).map((p) => p.url);
      const invalid = parsed.filter((p) => p.error);
      // Playlists go through the picker unless the user asked for single videos only.
      const toPick = pickAll ? valid : settings.currentOptions.noPlaylist ? [] : valid.filter(looksLikePlaylist);
      const direct = valid.filter((u) => !toPick.includes(u));
      if (direct.length) await enqueue(direct);
      if (toPick.length) openPicker(toPick);
      if (invalid.length) {
        setDraft(invalid.map((p) => p.url).join("\n"));
        toast("error", `${invalid.length} invalid URL${invalid.length > 1 ? "s" : ""} kept in the box: ${invalid[0].error}`);
      } else {
        setDraft("");
      }
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
            value={modified ? "" : (settings.activePresetId ?? "")}
            onChange={(e) => e.target.value && applyPreset(e.target.value)}
            aria-label="Preset"
          >
            <option value="">{modified && activePreset ? `Custom (changed from ${activePreset.name})` : "Custom settings"}</option>
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
        <div className="add-buttons">
          <button
            className="secondary-button"
            onClick={() => add(true)}
            disabled={!draft.trim() || busy}
            title="Inspect the URL first and choose which items to download (any site)"
          >
            <ListChecks size={15} />
            Pick items…
          </button>
          <button
            className="primary-button"
            onClick={() => add()}
            disabled={!draft.trim() || busy}
            title="Ctrl+Enter. Playlists open the item picker."
          >
            <Plus size={15} />
            Add to queue
          </button>
        </div>
      </div>
    </Panel>
  );
}
