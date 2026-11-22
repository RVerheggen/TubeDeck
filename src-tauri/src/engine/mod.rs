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
//!
//! Persistence: every UI batch is also written to SQLite, so the queue survives restarts.
//! Finished jobs stay in the queue until cleared, then live on in history (`archived`).
//! Each job's log is kept in memory and written to `logs/jobs/<id>.log` whenever the job
//! stops (finish, pause, failure) and on shutdown.

pub mod errors;
pub mod job;
pub mod progress;
mod runner;
mod winjob;

use crate::{args, commands::AppState, db::Db, options::DownloadOptions};
use job::{Job, JobStatus, now_ms};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    process::ExitStatus,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
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
    job_log_dir: PathBuf,
    db: Arc<Db>,
    seq: AtomicU64,
    shutting_down: AtomicBool,
}

/// Bring jobs from the previous session back into a consistent state.
/// Returns the jobs to keep in the queue and the ids of finished jobs to archive.
pub(crate) fn recover(jobs: Vec<Job>, resume: bool) -> (Vec<Job>, Vec<String>) {
    let (finished, mut queue): (Vec<Job>, Vec<Job>) =
        jobs.into_iter().partition(|j| j.status.is_finished());
    for j in &mut queue {
        j.progress.speed = None;
        j.progress.eta = None;
        j.retry_at = None;
        match j.status {
            JobStatus::Starting | JobStatus::Downloading | JobStatus::Processing if resume => {
                j.status = JobStatus::Queued;
                j.stage = Some("Resuming after restart".into());
            }
            JobStatus::Starting | JobStatus::Downloading | JobStatus::Processing => {
                j.status = JobStatus::Paused;
                j.stage = Some("Interrupted when TubeDeck closed".into());
            }
            JobStatus::Queued if !resume => {
                j.status = JobStatus::Paused;
                j.stage = Some("Paused when TubeDeck closed".into());
            }
            _ => {}
        }
    }
    (queue, finished.into_iter().map(|j| j.id).collect())
}

