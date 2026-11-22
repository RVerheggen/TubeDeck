//! Persistent app settings (`settings.json` in the app config dir).
//! Moves to SQLite in M4.

use crate::options::DownloadOptions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    /// Default output folder when a job's options don't set one.
    pub download_folder: String,
    pub max_concurrent: u32,
    /// Automatic retries for transient failures (network, HTTP 403/429/5xx). 0 = off.
    pub auto_retry: u32,
    /// Resume downloads that were running or queued when TubeDeck closed.
    pub resume_on_startup: bool,
    /// Optional explicit binary paths; empty = auto-detect.
    pub ytdlp_path: String,
    pub ffmpeg_path: String,
    pub deno_path: String,
    /// Options currently loaded in the Configure view.
    pub current_options: DownloadOptions,
    pub active_preset_id: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            download_folder: String::new(),
            max_concurrent: 3,
            auto_retry: 3,
            resume_on_startup: true,
            ytdlp_path: String::new(),
            ffmpeg_path: String::new(),
            deno_path: String::new(),
            current_options: DownloadOptions::default(),
            active_preset_id: Some("builtin-best-video".into()),
        }
    }
}

impl Settings {
    /// Clamp values that came from disk or the UI into valid ranges.
    pub fn sanitized(mut self) -> Self {
        self.max_concurrent = self.max_concurrent.clamp(1, 10);
        self.auto_retry = self.auto_retry.min(5);
        self
    }
}
