import { useAppStore, useOptions } from "../store/useAppStore";

const EXAMPLES = ["--no-mtime", "--restrict-filenames", '--match-filter "duration > 60"', "--write-description"];

export function CustomArgsEditor() {
  const o = useOptions();
  const set = useAppStore((s) => s.setOptions);

  const add = (arg: string) => {
    const cur = o.extraArgs.trim();
    if (cur.split(/\s+/).includes(arg.split(" ")[0])) return;
    set({ extraArgs: cur ? `${cur} ${arg}` : arg });
  };

  return (
    <div className="custom-args">
      <div className="custom-args-head">
        <span className="field-label">Custom yt-dlp arguments</span>
        <div className="chip-row">
          {EXAMPLES.map((ex) => (
            <button key={ex} type="button" className="chip button" onClick={() => add(ex)}>
              + {ex}
            </button>
          ))}
        </div>
      </div>
      <textarea
        className="mono"
        value={o.extraArgs}
        onChange={(e) => set({ extraArgs: e.target.value })}
        placeholder='Appended before the URL. Quote values with spaces: --match-filter "like_count > 100"'
        spellCheck={false}
        rows={2}
        aria-label="Custom yt-dlp arguments"
      />
      <small className="field-hint">
        Backslashes are kept as-is, so Windows paths work without escaping. Warnings appear in the command preview.
      </small>
    </div>
  );
}