impl Engine {
    pub fn new(
        app: AppHandle,
        temp_root: PathBuf,
        job_log_dir: PathBuf,
        db: Arc<Db>,
        resume: bool,
    ) -> Arc<Self> {
        let stored = db.load_queue().unwrap_or_else(|e| {
            log::error!("Could not load the saved queue: {e}");
            Vec::new()
        });
        let (jobs, finished) = recover(stored, resume);
        if let Err(e) = db.archive_jobs(&finished) {
            log::warn!("Could not move finished jobs to history: {e}");
        }
        let positioned: Vec<(Job, usize)> = jobs
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, j)| (j, i))
            .collect();
        if let Err(e) = db.upsert_jobs(&positioned) {
            log::warn!("Could not save the recovered queue: {e}");
        }
        log::info!(
            "Restored {} queued job(s); {} finished job(s) moved to history",
            jobs.len(),
            finished.len()
        );

        // Partial downloads are only useful for jobs that can still resume.
        let keep: HashSet<&str> = jobs.iter().map(|j| j.id.as_str()).collect();
        if let Ok(entries) = std::fs::read_dir(&temp_root) {
            for entry in entries.flatten() {
                if !keep.contains(entry.file_name().to_string_lossy().as_ref()) {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }

        let mut logs = HashMap::new();
        for j in &jobs {
            if let Ok(text) = std::fs::read_to_string(job_log_dir.join(format!("{}.log", j.id))) {
                logs.insert(
                    j.id.clone(),
                    text.lines().map(String::from).collect::<VecDeque<_>>(),
                );
            }
        }

        let engine = Arc::new(Self {
            app,
            inner: Mutex::new(Inner {
                jobs,
                logs,
                ..Default::default()
            }),
            temp_root,
            job_log_dir,
            db,
            seq: AtomicU64::new(0),
            shutting_down: AtomicBool::new(false),
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

    /// Send changed jobs to the UI and write them to the database (one transaction).
    fn flush(&self) {
        let batch: Vec<(Job, usize)> = {
            let mut inner = self.lock();
            if inner.dirty.is_empty() {
                return;
            }
            let dirty = std::mem::take(&mut inner.dirty);
            inner
                .jobs
                .iter()
                .enumerate()
                .filter(|(_, j)| dirty.contains(&j.id))
                .map(|(i, j)| (j.clone(), i))
                .collect()
        };
        if batch.is_empty() {
            return;
        }
        if let Err(e) = self.db.upsert_jobs(&batch) {
            log::error!("Could not save job updates: {e}");
        }
        let jobs: Vec<Job> = batch.into_iter().map(|(j, _)| j).collect();
        let _ = self.app.emit(EVENT_UPDATED, jobs);
    }

    fn log_path(&self, id: &str) -> PathBuf {
        self.job_log_dir.join(format!("{id}.log"))
    }

    /// Write a job's in-memory log to its file (overwriting; the buffer holds the full log).
    fn write_log(&self, id: &str) {
        let text = {
            let inner = self.lock();
            match inner.logs.get(id) {
                Some(lines) => {
                    lines
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join("\n")
                        + "\n"
                }
                None => return,
            }
        };
        if let Err(e) = std::fs::create_dir_all(&self.job_log_dir)
            .and_then(|_| std::fs::write(self.log_path(id), text))
        {
            log::warn!("Could not write log for job {id}: {e}");
        }
    }

    fn delete_log(&self, id: &str) {
        let _ = std::fs::remove_file(self.log_path(id));
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

    /// A job's log: from memory while it's in the queue, else from its file (history).
    pub fn log(&self, id: &str) -> Vec<String> {
        if let Some(l) = self.lock().logs.get(id) {
            return l.iter().cloned().collect();
        }
        std::fs::read_to_string(self.log_path(id))
            .map(|t| t.lines().map(String::from).collect())
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
        // During shutdown the job was already saved as interrupted; don't overwrite that
        // with the "failed" exit caused by killing the process.
        if self.shutting_down.load(Ordering::SeqCst) {
            return;
        }
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
        self.write_log(id);
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
        if let Err(e) = self.db.set_positions(&order) {
            log::warn!("Could not save queue order: {e}");
        }
        let _ = self.app.emit(EVENT_ORDER, order);
        Ok(())
    }

    /// Take jobs out of the queue. Finished jobs move to history; jobs that never finished
    /// (queued, paused) are deleted along with their partial files and log.
    pub fn remove(&self, ids: &[String]) -> Result<(), String> {
        let (archive, delete): (Vec<String>, Vec<String>) = {
            let mut inner = self.lock();
            if let Some(id) = ids.iter().find(|id| inner.running.contains_key(*id)) {
                return Err(format!("Job {id} is still running; cancel it first"));
            }
            let set: HashSet<&String> = ids.iter().collect();
            let (finished, unfinished): (Vec<&Job>, Vec<&Job>) = inner
                .jobs
                .iter()
                .filter(|j| set.contains(&j.id))
                .partition(|j| j.status.is_finished());
            let split = (
                finished.iter().map(|j| j.id.clone()).collect(),
                unfinished.iter().map(|j| j.id.clone()).collect(),
            );
            inner.jobs.retain(|j| !set.contains(&j.id));
            for id in ids {
                inner.logs.remove(id);
                inner.dirty.remove(id);
            }
            split
        };
        self.db.archive_jobs(&archive)?;
        self.db.delete_jobs(&delete)?;
        for id in ids {
            self.remove_temp(id);
        }
        for id in &delete {
            self.delete_log(id);
        }
        if !ids.is_empty() {
            let _ = self.app.emit(EVENT_REMOVED, ids.to_vec());
        }
        Ok(())
    }

    /// Move all finished jobs from the queue to history.
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

    /// Permanently delete finished jobs (from the queue, history and their logs).
    pub fn delete_history(&self, ids: &[String]) -> Result<(), String> {
        let in_queue: Vec<String> = {
            let inner = self.lock();
            let set: HashSet<&String> = ids.iter().collect();
            if inner
                .jobs
                .iter()
                .any(|j| set.contains(&j.id) && !j.status.is_finished())
            {
                return Err("Only finished downloads can be deleted from history".into());
            }
            inner
                .jobs
                .iter()
                .filter(|j| set.contains(&j.id))
                .map(|j| j.id.clone())
                .collect()
        };
        if !in_queue.is_empty() {
            self.remove(&in_queue)?;
        }
        self.db.delete_jobs(ids)?;
        for id in ids {
            self.delete_log(id);
        }
        Ok(())
    }

    /// Delete every finished job, in the queue and in history.
    pub fn clear_history(&self) -> Result<(), String> {
        let mut ids = self.db.archived_finished_ids()?;
        ids.extend(
            self.lock()
                .jobs
                .iter()
                .filter(|j| j.status.is_finished())
                .map(|j| j.id.clone()),
        );
        self.delete_history(&ids)
    }

    /// App exit: kill every process, save the queue and write all logs.
    ///
    /// Running jobs keep their running status in the database on purpose: startup recovery
    /// (`recover`) then treats a normal quit exactly like a crash and resumes them or
    /// pauses them according to the "resume on startup" setting.
    pub fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        let (snapshot, ids) = {
            let mut inner = self.lock();
            let running: Vec<String> = inner.running.keys().cloned().collect();
            for r in inner.running.values() {
                if let Some(tree) = &r.tree {
                    let _ = tree.kill();
                }
            }
            for id in &running {
                if let Ok(job) = inner.job_mut(id) {
                    job.stage = Some("Interrupted when TubeDeck closed".into());
                    job.progress.speed = None;
                    job.progress.eta = None;
                }
            }
            let snapshot: Vec<(Job, usize)> = inner
                .jobs
                .iter()
                .cloned()
                .enumerate()
                .map(|(i, j)| (j, i))
                .collect();
            (snapshot, inner.logs.keys().cloned().collect::<Vec<_>>())
        };
        if let Err(e) = self.db.upsert_jobs(&snapshot) {
            log::error!("Could not save the queue on exit: {e}");
        }
        for id in ids {
            self.write_log(&id);
        }
        log::info!("Engine shut down; {} job(s) saved", snapshot.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, status: JobStatus) -> Job {
        let mut j = Job::new(
            id.into(),
            "https://x".into(),
            DownloadOptions::default(),
            None,
        );
        j.status = status;
        j.progress.speed = Some(1.0);
        j.retry_at = Some(5);
        j
    }

    #[test]
    fn recover_with_resume() {
        let (queue, finished) = recover(
            vec![
                job("run", JobStatus::Downloading),
                job("done", JobStatus::Completed),
                job("q", JobStatus::Queued),
                job("p", JobStatus::Paused),
                job("fail", JobStatus::Failed),
            ],
            true,
        );
        assert_eq!(finished, ["done", "fail"]);
        let st: Vec<_> = queue.iter().map(|j| (j.id.as_str(), j.status)).collect();
        assert_eq!(
            st,
            [
                ("run", JobStatus::Queued),
                ("q", JobStatus::Queued),
                ("p", JobStatus::Paused)
            ]
        );
        assert!(
            queue
                .iter()
                .all(|j| j.progress.speed.is_none() && j.retry_at.is_none())
        );
    }

    #[test]
    fn recover_without_resume_pauses_everything() {
        let (queue, _) = recover(
            vec![
                job("run", JobStatus::Processing),
                job("q", JobStatus::Queued),
            ],
            false,
        );
        assert!(queue.iter().all(|j| j.status == JobStatus::Paused));
        assert!(queue[0].stage.as_deref().unwrap().contains("Interrupted"));
    }
}

#[cfg(test)]
mod shutdown_contract {
    use super::*;

    /// Regression: a graceful quit must leave running jobs in a state that `recover`
    /// resumes. Shutdown keeps the running status; recovery decides from the setting.
    #[test]
    fn running_status_from_shutdown_is_resumed_or_paused() {
        let mut j = Job::new(
            "r".into(),
            "https://x".into(),
            DownloadOptions::default(),
            None,
        );
        j.status = JobStatus::Downloading;
        j.stage = Some("Interrupted when TubeDeck closed".into());
        let (q, _) = recover(vec![j.clone()], true);
        assert_eq!(q[0].status, JobStatus::Queued);
        let (q, _) = recover(vec![j], false);
        assert_eq!(q[0].status, JobStatus::Paused);
    }
}
