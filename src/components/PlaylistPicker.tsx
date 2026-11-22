import { ArrowLeft, CheckSquare, Film, FolderTree, ListOrdered, ListVideo, LoaderCircle, Square, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { api, errorMessage } from "../lib/api";
import { formatDuration, isVideoInPlaylist, parseRange } from "../lib/playlist";
import type { ProbeEntry, ProbeResult } from "../lib/types";
import { useAppStore } from "../store/useAppStore";
import { Toggle } from "./ui";

type State =
  | { phase: "probing" }
  | { phase: "error"; message: string }
  | { phase: "ready"; result: ProbeResult };

/** Inspects the first URL in the picker queue and lets the user choose what to add. */
export function PlaylistPicker() {
  const url = useAppStore((s) => s.pickerUrls[0] ?? null);
  return url ? <Picker key={url} initialUrl={url} /> : null;
}

function Picker({ initialUrl }: { initialUrl: string }) {
  const settings = useAppStore((s) => s.settings!);
  const presets = useAppStore((s) => s.presets);
  const nextPicker = useAppStore((s) => s.nextPicker);
  const enqueue = useAppStore((s) => s.enqueue);
  const applyJobUpdates = useAppStore((s) => s.applyJobUpdates);
  const toast = useAppStore((s) => s.toast);

  // A stack so nested playlists (e.g. a channel's Playlists tab) can be opened and left.
  const [stack, setStack] = useState<string[]>([initialUrl]);
  const url = stack[stack.length - 1];
  const [state, setState] = useState<State>({ phase: "probing" });
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [filter, setFilter] = useState("");
  const [range, setRange] = useState("");
  const [subfolder, setSubfolder] = useState(true);
  const [numbered, setNumbered] = useState(true);
  const [adding, setAdding] = useState(false);
  const lastClicked = useRef<number | null>(null);

  useEffect(() => {
    const token = `p${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
    let done = false;
    setState({ phase: "probing" });
    setFilter("");
    setRange("");
    api
      .probeUrl(url, token, settings.currentOptions.cookiesFromBrowser)
      .then((result) => {
        done = true;
        setState({ phase: "ready", result });
        if (result.kind === "playlist") {
          setSelected(new Set(result.entries.filter((e) => !e.unavailable && !e.isPlaylist).map((e) => e.index)));
        }
      })
      .catch((e) => {
        done = true;
        const message = errorMessage(e);
        if (message !== "Canceled") setState({ phase: "error", message });
      });
    return () => {
      if (!done) void api.cancelProbe(token);
    };
  }, [url, settings.currentOptions.cookiesFromBrowser]);

  const playlist = state.phase === "ready" && state.result.kind === "playlist" ? state.result : null;
  const entries = playlist?.entries ?? [];
  const selectable = useMemo(() => entries.filter((e) => !e.unavailable && !e.isPlaylist), [entries]);
  const q = filter.trim().toLowerCase();
  const visible = q ? entries.filter((e) => e.title.toLowerCase().includes(q)) : entries;
  const chosen = selectable.filter((e) => selected.has(e.index));
  const totalSeconds = chosen.reduce((s, e) => s + (e.duration ?? 0), 0);
  const presetName = presets.find((p) => p.id === settings.activePresetId)?.name ?? null;

  const toggle = (e: ProbeEntry, shift: boolean) => {
    const next = new Set(selected);
    const on = !next.has(e.index);
    if (shift && lastClicked.current != null) {
      const [a, b] = [lastClicked.current, e.index].sort((x, y) => x - y);
      for (const s of selectable) if (s.index >= a && s.index <= b) on ? next.add(s.index) : next.delete(s.index);
    } else {
      on ? next.add(e.index) : next.delete(e.index);
    }
    lastClicked.current = e.index;
    setSelected(next);
  };

  const setVisible = (on: boolean) => {
    const next = new Set(selected);
    for (const e of visible) if (!e.unavailable && !e.isPlaylist) on ? next.add(e.index) : next.delete(e.index);
    setSelected(next);
  };

  const rangeSet = range.trim() ? parseRange(range, entries.length) : null;
  const rangeInvalid = range.trim() !== "" && rangeSet == null;
  const applyRange = () => {
    if (!rangeSet) return;
    setSelected(new Set(selectable.filter((e) => rangeSet.has(e.index)).map((e) => e.index)));
  };

  const addPlaylist = async () => {
    if (!playlist || chosen.length === 0) return;
    setAdding(true);
    try {
      const created = await api.enqueuePlaylist({
        playlistId: playlist.id,
        title: playlist.title,
        count: entries.length,
        items: chosen.map((e) => ({ url: e.url!, index: e.index, title: e.title, thumbnail: e.thumbnail })),
        options: settings.currentOptions,
        presetName,
        subfolder,
        numbered,
      });
      applyJobUpdates(created);
      toast("success", `Added ${created.length} item${created.length > 1 ? "s" : ""} from "${playlist.title}"`);
      nextPicker();
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setAdding(false);
    }
  };

  const addSingle = async (singleUrl: string) => {
    setAdding(true);
    await enqueue([singleUrl], { noPlaylist: true });
    setAdding(false);
    nextPicker();
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <div className="modal picker" role="dialog" aria-modal="true" aria-labelledby="picker-title">
        <header className="picker-head">
          {stack.length > 1 && (
            <button className="icon-button" title="Back" aria-label="Back" onClick={() => setStack(stack.slice(0, -1))}>
              <ArrowLeft size={15} />
            </button>
          )}
          <div className="picker-title">
            <h2 id="picker-title">
              <ListVideo size={18} />
              {state.phase === "ready" ? state.result.title : "Inspecting URL…"}
            </h2>
            <small title={url}>
              {playlist
                ? `${playlist.uploader ? `${playlist.uploader} · ` : ""}${entries.length} item${entries.length === 1 ? "" : "s"}`
                : url}
            </small>
          </div>
          <button className="icon-button" title="Close" aria-label="Close" onClick={nextPicker}>
            <X size={15} />
          </button>
        </header>

        {state.phase === "probing" && (
          <div className="picker-status">
            <LoaderCircle size={22} className="spin" />
            <span>Asking yt-dlp what's at this URL…</span>
            <small>Large channels can take a little while.</small>
          </div>
        )}

        {state.phase === "error" && (
          <div className="picker-status error">
            <strong>Couldn't inspect this URL</strong>
            <span>{state.message}</span>
            <div className="modal-actions">
              <button className="secondary-button" onClick={nextPicker}>Close</button>
              <button className="primary-button" onClick={() => addSingle(url)}>Add as a single download anyway</button>
            </div>
          </div>
        )}

        {state.phase === "ready" && state.result.kind === "video" && (
          <div className="picker-status">
            <div className="single-video">
              {state.result.thumbnail ? <img src={state.result.thumbnail} alt="" referrerPolicy="no-referrer" /> : <Film size={28} />}
              <div>
                <strong>{state.result.title}</strong>
                <small>
                  {state.result.uploader ?? ""} {formatDuration(state.result.duration)}
                </small>
              </div>
            </div>
            <span>This URL is a single video, not a playlist.</span>
            <div className="modal-actions">
              <button className="secondary-button" onClick={nextPicker}>Cancel</button>
              <button className="primary-button" onClick={() => addSingle(state.result.url)} disabled={adding}>
                Add to queue
              </button>
            </div>
          </div>
        )}

        {playlist && (
          <>
            <div className="picker-tools">
              <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Filter by title…" aria-label="Filter items" />
              <div className={`range-input${rangeInvalid ? " invalid" : ""}`}>
                <input
                  value={range}
                  onChange={(e) => setRange(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && applyRange()}
                  placeholder="Range, e.g. 1-10, 15"
                  aria-label="Select a range of items"
                  title='Select by position: "1-10", "15", "20-" (to the end). Press Enter.'
                />
                <button className="secondary-button" onClick={applyRange} disabled={!rangeSet}>Select</button>
              </div>
              <button className="secondary-button" onClick={() => setVisible(true)}>
                <CheckSquare size={14} /> {q ? "Select shown" : "All"}
              </button>
              <button className="secondary-button" onClick={() => setVisible(false)}>
                <Square size={14} /> {q ? "Clear shown" : "None"}
              </button>
            </div>

            <div className="picker-list" role="listbox" aria-multiselectable="true" aria-label="Playlist items">
              {visible.length === 0 && <div className="muted picker-empty">No items match "{filter}".</div>}
              {visible.map((e) => {
                const disabled = e.unavailable || e.isPlaylist;
                const on = selected.has(e.index) && !disabled;
                return (
                  <div
                    key={e.index}
                    className={`picker-row${on ? " on" : ""}${disabled ? " disabled" : ""}`}
                    role="option"
                    aria-selected={on}
                    aria-disabled={disabled}
                    onClick={(ev) => !disabled && toggle(e, ev.shiftKey)}
                  >
                    <input type="checkbox" checked={on} disabled={disabled} readOnly tabIndex={-1} aria-hidden="true" />
                    <span className="picker-index">{e.index}</span>
                    <span className="picker-thumb" aria-hidden="true">
                      {e.thumbnail ? <img src={e.thumbnail} alt="" loading="lazy" referrerPolicy="no-referrer" /> : <Film size={14} />}
                    </span>
                    <span className="picker-name" title={e.title}>
                      {e.title}
                      {e.unavailable && <em> · unavailable</em>}
                    </span>
                    {e.isPlaylist ? (
                      <button
                        className="secondary-button small-button"
                        onClick={(ev) => {
                          ev.stopPropagation();
                          if (e.url) setStack([...stack, e.url]);
                        }}
                        disabled={!e.url}
                      >
                        Open playlist
                      </button>
                    ) : (
                      <span className="picker-duration">{formatDuration(e.duration)}</span>
                    )}
                  </div>
                );
              })}
            </div>

            <div className="picker-options">
              <Toggle label={`Save in a folder named "${playlist.title}"`} checked={subfolder} onChange={setSubfolder} />
              <Toggle label="Number files by playlist position (01 - …)" checked={numbered} onChange={setNumbered} />
              <span className="muted small picker-hint">
                <FolderTree size={13} /> <ListOrdered size={13} /> Uses the current settings{presetName ? ` (${presetName})` : ""}. Shift+click selects a range.
              </span>
            </div>

            <footer className="modal-actions picker-foot">
              <span className="muted small">
                {chosen.length} of {selectable.length} selected{totalSeconds > 0 ? ` · ${formatDuration(totalSeconds)}` : ""}
              </span>
              {isVideoInPlaylist(initialUrl) && stack.length === 1 && (
                <button className="secondary-button" onClick={() => addSingle(initialUrl)} disabled={adding}>
                  Only the linked video
                </button>
              )}
              <button className="primary-button" onClick={addPlaylist} disabled={chosen.length === 0 || adding}>
                Add {chosen.length} item{chosen.length === 1 ? "" : "s"}
              </button>
            </footer>
          </>
        )}
      </div>
    </div>
  );
}
