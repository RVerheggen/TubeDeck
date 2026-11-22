//! Thin `#[tauri::command]` wrappers. Logic lives in the other modules so it can be unit-tested.

use crate::{
    args::{self, BuiltCommand},
    binaries::{self, EngineStatus},
    db::{Db, HistoryPage, HistoryQuery},
    engine::{
        Engine, PlaylistRequest,
        job::Job,
        probe::{self, ProbeResult, Probes},
        runner,
    },
    installer::{self, Component, UpdateInfo},
    jsonfile,
    options::Browser,
    options::DownloadOptions,
    presets::{self, Preset},
    settings::Settings,
};
use serde::Serialize;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter, Manager, State};

pub const LOG_FILE_NAME: &str = "tubedeck";

pub struct AppState {
    pub db: Arc<Db>,
    pub settings: Mutex<Settings>,
    pub user_presets: Mutex<Vec<Preset>>,
    pub config_dir: PathBuf,
    pub log_dir: PathBuf,
    pub managed_bin_dir: PathBuf,
    pub default_download_folder: String,
}

impl AppState {
    /// Load settings and presets from the database. On first run, settings and presets from
    /// the M1–M3 JSON files are imported. Unreadable data is logged and replaced by defaults
    /// so the app always starts.
    pub fn load(
        db: Arc<Db>,
        config_dir: PathBuf,
        log_dir: PathBuf,
        managed_bin_dir: PathBuf,
        default_download_folder: String,
    ) -> Self {
        let mut settings = match db.load_settings() {
            Ok(Some(s)) => s,
            Ok(None) => import_legacy::<Settings>(&config_dir.join("settings.json"))
                .inspect(|s| {
                    if let Err(e) = db.save_settings(s) {
                        log::error!("Could not import settings: {e}");
                    }
                })
                .unwrap_or_default(),
            Err(e) => {
                log::error!("{e}; using default settings");
                Settings::default()
            }
        }
        .sanitized();
        if settings.download_folder.trim().is_empty() {
            settings.download_folder = default_download_folder.clone();
        }

        let mut user = db.load_presets().unwrap_or_else(|e| {
            log::error!("{e}; starting without user presets");
            Vec::new()
        });
        if user.is_empty()
            && let Some(legacy) = import_legacy::<Vec<Preset>>(&config_dir.join("presets.json"))
        {
            match db.save_presets(&legacy) {
                Ok(()) => user = legacy,
                Err(e) => log::error!("Could not import presets: {e}"),
            }
        }

        Self {
            db,
            settings: Mutex::new(settings),
            user_presets: Mutex::new(user),
            config_dir,
            log_dir,
            managed_bin_dir,
            default_download_folder,
        }
    }
}

/// Read a pre-M4 JSON file and rename it so it's imported only once.
fn import_legacy<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    match jsonfile::read::<T>(path) {
        Ok(Some(v)) => {
            let _ = std::fs::rename(path, path.with_extension("json.imported"));
            log::info!("Imported {} into the database", path.display());
            Some(v)
        }
        Ok(None) => None,
        Err(e) => {
            log::warn!("{e}; not imported");
            None
        }
    }
}

type CmdResult<T> = Result<T, String>;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    engine: State<'_, Arc<Engine>>,
    settings: Settings,
) -> CmdResult<Settings> {
    let mut settings = settings.sanitized();
    if settings.download_folder.trim().is_empty() {
        settings.download_folder = state.default_download_folder.clone();
    }
    state.db.save_settings(&settings)?;
    *state.settings.lock().unwrap() = settings.clone();
    // A higher concurrency limit may allow more queued jobs to start now.
    engine.schedule();
    Ok(settings)
}

// ---- Download engine -------------------------------------------------------------------

#[tauri::command]
pub fn enqueue_jobs(
    engine: State<'_, Arc<Engine>>,
    urls: Vec<String>,
    options: DownloadOptions,
    preset_name: Option<String>,
) -> CmdResult<Vec<Job>> {
    engine.enqueue(urls, options, preset_name)
}

#[tauri::command]
pub fn list_jobs(engine: State<'_, Arc<Engine>>) -> Vec<Job> {
    engine.list()
}

#[tauri::command]
pub fn cancel_job(engine: State<'_, Arc<Engine>>, id: String) -> CmdResult<()> {
    engine.cancel(&id)
}

