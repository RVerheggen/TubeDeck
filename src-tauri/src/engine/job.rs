//! Job model shared with the UI (mirrors `Job` in `src/lib/types.ts`).

use super::errors::ErrorKind;
use crate::options::DownloadOptions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Starting,
    Downloading,
    Processing,
    Paused,
    Completed,
    Failed,
    Canceled,
}

impl JobStatus {
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Canceled
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub error_kind: Option<ErrorKind>,
    pub error_hint: Option<String>,
    /// Automatic retries made so far for the current run.
    #[serde(default)]
    pub attempts: u32,
    /// When a queued job is waiting for an automatic retry (ms since epoch).
    pub retry_at: Option<u64>,
}

impl Job {
    pub fn new(
        id: String,
        url: String,
        options: DownloadOptions,
        preset_name: Option<String>,
    ) -> Self {
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
            error_kind: None,
            error_hint: None,
            attempts: 0,
            retry_at: None,
        }
    }

    pub fn clear_error(&mut self) {
        self.error = None;
        self.error_kind = None;
        self.error_hint = None;
    }

    /// Reset runtime fields so the job can run again from the user's Retry button.
    pub fn reset_for_retry(&mut self) {
        self.status = JobStatus::Queued;
        self.stage = None;
        self.progress = Progress::default();
        self.output_path = None;
        self.started_at = None;
        self.finished_at = None;
        self.attempts = 0;
        self.retry_at = None;
        self.clear_error();
    }

    /// Whether the scheduler may start this job now.
    pub fn is_ready(&self, now: u64) -> bool {
        self.status == JobStatus::Queued && self.retry_at.is_none_or(|t| t <= now)
    }
}

pub fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Move the job `id` to `to` (clamped) within `jobs`. Returns false if `id` is unknown.
pub fn move_job(jobs: &mut Vec<Job>, id: &str, to: usize) -> bool {
    let Some(from) = jobs.iter().position(|j| j.id == id) else {
        return false;
    };
    let job = jobs.remove(from);
    let to = to.min(jobs.len());
    jobs.insert(to, job);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jobs(ids: &[&str]) -> Vec<Job> {
        ids.iter()
            .map(|id| {
                Job::new(
                    id.to_string(),
                    "https://x".into(),
                    DownloadOptions::default(),
                    None,
                )
            })
            .collect()
    }
    fn order(v: &[Job]) -> Vec<&str> {
        v.iter().map(|j| j.id.as_str()).collect()
    }

    #[test]
    fn moves_jobs() {
        let mut v = jobs(&["a", "b", "c", "d"]);
        assert!(move_job(&mut v, "d", 1));
        assert_eq!(order(&v), ["a", "d", "b", "c"]);
        assert!(move_job(&mut v, "a", 99));
        assert_eq!(order(&v), ["d", "b", "c", "a"]);
        assert!(move_job(&mut v, "c", 0));
        assert_eq!(order(&v), ["c", "d", "b", "a"]);
        assert!(!move_job(&mut v, "zz", 0));
    }

    #[test]
    fn readiness_respects_retry_time() {
        let mut j = jobs(&["a"]).remove(0);
        assert!(j.is_ready(100));
        j.retry_at = Some(200);
        assert!(!j.is_ready(100));
        assert!(j.is_ready(200));
        j.status = JobStatus::Paused;
        assert!(!j.is_ready(300));
    }
}
