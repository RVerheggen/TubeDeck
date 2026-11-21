//! Download engine: owns the job queue, starts yt-dlp processes up to the configured
//! concurrency, and pushes batched job updates to the UI.
//!
//! Events:
//! - `jobs://updated`: `Job[]` with every job that changed since the last batch (≤ 4/s)
//! - `jobs://removed`: `string[]` ids removed from the queue

pub mod job;
pub mod progress;
mod runner;
mod winjob;

use crate::{args, commands::AppState, options::DownloadOptions};
use job::{Job, JobStatus, now_ms};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
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

const FLUSH_INTERVAL: Duration = Duration::from_millis(250);
const MAX_LOG_LINES: usize = 2000;

#[derive(Default)]
struct Running {
    tree: Option<Arc<ProcessTree>>,
    cancel_requested: bool,
}

#[derive(Default)]
struct Inner {
    jobs: Vec<Job>,
    running: HashMap<String, Running>,
    dirty: HashSet<String>,
    logs: HashMap<String, VecDeque<String>>,
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
        let engine = Arc::new(Self { app, inner: Mutex::new(Inner::default()), temp_root, seq: AtomicU64::new(0) });
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
            inner.jobs.iter().filter(|j| dirty.contains(&j.id)).cloned().collect()
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

    // ---- Queries --------------------------------------------------------------------

    pub fn list(&self) -> Vec<Job> {
        self.lock().jobs.clone()
    }

    pub fn job(&self, id: &str) -> Option<Job> {
        self.lock().jobs.iter().find(|j| j.id == id).cloned()
    }

    pub fn log(&self, id: &str) -> Vec<String> {
        self.lock().logs.get(id).map(|l| l.iter().cloned().collect()).unwrap_or_default()
    }

    // ---- Mutations used by the runner ------------------------------------------------

    /// Apply `f` to a job and schedule it for the next UI batch.
    pub(crate) fn update(&self, id: &str, f: impl FnOnce(&mut Job)) {
        let mut inner = self.lock();
        if let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) {
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

    /// Register the process tree; kills it immediately if cancel was already requested.
    pub(crate) fn attach_tree(&self, id: &str, tree: Arc<ProcessTree>) {
        let mut inner = self.lock();
        if let Some(r) = inner.running.get_mut(id) {
            if r.cancel_requested {
                let _ = tree.kill();
            }
            r.tree = Some(tree);
        }
    }

    /// Called by the runner when the process has exited. Returns whether cancel was requested.
    pub(crate) fn finish(self: &Arc<Self>, id: &str, f: impl FnOnce(&mut Job, bool)) {
        {
            let mut inner = self.lock();
            let canceled = inner.running.remove(id).is_some_and(|r| r.cancel_requested);
            if let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) {
                f(job, canceled);
                job.finished_at = Some(now_ms());
                inner.dirty.insert(id.to_string());
            }
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
            created.push(Job::new(self.new_id(), url, options.clone(), preset_name.clone()));
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

    /// Start queued jobs until the concurrency limit is reached. Reads the limit from the
    /// current settings every time, so changing it takes effect without a restart.
    pub fn schedule(self: &Arc<Self>) {
        let max = self.app.state::<AppState>().settings.lock().unwrap().max_concurrent.max(1) as usize;
        let to_start: Vec<String> = {
            let mut inner = self.lock();
            let free = max.saturating_sub(inner.running.len());
            let mut ids = Vec::new();
            let now = now_ms();
            let Inner { jobs, running: run_map, dirty, .. } = &mut *inner;
            for job in jobs.iter_mut().filter(|j| j.status == JobStatus::Queued).take(free) {
                job.status = JobStatus::Starting;
                job.started_at = Some(now);
                job.stage = Some("Starting yt-dlp".into());
                run_map.insert(job.id.clone(), Running::default());
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
        let mut inner = self.lock();
        if let Some(r) = inner.running.get_mut(id) {
            r.cancel_requested = true;
            if let Some(tree) = &r.tree {
                tree.kill().map_err(|e| format!("Could not stop yt-dlp: {e}"))?;
            }
            log::info!("Cancel requested for job {id}");
            return Ok(());
        }
        let job = inner.jobs.iter_mut().find(|j| j.id == id).ok_or("Job not found")?;
        if job.status != JobStatus::Queued {
            return Err("Only queued or running jobs can be canceled".into());
        }
        job.status = JobStatus::Canceled;
        job.finished_at = Some(now_ms());
        job.stage = None;
        inner.dirty.insert(id.to_string());
        Ok(())
    }

    pub fn retry(self: &Arc<Self>, id: &str) -> Result<(), String> {
        {
            let mut inner = self.lock();
            let job = inner.jobs.iter_mut().find(|j| j.id == id).ok_or("Job not found")?;
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

    pub fn remove(&self, ids: &[String]) -> Result<(), String> {
        let removed: Vec<String> = {
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
            ids.to_vec()
        };
        for id in &removed {
            let _ = std::fs::remove_dir_all(self.temp_dir(id));
        }
        if !removed.is_empty() {
            let _ = self.app.emit(EVENT_REMOVED, removed);
        }
        Ok(())
    }

    pub fn clear_finished(&self) -> Result<(), String> {
        let ids: Vec<String> =
            self.lock().jobs.iter().filter(|j| j.status.is_finished()).map(|j| j.id.clone()).collect();
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