#[tauri::command]
pub fn pause_job(engine: State<'_, Arc<Engine>>, id: String) -> CmdResult<()> {
    engine.pause(&id)
}

#[tauri::command]
pub fn resume_job(engine: State<'_, Arc<Engine>>, id: String) -> CmdResult<()> {
    engine.resume(&id)
}

#[tauri::command]
pub fn pause_all_jobs(engine: State<'_, Arc<Engine>>) -> CmdResult<()> {
    engine.pause_all()
}

#[tauri::command]
pub fn resume_all_jobs(engine: State<'_, Arc<Engine>>) -> CmdResult<()> {
    engine.resume_all()
}

/// Move a job to position `to` in the queue (0 = top).
#[tauri::command]
pub fn move_job(engine: State<'_, Arc<Engine>>, id: String, to: usize) -> CmdResult<()> {
    engine.move_to(&id, to)
}

#[tauri::command]
pub fn retry_job(engine: State<'_, Arc<Engine>>, id: String) -> CmdResult<()> {
    engine.retry(&id)
}

#[tauri::command]
pub fn remove_job(engine: State<'_, Arc<Engine>>, id: String) -> CmdResult<()> {
    engine.remove(&[id])
}

#[tauri::command]
pub fn clear_finished_jobs(engine: State<'_, Arc<Engine>>) -> CmdResult<()> {
    engine.clear_finished()
}

#[tauri::command]
pub fn get_job_log(engine: State<'_, Arc<Engine>>, id: String) -> Vec<String> {
    engine.log(&id)
}

// ---- Managed binaries ------------------------------------------------------------------

/// Components currently being installed (one install per component at a time).
#[derive(Default)]
pub struct Installs(Mutex<HashSet<Component>>);

pub const EVENT_NOTICE: &str = "engine://notice";
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

#[tauri::command]
pub async fn install_binary(
    app: AppHandle,
    state: State<'_, AppState>,
    installs: State<'_, Installs>,
    component: Component,
) -> CmdResult<()> {
    install_component(&app, &state, &installs, component).await
}

async fn install_component(
    app: &AppHandle,
    state: &AppState,
    installs: &Installs,
    component: Component,
) -> CmdResult<()> {
    if !installs.0.lock().unwrap().insert(component) {
        return Err("Already installing".into());
    }
    let channel = state.settings.lock().unwrap().ytdlp_channel;
    let bin = state.managed_bin_dir.clone();
    let emitter = app.clone();
    log::info!(
        "Installing {component:?} ({channel:?}) into {}",
        bin.display()
    );
    let result = installer::install(component, channel, &bin, |p| {
        let _ = emitter.emit(installer::EVENT_INSTALL, p);
    })
    .await;
    installs.0.lock().unwrap().remove(&component);
    match &result {
        Ok(()) => log::info!("Installed {component:?}"),
        Err(e) => {
            log::error!("Installing {component:?} failed: {e}");
            let _ = app.emit(
                installer::EVENT_INSTALL,
                installer::InstallProgress {
                    component,
                    phase: "error",
                    downloaded: 0,
                    total: None,
                    message: Some(e.clone()),
                },
            );
        }
    }
    result
}

