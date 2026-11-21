import {
  Bookmark,
  Download,
  History,
  Moon,
  ScrollText,
  Settings,
  SlidersHorizontal,
  Sun,
  SunMoon,
} from "lucide-react";
import type { ReactNode } from "react";
import { useAppStore } from "../store/useAppStore";
import type { Theme, ViewId } from "../lib/types";
import { EngineStatus } from "./EngineStatus";

const ACTIVE = new Set(["queued", "starting", "downloading", "processing"]);
const FINISHED = new Set(["completed", "failed", "canceled"]);
const NEXT_THEME: Record<Theme, Theme> = { system: "dark", dark: "light", light: "system" };

export function Sidebar() {
  const view = useAppStore((s) => s.view);
  const setView = useAppStore((s) => s.setView);
  const jobs = useAppStore((s) => s.jobs);
  const presets = useAppStore((s) => s.presets);
  const theme = useAppStore((s) => s.settings?.theme ?? "system");
  const updateSettings = useAppStore((s) => s.updateSettings);

  const activeCount = jobs.filter((j) => ACTIVE.has(j.status)).length;
  const finishedCount = jobs.filter((j) => FINISHED.has(j.status)).length;

  const item = (id: ViewId, label: string, icon: ReactNode, count?: number) => (
    <button
      className={`nav-item${view === id ? " selected" : ""}`}
      onClick={() => setView(id)}
      aria-current={view === id ? "page" : undefined}
      title={label}
    >
      {icon}
      <span className="nav-text">{label}</span>
      {count ? <span className="nav-count">{count}</span> : null}
    </button>
  );

  const themeIcon = theme === "dark" ? <Moon size={16} /> : theme === "light" ? <Sun size={16} /> : <SunMoon size={16} />;

  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark" aria-hidden="true">
          <span className="bm-card bm-a" />
          <span className="bm-card bm-b" />
          <span className="bm-play" />
        </div>
        <div className="brand-text">
          <strong>TubeDeck</strong>
          <small>yt-dlp download manager</small>
        </div>
      </div>

      <div className="nav-label">WORKSPACE</div>
      {item("downloads", "Downloads", <Download size={17} />, activeCount)}
      {item("configure", "Configure", <SlidersHorizontal size={17} />)}
      {item("presets", "Presets", <Bookmark size={17} />, presets.length)}

      <div className="nav-label">ACTIVITY</div>
      {item("history", "History", <History size={17} />, finishedCount)}
      {item("logs", "Logs", <ScrollText size={17} />)}

      <div className="sidebar-bottom">
        {item("settings", "Settings", <Settings size={17} />)}
        <button
          className="nav-item"
          onClick={() => updateSettings({ theme: NEXT_THEME[theme] })}
          title={`Theme: ${theme} (click to change)`}
        >
          {themeIcon}
          <span className="nav-text">Theme: {theme}</span>
        </button>
        <EngineStatus />
      </div>
    </aside>
  );
}
