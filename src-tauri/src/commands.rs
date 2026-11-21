//! Thin `#[tauri::command]` wrappers. Logic lives in the other modules so it can be unit-tested.

use crate::{
    args::{self, BuiltCommand},
    binaries::{self, EngineStatus},
    engine::{Engine, job::Job},
    jsonfile,
    options::DownloadOptions,
    presets::{self, Preset},
    settings::Settings,
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::State;

pub const LOG_FILE_NAME: &str = "tubedeck";

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub user_presets: Mutex<Vec<Preset>>,
    pub config_dir: PathBuf,
    pub log_dir: PathBuf,
    pub managed_bin_dir: PathBuf,
    pub default_download_folder: String,
}

impl AppState {
    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
    fn presets_path(&self) -> PathBuf {
        self.config_dir.join("presets.json")
    }

    /// Load settings and presets from disk. Corrupt files are logged and replaced by defaults
    /// so the app always starts.
    pub fn load(
        config_dir: PathBuf,
        log_dir: PathBuf,
        managed_bin_dir: PathBuf,
        default_download_folder: String,
    ) -> Self {
        let state = Self {
            settings: Mutex::new(Settings::default()),
            user_presets: Mutex::new(Vec::new()),
            config_dir,
            log_dir,
            managed_bin_dir,
            default_download_folder,
        };
        let mut settings = match jsonfile::read::<Settings>(&state.settings_path()) {
            Ok(s) => s.unwrap_or_default(),
            Err(e) => {
                log::error!("{e}; using default settings");
                Settings::default()
            }
        }
        .sanitized();
        if settings.download_folder.trim().is_empty() {
            settings.download_folder = state.default_download_folder.clone();
        }
        let user = match jsonfile::read::<Vec<Preset>>(&state.presets_path()) {
            Ok(p) => p.unwrap_or_default(),
            Err(e) => {
                log::error!("{e}; starting without user presets");
                Vec::new()
            }
        };
        *state.settings.lock().unwrap() = settings;
        *state.user_presets.lock().unwrap() = user;
        state
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
    jsonfile::write(&state.settings_path(), &settings)?;
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
    jsonfile::write(&state.presets_path(), &next)?;
    *user = next;
    Ok(saved)
}

#[tauri::command]
pub fn delete_preset(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let mut user = state.user_presets.lock().unwrap();
    let mut next = user.clone();
    presets::remove(&mut next, &id)?;
    jsonfile::write(&state.presets_path(), &next)?;
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
}

#[tauri::command]
pub fn app_paths(state: State<'_, AppState>) -> AppPaths {
    AppPaths {
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