async fn current_status(state: &AppState) -> CmdResult<EngineStatus> {
    let (y, f, d) = {
        let s = state.settings.lock().unwrap();
        (
            s.ytdlp_path.clone(),
            s.ffmpeg_path.clone(),
            s.deno_path.clone(),
        )
    };
    let dir = state.managed_bin_dir.clone();
    tauri::async_runtime::spawn_blocking(move || binaries::engine_status(&y, &f, &d, &dir))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_updates(state: State<'_, AppState>) -> CmdResult<UpdateInfo> {
    let status = current_status(&state).await?;
    let channel = state.settings.lock().unwrap().ytdlp_channel;
    Ok(installer::check(&status, channel).await)
}

/// Runs once after startup: tidy the managed folder, and (at most daily) update the managed
/// yt-dlp, since sites like YouTube break older versions within weeks.
pub async fn startup_maintenance(app: AppHandle) {
    let state = app.state::<AppState>();
    installer::cleanup(&state.managed_bin_dir);
    let (auto, last, channel) = {
        let s = state.settings.lock().unwrap();
        (s.auto_update, s.last_update_check, s.ytdlp_channel)
    };
    let now = crate::engine::job::now_ms();
    if !auto || now.saturating_sub(last) < DAY_MS {
        return;
    }
    let Ok(status) = current_status(&state).await else {
        return;
    };
    if status.ytdlp.source != Some(binaries::Source::Managed) {
        return; // only update copies TubeDeck installed
    }
    let info = installer::check(&status, channel).await;
    if info.ytdlp.error.is_some() {
        return; // offline: try again next start
    }
    {
        let mut s = state.settings.lock().unwrap();
        s.last_update_check = now;
        if let Err(e) = state.db.save_settings(&s) {
            log::warn!("{e}");
        }
    }
    if info.ytdlp.update_available {
        let latest = info.ytdlp.latest.clone().unwrap_or_default();
        let installs = app.state::<Installs>();
        let msg = match install_component(&app, &state, &installs, Component::Ytdlp).await {
            Ok(()) => format!("yt-dlp was updated to {latest}."),
            Err(e) => format!("yt-dlp update to {latest} failed: {e}"),
        };
        let _ = app.emit(EVENT_NOTICE, msg);
    } else {
        log::info!(
            "yt-dlp {} is up to date",
            info.ytdlp.installed.unwrap_or_default()
        );
    }
}

// ---- Playlists -------------------------------------------------------------------------

/// Inspect a URL: single video or playlist (with its entries). `token` lets the UI cancel.
#[tauri::command]
pub async fn probe_url(
    state: State<'_, AppState>,
    probes: State<'_, Probes>,
    url: String,
    token: String,
    cookies_from_browser: Option<Browser>,
) -> CmdResult<ProbeResult> {
    let tools = runner::tools(&state)?;
    let started = std::time::Instant::now();
    let result = probe::run(&probes, &token, url.trim(), &tools, cookies_from_browser).await;
    match &result {
        Ok(ProbeResult::Playlist { entries, title, .. }) => log::info!(
            "Probed playlist \"{title}\": {} entries in {:.1}s",
            entries.len(),
            started.elapsed().as_secs_f32()
        ),
        Ok(ProbeResult::Video { .. }) => log::info!(
            "Probed single video in {:.1}s",
            started.elapsed().as_secs_f32()
        ),
        Err(e) => log::warn!("Probe failed: {e}"),
    }
    result
}

#[tauri::command]
pub fn cancel_probe(probes: State<'_, Probes>, token: String) {
    probes.cancel(&token);
}

#[tauri::command]
pub fn enqueue_playlist(
    engine: State<'_, Arc<Engine>>,
    request: PlaylistRequest,
) -> CmdResult<Vec<Job>> {
    engine.enqueue_playlist(request)
}

// ---- History ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_history(state: State<'_, AppState>, query: HistoryQuery) -> CmdResult<HistoryPage> {
    state.db.history(&query)
}

#[tauri::command]
pub fn delete_history(engine: State<'_, Arc<Engine>>, ids: Vec<String>) -> CmdResult<()> {
    engine.delete_history(&ids)
}

#[tauri::command]
pub fn clear_history(engine: State<'_, Arc<Engine>>) -> CmdResult<()> {
    engine.clear_history()
}

/// Queue a new download with the same URL and options as a finished job.
#[tauri::command]
pub fn redownload(
    state: State<'_, AppState>,
    engine: State<'_, Arc<Engine>>,
    id: String,
) -> CmdResult<Job> {
    let old = state.db.job(&id)?.ok_or("Job not found in history")?;
    let mut created = engine.enqueue(vec![old.url], old.options, old.preset_name)?;
    created.pop().ok_or_else(|| "Nothing was queued".into())
}

/// Write a plain-text diagnostics report (versions, settings, recent logs) to `path`.
#[tauri::command]
pub async fn export_diagnostics(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    engine: State<'_, Arc<Engine>>,
    path: String,
) -> CmdResult<()> {
    let (y, f, d, settings) = {
        let s = state.settings.lock().unwrap();
        (
            s.ytdlp_path.clone(),
            s.ffmpeg_path.clone(),
            s.deno_path.clone(),
            s.clone(),
        )
    };
    let dir = state.managed_bin_dir.clone();
    let status =
        tauri::async_runtime::spawn_blocking(move || binaries::engine_status(&y, &f, &d, &dir))
            .await
            .map_err(|e| e.to_string())?;
    let app_log = read_app_log(state.clone(), Some(400))
        .map(|l| l.lines)
        .unwrap_or_default();
    let report = crate::diagnostics::report(
        &app.package_info().version.to_string(),
        &status,
        &settings,
        &engine.list(),
        &app_log,
        |id| engine.log(id),
    );
    std::fs::write(&path, report).map_err(|e| format!("Could not write {path}: {e}"))
}

