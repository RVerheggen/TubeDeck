import { useEffect, type ReactNode } from "react";
import { CloseGuard } from "./components/CloseGuard";
import { HistoryTable } from "./components/HistoryTable";
import { LogViewer } from "./components/LogViewer";
import { PlaylistPicker } from "./components/PlaylistPicker";
import { PresetList } from "./components/PresetList";
import { Sidebar } from "./components/Sidebar";
import { Toasts } from "./components/Toasts";
import { inDesktop } from "./lib/api";
import type { Theme, ViewId } from "./lib/types";
import { useAppStore } from "./store/useAppStore";
import { ConfigureView } from "./views/ConfigureView";
import { DownloadsView } from "./views/DownloadsView";
import { SettingsView } from "./views/SettingsView";

const VIEWS: Record<ViewId, { title: string; subtitle: string; render: () => ReactNode }> = {
  downloads: { title: "Downloads", subtitle: "Queue videos and playlists and follow their progress.", render: () => <DownloadsView /> },
  configure: { title: "Configure", subtitle: "Format, quality and advanced yt-dlp options, with a live command preview.", render: () => <ConfigureView /> },
  presets: { title: "Presets", subtitle: "Save and reuse your favourite download settings.", render: () => <PresetList /> },
  history: { title: "History", subtitle: "Every finished, failed and canceled download, kept across restarts.", render: () => <HistoryTable /> },
  logs: { title: "Logs", subtitle: "Diagnostic output from TubeDeck and the download engine.", render: () => <LogViewer /> },
  settings: { title: "Settings", subtitle: "Appearance, folders and the yt-dlp engine.", render: () => <SettingsView /> },
};

function useResolvedTheme(theme: Theme | undefined) {
  useEffect(() => {
    const root = document.documentElement;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const resolved = theme === "dark" || theme === "light" ? theme : media.matches ? "dark" : "light";
      root.dataset.theme = resolved;
      root.style.colorScheme = resolved;
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
}

export default function App() {
  const init = useAppStore((s) => s.init);
  const ready = useAppStore((s) => s.ready);
  const loadError = useAppStore((s) => s.loadError);
  const settings = useAppStore((s) => s.settings);
  const view = useAppStore((s) => s.view);

  useEffect(() => {
    void init();
  }, [init]);

  useResolvedTheme(settings?.theme);

  if (!ready) {
    return <div className="splash">Loading TubeDeck…</div>;
  }
  if (loadError || !settings) {
    return (
      <div className="splash error">
        <strong>TubeDeck could not start</strong>
        <p>{loadError ?? "Settings unavailable"}</p>
      </div>
    );
  }

  const v = VIEWS[view];
  return (
    <div className="app">
      <Sidebar />
      <main className="main">
        <header className="topbar">
          <div>
            <div className="breadcrumb">TubeDeck / {v.title}</div>
            <h1>{v.title}</h1>
            <p>{v.subtitle}</p>
          </div>
          {!inDesktop && <span className="pill warn">Browser preview · backend mocked</span>}
        </header>
        {v.render()}
      </main>
      <Toasts />
      <CloseGuard />
      <PlaylistPicker />
    </div>
  );
}
