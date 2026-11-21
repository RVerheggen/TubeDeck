import { Bookmark, Check, Lock, Pencil, Save, Trash2, X } from "lucide-react";
import { useState } from "react";
import { optionsSummary } from "../lib/summary";
import type { Preset } from "../lib/types";
import { sameOptions, useAppStore } from "../store/useAppStore";
import { Panel } from "./ui";

export function SavePresetForm() {
  const savePresetAs = useAppStore((s) => s.savePresetAs);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    await savePresetAs(name, description);
    setName("");
    setDescription("");
  };

  return (
    <form className="preset-form" onSubmit={submit}>
      <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Preset name" maxLength={60} aria-label="Preset name" />
      <input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Description (optional)" aria-label="Preset description" />
      <button className="primary-button" type="submit" disabled={!name.trim()}>
        <Save size={15} /> Save current settings
      </button>
    </form>
  );
}

function PresetRow({ preset }: { preset: Preset }) {
  const settings = useAppStore((s) => s.settings!);
  const applyPreset = useAppStore((s) => s.applyPreset);
  const updatePreset = useAppStore((s) => s.updatePreset);
  const deletePreset = useAppStore((s) => s.deletePreset);
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(preset.name);
  const [description, setDescription] = useState(preset.description);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const active = settings.activePresetId === preset.id;
  const modified = active && !sameOptions(settings.currentOptions, preset.options);

  if (editing) {
    return (
      <div className="preset-row editing">
        <div className="preset-edit">
          <input value={name} onChange={(e) => setName(e.target.value)} maxLength={60} aria-label="Preset name" />
          <input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Description" aria-label="Preset description" />
        </div>
        <button
          className="icon-button"
          title="Save"
          onClick={async () => {
            await updatePreset({ ...preset, name, description });
            setEditing(false);
          }}
        >
          <Check size={15} />
        </button>
        <button className="icon-button" title="Cancel" onClick={() => setEditing(false)}>
          <X size={15} />
        </button>
      </div>
    );
  }

  return (
    <div className={`preset-row${active ? " active" : ""}`}>
      <div className="preset-icon">{preset.builtin ? <Lock size={16} /> : <Bookmark size={16} />}</div>
      <div className="preset-body">
        <strong>
          {preset.name}
          {active && <span className="pill">{modified ? "Active · modified" : "Active"}</span>}
        </strong>
        {preset.description && <small>{preset.description}</small>}
        <div className="chip-row">
          {optionsSummary(preset.options).map((c) => (
            <span className="chip" key={c}>
              {c}
            </span>
          ))}
        </div>
      </div>
      <div className="preset-actions">
        <button className="secondary-button" onClick={() => applyPreset(preset.id)} disabled={active && !modified}>
          {active && modified ? "Reset to preset" : "Use"}
        </button>
        {!preset.builtin && (
          <>
            {active && modified && (
              <button
                className="icon-button"
                title="Overwrite preset with current settings"
                onClick={() => updatePreset({ ...preset, options: settings.currentOptions })}
              >
                <Save size={15} />
              </button>
            )}
            <button className="icon-button" title="Rename" onClick={() => setEditing(true)}>
              <Pencil size={15} />
            </button>
            {confirmDelete ? (
              <button className="danger-button" onClick={() => deletePreset(preset.id)} onBlur={() => setConfirmDelete(false)} autoFocus>
                Delete?
              </button>
            ) : (
              <button className="icon-button" title="Delete" onClick={() => setConfirmDelete(true)}>
                <Trash2 size={15} />
              </button>
            )}
          </>
        )}
      </div>
    </div>
  );
}

export function PresetList() {
  const presets = useAppStore((s) => s.presets);
  const builtin = presets.filter((p) => p.builtin);
  const user = presets.filter((p) => !p.builtin);

  return (
    <>
      <Panel title="Save as preset" icon={<Save size={17} />}>
        <SavePresetForm />
      </Panel>
      <Panel title="Your presets" icon={<Bookmark size={17} />}>
        {user.length === 0 ? (
          <p className="muted">No custom presets yet. Configure your options and save them above.</p>
        ) : (
          user.map((p) => <PresetRow key={p.id} preset={p} />)
        )}
      </Panel>
      <Panel title="Built-in presets" icon={<Lock size={17} />}>
        {builtin.map((p) => (
          <PresetRow key={p.id} preset={p} />
        ))}
      </Panel>
    </>
  );
}
