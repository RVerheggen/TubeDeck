//! Download engine: owns the job queue, starts yt-dlp processes up to the configured
//! concurrency, and pushes batched job updates to the UI.
//!
//! Events:
//! - `jobs://updated`: `Job[]` with every job that changed since the last batch (≤ 4/s)
//! - `jobs://removed`: `string[]` ids removed from the queue
//! - `jobs://order`: `string[]` all job ids in queue order (after a reorder)
//!
//! The order of `Inner::jobs` is the queue priority: the scheduler starts the first ready
//! queued job. Pausing kills the process tree but keeps the job's temp folder, so resuming
//! re-runs the same command and yt-dlp continues from the partial files.

pub mod errors;
pub mod job;
pub mod progress;
mod runner;
mod winjob;

use crate::{args, commands::AppState, options::DownloadOptions};
use job::{Job, JobStatus, now_ms};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    process::ExitStatus,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use winjob::ProcessTree;

pub const EVENT_UPDATED: &str = "jobs://updated";
pub const EVENT_REMOVED: &str = "jobs://removed";
pub const EVENT_ORDER: &str = "jobs://order";

const FLUSH_INTERVAL: Duration = Duration::from_millis(250);
const MAX_LOG_LINES: usize = 2000;

/// Why the engine is stopping a running process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Cancel,
    Pause,
}

#[derive(Default)]
struct Running {
    tree: Option<Arc<ProcessTree>>,
    stop: Option<Stop>,
}

#[derive(Default)]
struct Inner {
    jobs: Vec<Job>,
    running: HashMap<String, Running>,
    dirty: HashSet<String>,
    logs: HashMap<String, VecDeque<String>>,
}

impl Inner {
    fn job_mut(&mut self, id: &str) -> Result<&mut Job, String> {
        self.jobs
            .iter_mut()
            .find(|j| j.id == id)
            .ok_or_else(|| "Job not found".to_string())
    }

    /// Ask a running job's process tree to stop. Returns false if it isn't running.
    fn stop_running(&mut self, id: &str, why: Stop) -> Result<bool, String> {
        let Some(r) = self.running.get_mut(id) else {
            return Ok(false);
        };
        // A cancel wins over an earlier pause request, never the other way round.
        if r.stop != Some(Stop::Cancel) {
            r.stop = Some(why);
        }
        if let Some(tree) = &r.tree {
            tree.kill()
                .map_err(|e| format!("Could not stop yt-dlp: {e}"))?;
        }
        Ok(true)
    }
}

/// What the runner reports when yt-dlp has exited (or never started).
pub(crate) struct Outcome {
    pub exit: Result<ExitStatus, String>,
    pub output: Option<String>,
    pub stderr_tail: Vec<String>,
}

pub struct Engine {
    app: AppHandle,
    inner: Mutex<Inner>,
    temp_root: PathBuf,
    seq: AtomicU64,
}