#[tauri::command]
pub fn build_command(
    state: State<'_, AppState>,
    url: String,
    options: DownloadOptions,
    default_folder: Option<String>,
) -> BuiltCommand {
    // The UI passes its (possibly not-yet-saved) default folder so the preview never lags.
    let folder = default_folder
        .filter(|f| !f.trim().is_empty())
        .unwrap_or_else(|| state.settings.lock().unwrap().download_folder.clone());
    args::build(&url, &options, &folder)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedUrl {
    pub url: String,
    pub error: Option<String>,
}

/// Split pasted text into URLs (one per line or whitespace-separated), dropping duplicates.
#[tauri::command]
pub fn parse_urls(text: String) -> Vec<ParsedUrl> {
    parse_url_list(&text)
}

pub fn parse_url_list(text: &str) -> Vec<ParsedUrl> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace()
        .filter(|t| seen.insert(t.to_string()))
        .map(|t| ParsedUrl {
            url: t.to_string(),
            error: args::validate_url(t).err(),
        })
        .collect()
}

#[tauri::command]
pub fn list_presets(state: State<'_, AppState>) -> Vec<Preset> {
    presets::all(&state.user_presets.lock().unwrap())
}

#[tauri::command]
pub fn save_preset(state: State<'_, AppState>, preset: Preset) -> CmdResult<Preset> {
    let mut user = state.user_presets.lock().unwrap();
    let mut next = user.clone();
    let saved = presets::upsert(&mut next, preset)?;
    state.db.save_presets(&next)?;
    *user = next;
    Ok(saved)
}

#[tauri::command]
pub fn delete_preset(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let mut user = state.user_presets.lock().unwrap();
    let mut next = user.clone();
    presets::remove(&mut next, &id)?;
    state.db.save_presets(&next)?;
    *user = next;
    Ok(())
}

#[tauri::command]
pub async fn engine_status(state: State<'_, AppState>) -> CmdResult<EngineStatus> {
    let (y, f, d) = {
        let s = state.settings.lock().unwrap();
        (
            s.ytdlp_path.clone(),
            s.ffmpeg_path.clone(),
            s.deno_path.clone(),
        )
    };
    let dir = state.managed_bin_dir.clone();
    tauri::async_runtime::spawn_blocking(move || binaries::engine_status(&y, &f, &d, &dir))
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLog {
    pub path: String,
    pub lines: Vec<String>,
}

/// Last `max_lines` lines of the application log.
#[tauri::command]
pub fn read_app_log(state: State<'_, AppState>, max_lines: Option<usize>) -> CmdResult<AppLog> {
    let path = state.log_dir.join(format!("{LOG_FILE_NAME}.log"));
    let max = max_lines.unwrap_or(500).clamp(1, 5000);
    let text = match std::fs::read(&path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("Could not read log: {e}")),
    };
    let lines: Vec<String> = text.lines().map(String::from).collect();
    let start = lines.len().saturating_sub(max);
    Ok(AppLog {
        path: path.display().to_string(),
        lines: lines[start..].to_vec(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub config_dir: String,
    pub log_dir: String,
    pub managed_bin_dir: String,
    pub default_download_folder: String,
    pub data_dir: String,
}

#[tauri::command]
pub fn app_paths(state: State<'_, AppState>) -> AppPaths {
    AppPaths {
        data_dir: state
            .managed_bin_dir
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        config_dir: state.config_dir.display().to_string(),
        log_dir: state.log_dir.display().to_string(),
        managed_bin_dir: state.managed_bin_dir.display().to_string(),
        default_download_folder: state.default_download_folder.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_dedupes_urls() {
        let r =
            parse_url_list("https://a.com/1\n  https://a.com/1 https://b.com/2\r\nnot-a-url\n\n");
        assert_eq!(r.len(), 3);
        assert!(r[0].error.is_none());
        assert!(r[1].error.is_none());
        assert!(r[2].error.is_some());
    }
}
