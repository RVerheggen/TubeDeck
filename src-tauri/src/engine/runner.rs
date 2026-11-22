//! Runs one yt-dlp process for a job and streams its output into the engine.

use super::{
    Engine, Outcome,
    job::JobStatus,
    progress::{self, Line, Tracker},
    winjob::ProcessTree,
};
use crate::{args, binaries, commands::AppState};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};
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
        // yt-dlp.exe ignores PYTHONIOENCODING/PYTHONUTF8 and writes piped output in the
        // ANSI code page, which garbles titles like "A – B" in printed file paths.
        "--encoding",
        "utf-8",
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

/// Resolved programs for running yt-dlp.
pub(crate) struct Tools {
    pub ytdlp: PathBuf,
    pub ffmpeg: Option<PathBuf>,
    pub deno: Option<PathBuf>,
}

/// Find yt-dlp (required), FFmpeg and Deno using the configured paths, the managed folder
/// and PATH (see `binaries::resolve`).
pub(crate) fn tools(state: &AppState) -> Result<Tools, String> {
    let (y_cfg, f_cfg, d_cfg) = {
        let s = state.settings.lock().unwrap();
        (
            s.ytdlp_path.clone(),
            s.ffmpeg_path.clone(),
            s.deno_path.clone(),
        )
    };
    let bin = &state.managed_bin_dir;
    let found = |spec, cfg: &str| {
        binaries::resolve(spec, cfg, bin)
            .map(|(p, _)| p)
            .filter(|p| p.is_file())
    };
    Ok(Tools {
        ytdlp: found(&binaries::YTDLP, &y_cfg)
            .ok_or("yt-dlp was not found. Install it or set its path in Settings → Engine.")?,
        ffmpeg: found(&binaries::FFMPEG, &f_cfg),
        deno: found(&binaries::DENO, &d_cfg),
    })
}

/// A tokio command for yt-dlp: no console window, piped output. The environment asks for
/// UTF-8, but the frozen yt-dlp.exe ignores it; callers also pass `--encoding utf-8`.
pub(crate) fn command(program: &Path, args: &[String]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(binaries::CREATE_NO_WINDOW);
    cmd
}

/// Put a spawned child (and everything it starts) in a kill-on-close Job Object.
pub(crate) fn adopt_tree(child: &tokio::process::Child) -> Option<ProcessTree> {
    #[cfg(windows)]
    {
        let handle = child.raw_handle()?;
        ProcessTree::adopt(handle)
            .map_err(|e| log::warn!("Could not create job object: {e}"))
            .ok()
    }
    #[cfg(not(windows))]
    {
        let _ = child;
        ProcessTree::adopt(std::ptr::null_mut()).ok()
    }
}

fn prepare(engine: &Engine, id: &str) -> Result<Prepared, String> {
    let job = engine.job(id).ok_or("Job disappeared")?;
    let state = engine.app.state::<AppState>();
    let folder = state.settings.lock().unwrap().download_folder.clone();
    let Tools {
        ytdlp: program,
        ffmpeg,
        deno,
    } = tools(&state)?;

    let temp_dir = engine.temp_dir(id);
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Could not create temp folder: {e}"))?;

    let mut warnings = Vec::new();
    let mut a = args::user_args(&job.options, &folder, &mut warnings);
    a.extend(engine_args(&temp_dir, ffmpeg.as_deref(), deno.as_deref()));
    a.push("--".into());
    a.push(job.url.clone());

    let display = args::powershell_display(&program.display().to_string(), &a);
    let mut warnings: Vec<String> = warnings
        .into_iter()
        .map(|w| format!("[TubeDeck] {:?}: {}", w.level, w.message))
        .collect();
    if ffmpeg.is_none() {
        warnings.push("[TubeDeck] FFmpeg not found: merging and conversion will fail.".into());
    }
    if deno.is_none() {
        warnings.push("[TubeDeck] Deno not found: YouTube may offer fewer formats.".into());
    }
    Ok(Prepared {
        program,
        args: a,
        display,
        warnings,
    })
}

pub async fn run(engine: Arc<Engine>, id: String) {
    let prepared = match prepare(&engine, &id) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Job {id} could not start: {e}");
            engine.push_log(&id, format!("[TubeDeck] {e}"));
            // Treat as a TubeDeck-side failure (e.g. yt-dlp missing): message as the error line.
            engine.complete(
                &id,
                Outcome {
                    exit: Err(e.clone()),
                    output: None,
                    stderr_tail: vec![format!("ERROR: {e}")],
                },
            );
            return;
        }
    };

    engine.push_log(&id, format!("[TubeDeck] {}", prepared.display));
    for w in &prepared.warnings {
        engine.push_log(&id, w.clone());
    }

    let mut child = match command(&prepared.program, &prepared.args).spawn() {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Could not start yt-dlp: {e}");
            log::error!("Job {id}: {msg}");
            engine.push_log(&id, format!("[TubeDeck] {msg}"));
            engine.complete(
                &id,
                Outcome {
                    exit: Err(msg),
                    output: None,
                    stderr_tail: Vec::new(),
                },
            );
            return;
        }
    };
    log::info!("Job {id} started (pid {:?})", child.id());

    if let Some(tree) = adopt_tree(&child) {
        engine.attach_tree(&id, Arc::new(tree));
    }

    engine.update(&id, |job| {
        job.status = JobStatus::Downloading;
        job.stage = Some("Fetching video info".into());
    });

    // yt-dlp prints download progress to stdout but postprocess progress and errors to
    // stderr, so both streams feed one ordered channel and go through the same parser.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(bool, String)>();
    for (is_err, reader) in [
        (
            false,
            Box::new(child.stdout.take().expect("stdout piped"))
                as Box<dyn AsyncRead + Unpin + Send>,
        ),
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

    let exit = child
        .wait()
        .await
        .map_err(|e| format!("Lost track of yt-dlp: {e}"));
    engine.complete(
        &id,
        Outcome {
            exit,
            output,
            stderr_tail: stderr_tail.into(),
        },
    );
}

fn handle_line(
    engine: &Engine,
    id: &str,
    tracker: &mut Tracker,
    output: &mut Option<String>,
    line: Line,
) {
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
            engine.push_log(
                id,
                format!(
                    "[TubeDeck] Downloading {}",
                    info.title.as_deref().unwrap_or("item")
                ),
            );
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
        let a = engine_args(
            Path::new(r"C:\tmp\job1"),
            Some(Path::new(r"C:\bin\ffmpeg.exe")),
            Some(Path::new(r"C:\bin\deno.exe")),
        );
        assert!(a.contains(&"--no-simulate".to_string()));
        let e = a.iter().position(|x| x == "--encoding").unwrap();
        assert_eq!(a[e + 1], "utf-8");
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