impl Engine {
    pub fn new(app: AppHandle, temp_root: PathBuf) -> Arc<Self> {
        // The queue isn't persisted yet (M4), so partial downloads from a previous session
        // can never be resumed; clear them instead of letting them pile up.
        if temp_root.exists() {
            match std::fs::remove_dir_all(&temp_root) {
                Ok(()) => log::info!("Cleared leftover temp files in {}", temp_root.display()),
                Err(e) => log::warn!("Could not clear {}: {e}", temp_root.display()),
            }
        }
        let engine = Arc::new(Self {
            app,
            inner: Mutex::new(Inner::default()),
            temp_root,
            seq: AtomicU64::new(0),
        });
        let weak = Arc::downgrade(&engine);
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(FLUSH_INTERVAL).await;
                let Some(engine) = weak.upgrade() else { break };
                engine.flush();
            }
        });
        engine
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A panic while holding the lock must not take the whole engine down.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn flush(&self) {
        let batch: Vec<Job> = {
            let mut inner = self.lock();
            if inner.dirty.is_empty() {
                return;
            }
            let dirty = std::mem::take(&mut inner.dirty);
            inner
                .jobs
                .iter()
                .filter(|j| dirty.contains(&j.id))
                .cloned()
                .collect()
        };
        if !batch.is_empty() {
            let _ = self.app.emit(EVENT_UPDATED, batch);
        }
    }

    fn new_id(&self) -> String {
        let n = self.seq.fetch_add(1, Ordering::Relaxed);
        format!("{:x}-{n:x}", now_ms())
    }

    pub fn temp_dir(&self, id: &str) -> PathBuf {
        self.temp_root.join(id)
    }

    fn remove_temp(&self, id: &str) {
        let _ = std::fs::remove_dir_all(self.temp_dir(id));
    }

    fn settings(&self) -> (usize, u32) {
        let s = self.app.state::<AppState>();
        let s = s.settings.lock().unwrap();
        (s.max_concurrent.max(1) as usize, s.auto_retry)
    }

    // ---- Queries --------------------------------------------------------------------

    pub fn list(&self) -> Vec<Job> {
        self.lock().jobs.clone()
    }

    pub fn job(&self, id: &str) -> Option<Job> {
        self.lock().jobs.iter().find(|j| j.id == id).cloned()
    }

    pub fn log(&self, id: &str) -> Vec<String> {
        self.lock()
            .logs
            .get(id)
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default()
    }

    // ---- Used by the runner ---------------------------------------------------------

    /// Apply `f` to a job and schedule it for the next UI batch.
    pub(crate) fn update(&self, id: &str, f: impl FnOnce(&mut Job)) {
        let mut inner = self.lock();
        if let Ok(job) = inner.job_mut(id) {
            f(job);
            inner.dirty.insert(id.to_string());
        }
    }

    pub(crate) fn push_log(&self, id: &str, line: impl Into<String>) {
        let mut inner = self.lock();
        let log = inner.logs.entry(id.to_string()).or_default();
        if log.len() >= MAX_LOG_LINES {
            log.pop_front();
        }
        log.push_back(line.into());
    }

    /// Register the process tree; kills it at once if a stop was requested meanwhile.
    pub(crate) fn attach_tree(&self, id: &str, tree: Arc<ProcessTree>) {
        let mut inner = self.lock();
        if let Some(r) = inner.running.get_mut(id) {
            if r.stop.is_some() {
                let _ = tree.kill();
            }
            r.tree = Some(tree);
        }
    }

    /// The process has exited: decide the job's final (or next) state.
    pub(crate) fn complete(self: &Arc<Self>, id: &str, outcome: Outcome) {
        let (_, max_auto_retry) = self.settings();
        let mut delete_temp = false;
        let mut retry_in: Option<Duration> = None;
        let summary;
        {
            let mut inner = self.lock();
            let stop = inner.running.remove(id).and_then(|r| r.stop);
            let Ok(job) = inner.job_mut(id) else { return };
            job.progress.speed = None;
            job.progress.eta = None;
            job.stage = None;

            match (stop, &outcome.exit) {
                (Some(Stop::Cancel), _) => {
                    job.status = JobStatus::Canceled;
                    delete_temp = true;
                }
                (Some(Stop::Pause), _) => {
                    job.status = JobStatus::Paused;
                    job.stage = Some("Paused; partial download kept".into());
                }
                (None, Ok(s)) if s.success() => {
                    job.status = JobStatus::Completed;
                    job.progress.percent = Some(100.0);
                    job.clear_error();
                    if outcome.output.is_some() {
                        job.output_path = outcome.output.clone();
                    }
                    delete_temp = true;
                }
                (None, exit) => {
                    let message = match exit {
                        Ok(s) => {
                            progress::error_message(&outcome.stderr_tail).unwrap_or_else(|| {
                                format!("yt-dlp exited with code {}", s.code().unwrap_or(-1))
                            })
                        }
                        Err(e) => e.clone(),
                    };
                    let kind = errors::classify(&outcome.stderr_tail);
                    job.error = Some(message);
                    job.error_kind = Some(kind);
                    job.error_hint = kind.hint().map(String::from);

                    // Partial files stay so the retry (automatic or manual) can resume.
                    if let Some(delay) = errors::retry_delay(kind, job.attempts, max_auto_retry) {
                        job.attempts += 1;
                        job.status = JobStatus::Queued;
                        job.retry_at = Some(now_ms() + delay.as_millis() as u64);
                        job.stage = Some(format!("Retry {}/{max_auto_retry}", job.attempts));
                        retry_in = Some(delay);
                    } else {
                        job.status = JobStatus::Failed;
                    }
                }
            }
            if job.status.is_finished() {
                job.finished_at = Some(now_ms());
            }
            summary = format!(
                "{:?}{}{}",
                job.status,
                job.error
                    .as_deref()
                    .map(|e| format!(": {e}"))
                    .unwrap_or_default(),
                retry_in
                    .map(|d| format!(" (auto-retry in {}s)", d.as_secs()))
                    .unwrap_or_default(),
            );
            inner.dirty.insert(id.to_string());
        }

        log::info!("Job {id} finished: {summary}");
        self.push_log(id, format!("[TubeDeck] Finished: {summary}"));
        if delete_temp {
            self.remove_temp(id);
        }
        if let Some(delay) = retry_in {
            let engine = Arc::clone(self);
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(delay).await;
                engine.schedule();
            });
        }
        self.schedule();
    }

    // ---- Public API (Tauri commands) -------------------------------------------------

    pub fn enqueue(
        self: &Arc<Self>,
        urls: Vec<String>,
        options: DownloadOptions,
        preset_name: Option<String>,
    ) -> Result<Vec<Job>, String> {
        let mut created = Vec::new();
        for url in urls {
            let url = url.trim().to_string();
            args::validate_url(&url).map_err(|e| format!("{url}: {e}"))?;
            created.push(Job::new(
                self.new_id(),
                url,
                options.clone(),
                preset_name.clone(),
            ));
        }
        {
            let mut inner = self.lock();
            for job in &created {
                inner.dirty.insert(job.id.clone());
            }
            inner.jobs.extend(created.iter().cloned());
        }
        log::info!("Queued {} job(s)", created.len());
        self.schedule();
        Ok(created)
    }

    /// Start ready jobs in queue order until the concurrency limit is reached. Reads the
    /// limit from the current settings every time, so changes apply immediately.
    pub fn schedule(self: &Arc<Self>) {
        let (max, max_auto_retry) = self.settings();
        let to_start: Vec<String> = {
            let mut inner = self.lock();
            let free = max.saturating_sub(inner.running.len());
            let now = now_ms();
            let mut ids = Vec::new();
            let Inner {
                jobs,
                running,
                dirty,
                ..
            } = &mut *inner;
            for job in jobs.iter_mut().filter(|j| j.is_ready(now)).take(free) {
                job.status = JobStatus::Starting;
                job.started_at = Some(now);
                job.retry_at = None;
                job.stage = Some(if job.attempts > 0 {
                    format!("Retry {}/{max_auto_retry}: starting", job.attempts)
                } else {
                    "Starting yt-dlp".into()
                });
                running.insert(job.id.clone(), Running::default());
                dirty.insert(job.id.clone());
                ids.push(job.id.clone());
            }
            ids
        };
        for id in to_start {
            let engine = Arc::clone(self);
            tauri::async_runtime::spawn(async move { runner::run(engine, id).await });
        }
    }

    pub fn cancel(self: &Arc<Self>, id: &str) -> Result<(), String> {
        {
            let mut inner = self.lock();
            if inner.stop_running(id, Stop::Cancel)? {
                log::info!("Cancel requested for job {id}");
                return Ok(());
            }
            let job = inner.job_mut(id)?;
            if !matches!(job.status, JobStatus::Queued | JobStatus::Paused) {
                return Err("Only queued, paused or running jobs can be canceled".into());
            }
            job.status = JobStatus::Canceled;
            job.finished_at = Some(now_ms());
            job.stage = None;
            job.retry_at = None;
            inner.dirty.insert(id.to_string());
        }
        self.remove_temp(id);
        Ok(())
    }

    pub fn pause(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let mut inner = self.lock();
        if inner.stop_running(id, Stop::Pause)? {
            log::info!("Pause requested for job {id}");
            return Ok(());
        }
        let job = inner.job_mut(id)?;
        if job.status != JobStatus::Queued {
            return Err("Only queued or running jobs can be paused".into());
        }
        job.status = JobStatus::Paused;
        job.retry_at = None;
        job.stage = Some("Paused".into());
        inner.dirty.insert(id.to_string());
        Ok(())
    }

    pub fn resume(self: &Arc<Self>, id: &str) -> Result<(), String> {
        {
            let mut inner = self.lock();
            let job = inner.job_mut(id)?;
            if job.status != JobStatus::Paused {
                return Err("Only paused jobs can be resumed".into());
            }
            job.status = JobStatus::Queued;
            job.stage = None;
            inner.dirty.insert(id.to_string());
        }
        self.schedule();
        Ok(())
    }

    /// Pause every queued and running job.
    pub fn pause_all(self: &Arc<Self>) -> Result<(), String> {
        let ids: Vec<String> = self
            .lock()
            .jobs
            .iter()
            .filter(|j| {
                matches!(
                    j.status,
                    JobStatus::Queued
                        | JobStatus::Starting
                        | JobStatus::Downloading
                        | JobStatus::Processing
                )
            })
            .map(|j| j.id.clone())
            .collect();
        ids.iter().try_for_each(|id| self.pause(id))
    }

    pub fn resume_all(self: &Arc<Self>) -> Result<(), String> {
        {
            let mut inner = self.lock();
            let Inner { jobs, dirty, .. } = &mut *inner;
            for job in jobs.iter_mut().filter(|j| j.status == JobStatus::Paused) {
                job.status = JobStatus::Queued;
                job.stage = None;
                dirty.insert(job.id.clone());
            }
        }
        self.schedule();
        Ok(())
    }

    pub fn retry(self: &Arc<Self>, id: &str) -> Result<(), String> {
        {
            let mut inner = self.lock();
            let job = inner.job_mut(id)?;
            if !matches!(job.status, JobStatus::Failed | JobStatus::Canceled) {
                return Err("Only failed or canceled jobs can be retried".into());
            }
            job.reset_for_retry();
            inner.dirty.insert(id.to_string());
            inner.logs.remove(id);
        }
        self.schedule();
        Ok(())
    }

    /// Move a job to `to` (0 = top). Queue order decides which queued job starts next.
    pub fn move_to(self: &Arc<Self>, id: &str, to: usize) -> Result<(), String> {
        let order: Vec<String> = {
            let mut inner = self.lock();
            if !job::move_job(&mut inner.jobs, id, to) {
                return Err("Job not found".into());
            }
            inner.jobs.iter().map(|j| j.id.clone()).collect()
        };
        let _ = self.app.emit(EVENT_ORDER, order);
        Ok(())
    }

    pub fn remove(&self, ids: &[String]) -> Result<(), String> {
        {
            let mut inner = self.lock();
            if let Some(id) = ids.iter().find(|id| inner.running.contains_key(*id)) {
                return Err(format!("Job {id} is still running; cancel it first"));
            }
            let set: HashSet<&String> = ids.iter().collect();
            inner.jobs.retain(|j| !set.contains(&j.id));
            for id in ids {
                inner.logs.remove(id);
                inner.dirty.remove(id);
            }
        }
        for id in ids {
            self.remove_temp(id);
        }
        if !ids.is_empty() {
            let _ = self.app.emit(EVENT_REMOVED, ids.to_vec());
        }
        Ok(())
    }

    pub fn clear_finished(&self) -> Result<(), String> {
        let ids: Vec<String> = self
            .lock()
            .jobs
            .iter()
            .filter(|j| j.status.is_finished())
            .map(|j| j.id.clone())
            .collect();
        self.remove(&ids)
    }

    /// Kill every running process (used on app exit).
    pub fn shutdown(&self) {
        let inner = self.lock();
        for r in inner.running.values() {
            if let Some(tree) = &r.tree {
                let _ = tree.kill();
            }
        }
    }
}
