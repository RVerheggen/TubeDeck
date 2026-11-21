import { AdvancedPanel } from "../components/AdvancedPanel";
import { CommandPreview } from "../components/CommandPreview";
import { ConfigPanel } from "../components/ConfigPanel";
import { useAppStore } from "../store/useAppStore";

export function ConfigureView() {
  const draft = useAppStore((s) => s.urlDraft);
  const setDraft = useAppStore((s) => s.setUrlDraft);

  return (
    <div className="config-layout">
      <div className="config-main">
        <ConfigPanel />
        <AdvancedPanel />
      </div>
      <aside className="config-side">
        <div className="sticky">
          <label className="field">
            <span className="field-label">Preview URL</span>
            <input
              value={draft.split(/\s+/).filter(Boolean)[0] ?? ""}
              onChange={(e) => setDraft(e.target.value)}
              placeholder="https://… (optional)"
              spellCheck={false}
            />
          </label>
          <CommandPreview />
        </div>
      </aside>
    </div>
  );
}
