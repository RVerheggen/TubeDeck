//! Runs one yt-dlp process for a job and streams its output into the engine.

use super::{
    Engine,
    job::JobStatus,
    progress::{self, Line, Tracker},
    winjob::ProcessTree,
};
use crate::{args, binaries, commands::AppState};
use std::{collections::VecDeque, path::Path, process::Stdio, sync::Arc};
use tauri::Manager;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
};

const STDERR_TAIL: usize = 40;

/// Arguments the engine adds on top of the user's options. They come after the user's
/// arguments so they win if a custom argument conflicts.
pub fn engine_args(temp_dir: &Path, ffmpeg: Option<&Path>, deno: Option<&Path>) -> Vec<String> {
    let mut a: Vec<String> = [
        "--newline",
        "--progress",
        "--progress-delta",
        "0.5",
        "--no-simulate",
        "--color",
        "never",
        "--progress-template",
        concat!(
            "download:TD_PROG %(info.format_id)s ",
            "%(progress.{status,downloaded_bytes,total_bytes,total_bytes_estimate,speed,eta,fragment_index,fragment_count})j"
        ),
        "--progress-template",
        "postprocess:TD_PP %(progress.postprocessor)s|%(progress.status)s",
        "--print",
        "before_dl:TD_INFO %(.{id,title,format_id,thumbnail,playlist_index,n_entries})j",
        "--print",
        "after_move:TD_FILE %(filepath)s",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    a.push("-P".into());
    a.push(format!("temp:{}", temp_dir.display()));
    if let Some(f) = ffmpeg {
        a.push("--ffmpeg-location".into());
        a.push(f.display().to_string());
    }
    if let Some(d) = deno {
        a.push("--js-runtimes".into());
        a.push(format!("deno:{}", d.display()));
    }
    a
}

struct Prepared {
    program: std::path::PathBuf,
    args: Vec<String>,
    display: String,
    warnings: Vec<String>,
}

fn prepare(engine: &Engine, id: &str) -> Result<Prepared, String> {
    let job = engine.job(id).ok_or("Job disappeared")?;
    let state = engine.app.state::<AppState>();
    let (folder, y_cfg, f_cfg, d_cfg) = {
        let s = state.settings.lock().unwrap();
        (s.download_folder.clone(), s.ytdlp_path.clone(), s.ffmpeg_path.clone(), s.deno_path.clone())
    };
    let bin = &state.managed_bin_dir;

    let (program, _) = binaries::resolve(&binaries::YTDLP, &y_cfg, bin)
        .filter(|(p, _)| p.is_file())
        .ok_or("yt-dlp was not found. Install it or set its path in Settings → Engine.")?;
    let ffmpeg = binaries::resolve(&binaries::FFMPEG, &f_cfg, bin).map(|(p, _)| p).filter(|p| p.is_file());
    let deno = binaries::resolve(&binaries::DENO, &d_cfg, bin).map(|(p, _)| p).filter(|p| p.is_file());

    let temp_dir = engine.temp_dir(id);
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Could not create temp folder: {e}"))?;

    let mut warnings = Vec::new();
    let mut a = args::user_args(&job.options, &folder, &mut warnings);
    a.extend(engine_args(&temp_dir, ffmpeg.as_deref(), deno.as_deref()));
    a.push("--".into());
    a.push(job.url.clone());

    let display = args::powershell_display(&program.display().to_string(), &a);
    let mut warnings: Vec<String> = warnings.into_iter().map(|w| format!("[TubeDeck] {:?}: {}", w.level, w.message)).collect();
    if ffmpeg.is_none() {
        warnings.push("[TubeDeck] FFmpeg not found: merging and conversion will fail.".into());
    }
    if deno.is_none() {
        warnings.push("[TubeDeck] Deno not found: YouTube may offer fewer formats.".into());
    }
    Ok(Prepared { program, args: a, display, warnings })
}

pub async fn run(engine: Arc<Engine>, id: String) {
    let prepared = match prepare(&engine, &id) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Job {id} could not start: {e}");
            engine.push_log(&id, format!("[TubeDeck] {e}"));
            engine.finish(&id, |job, _| {
                job.status = JobStatus::Failed;
                job.stage = None;
                job.error = Some(e);
            });
            return;
        }
    };

    engine.push_log(&id, format!("[TubeDeck] {}", prepared.display));
    for w in &prepared.warnings {
        engine.push_log(&id, w.clone());
    }

    let mut cmd = Command::new(&prepared.program);
    cmd.args(&prepared.args)
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(binaries::CREATE_NO_WINDOW);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Could not start yt-dlp: {e}");
            log::error!("Job {id}: {msg}");
            engine.push_log(&id, format!("[TubeDeck] {msg}"));
            engine.finish(&id, |job, _| {
                job.status = JobStatus::Failed;
                job.stage = None;
                job.error = Some(msg);
            });
            return;
        }
    };
    log::info!("Job {id} started (pid {:?})", child.id());

    #[cfg(windows)]
    {
        if let Some(handle) = child.raw_handle() {
            match ProcessTree::adopt(handle) {
                Ok(tree) => engine.attach_tree(&id, Arc::new(tree)),
                Err(e) => log::warn!("Job {id}: could not create job object: {e}"),
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(tree) = ProcessTree::adopt(std::ptr::null_mut()) {
            engine.attach_tree(&id, Arc::new(tree));
        }
    }

    engine.update(&id, |job| {
        job.status = JobStatus::Downloading;
        job.stage = Some("Fetching video info".into());
    });

    // yt-dlp prints download progress to stdout but postprocess progress and errors to
    // stderr, so both streams feed one ordered channel and go through the same parser.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(bool, String)>();
    for (is_err, reader) in [
        (false, Box::new(child.stdout.take().expect("stdout piped")) as Box<dyn AsyncRead + Unpin + Send>),
        (true, Box::new(child.stderr.take().expect("stderr piped"))),
    ] {
        let tx = tx.clone();
        tauri::async_runtime::spawn(async move {
            read_lines(reader, |line| {
                let _ = tx.send((is_err, line));
            })
            .await;
        });
    }
    drop(tx);

    let mut tracker = Tracker::default();
    let mut output: Option<String> = None;
    let mut stderr_tail: VecDeque<String> = VecDeque::new();
    while let Some((is_err, line)) = rx.recv().await {
        let parsed = progress::parse_line(&line);
        if is_err && let Line::Other(text) = &parsed {
            if stderr_tail.len() >= STDERR_TAIL {
                stderr_tail.pop_front();
            }
            stderr_tail.push_back(text.clone());
        }
        handle_line(&engine, &id, &mut tracker, &mut output, parsed);
    }

    let status = child.wait().await;
    let tail: Vec<String> = stderr_tail.into();

    let temp_dir = engine.temp_dir(&id);
    let mut outcome = String::new();
    engine.finish(&id, |job, canceled| {
        job.progress.speed = None;
        job.progress.eta = None;
        job.stage = None;
        match &status {
            _ if canceled => {
                job.status = JobStatus::Canceled;
            }
            Ok(s) if s.success() => {
                job.status = JobStatus::Completed;
                job.progress.percent = Some(100.0);
                if output.is_some() {
                    job.output_path = output.clone();
                }
            }
            Ok(s) => {
                job.status = JobStatus::Failed;
                job.error = Some(
                    progress::error_message(&tail)
                        .unwrap_or_else(|| format!("yt-dlp exited with code {}", s.code().unwrap_or(-1))),
                );
            }
            Err(e) => {
                job.status = JobStatus::Failed;
                job.error = Some(format!("Lost track of yt-dlp: {e}"));
            }
        }
        outcome = format!("{:?}{}", job.status, job.error.as_deref().map(|e| format!(": {e}")).unwrap_or_default());
    });
    log::info!("Job {id} finished: {outcome}");
    engine.push_log(&id, format!("[TubeDeck] Finished: {outcome}"));

    // Partial files are kept after a failure so Retry can resume them.
    let keep_partials = engine.job(&id).is_some_and(|j| j.status == JobStatus::Failed);
    if !keep_partials {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

fn handle_line(engine: &Engine, id: &str, tracker: &mut Tracker, output: &mut Option<String>, line: Line) {
    match line {
        Line::Info(info) => {
            tracker.on_info(&info);
            let stage = tracker.stage();
            engine.update(id, |job| {
                if info.title.is_some() {
                    job.title = info.title.clone();
                }
                if info.thumbnail.is_some() {
                    job.thumbnail = info.thumbnail.clone();
                }
                job.status = JobStatus::Downloading;
                job.stage = stage.or(Some("Downloading".into()));
            });
            engine.push_log(id, format!("[TubeDeck] Downloading {}", info.title.as_deref().unwrap_or("item")));
        }
        Line::Progress { format_id, data } => {
            let (p, stage) = tracker.on_progress(&format_id, &data);
            engine.update(id, |job| {
                job.status = JobStatus::Downloading;
                job.progress = p;
                job.stage = stage.or(Some("Downloading".into()));
            });
        }
        Line::PostProcess { name, status } => {
            if status == "started" {
                engine.push_log(id, format!("[TubeDeck] Post-processing: {name}"));
                engine.update(id, |job| {
                    job.status = JobStatus::Processing;
                    job.stage = Some(progress::postprocessor_label(&name));
                    job.progress.speed = None;
                    job.progress.eta = None;
                });
            }
        }
        Line::File(path) => {
            engine.push_log(id, format!("[TubeDeck] Saved {path}"));
            *output = Some(path.clone());
            engine.update(id, |job| job.output_path = Some(path));
        }
        Line::Other(text) => {
            if !text.trim().is_empty() {
                engine.push_log(id, text);
            }
        }
    }
}

/// Read `\n`-separated lines, tolerating invalid UTF-8 and `\r\n`.
async fn read_lines<R: AsyncRead + Unpin>(reader: R, mut on_line: impl FnMut(String)) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::with_capacity(512);
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf);
                on_line(line.trim_end_matches(['\r', '\n']).to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_args_include_templates_and_binaries() {
        let a = engine_args(Path::new(r"C:\tmp\job1"), Some(Path::new(r"C:\bin\ffmpeg.exe")), Some(Path::new(r"C:\bin\deno.exe")));
        assert!(a.contains(&"--no-simulate".to_string()));
        assert!(a.iter().any(|x| x.starts_with("download:TD_PROG ")));
        assert!(a.iter().any(|x| x.starts_with("postprocess:TD_PP ")));
        assert!(a.iter().any(|x| x.starts_with("before_dl:TD_INFO ")));
        assert!(a.iter().any(|x| x.starts_with("after_move:TD_FILE ")));
        assert!(a.contains(&r"temp:C:\tmp\job1".to_string()));
        assert!(a.contains(&r"deno:C:\bin\deno.exe".to_string()));
        let i = a.iter().position(|x| x == "--ffmpeg-location").unwrap();
        assert_eq!(a[i + 1], r"C:\bin\ffmpeg.exe");
    }

    #[test]
    fn engine_args_without_optional_binaries() {
        let a = engine_args(Path::new("t"), None, None);
        assert!(!a.contains(&"--ffmpeg-location".to_string()));
        assert!(!a.contains(&"--js-runtimes".to_string()));
    }
}
