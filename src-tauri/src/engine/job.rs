//! Job model shared with the UI (mirrors `Job` in `src/lib/types.ts`).

use crate::options::DownloadOptions;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Starting,
    Downloading,
    Processing,
    Completed,
    Failed,
    Canceled,
}

impl JobStatus {
    pub fn is_finished(self) -> bool {
        matches!(self, JobStatus::Completed | JobStatus::Failed | JobStatus::Canceled)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Overall percentage across all streams (0–100).
    pub percent: Option<f64>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    /// Bytes per second.
    pub speed: Option<f64>,
    /// Seconds remaining for the current stream.
    pub eta: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub status: JobStatus,
    /// Human-readable sub-state, e.g. "Stream 1/2" or "Merging formats".
    pub stage: Option<String>,
    pub progress: Progress,
    pub options: DownloadOptions,
    pub preset_name: Option<String>,
    pub output_path: Option<String>,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub error: Option<String>,
}

impl Job {
    pub fn new(id: String, url: String, options: DownloadOptions, preset_name: Option<String>) -> Self {
        Self {
            id,
            url,
            title: None,
            thumbnail: None,
            status: JobStatus::Queued,
            stage: None,
            progress: Progress::default(),
            options,
            preset_name,
            output_path: None,
            created_at: now_ms(),
            started_at: None,
            finished_at: None,
            error: None,
        }
    }

    /// Reset runtime fields so the job can run again.
    pub fn reset_for_retry(&mut self) {
        self.status = JobStatus::Queued;
        self.stage = None;
        self.progress = Progress::default();
        self.output_path = None;
        self.started_at = None;
        self.finished_at = None;
        self.error = None;
    }
}

pub fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}
