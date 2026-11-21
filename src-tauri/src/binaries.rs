//! Locate yt-dlp, FFmpeg and Deno and read their versions.
//!
//! Resolution order: explicit path from settings → managed `bin` folder in app data → PATH.
//! Downloading/updating managed binaries arrives in M6.

use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// `CREATE_NO_WINDOW`: don't flash a console window for child processes.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Configured,
    Managed,
    Path,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryStatus {
    pub name: &'static str,
    pub found: bool,
    pub path: Option<String>,
    pub source: Option<Source>,
    pub version: Option<String>,
    /// False when found but unusable (too old, fails to run).
    pub ok: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub ytdlp: BinaryStatus,
    pub ffmpeg: BinaryStatus,
    pub deno: BinaryStatus,
    pub managed_dir: String,
}

pub struct Spec {
    pub name: &'static str,
    pub exe: &'static str,
    pub version_args: &'static [&'static str],
}

pub const YTDLP: Spec = Spec { name: "yt-dlp", exe: "yt-dlp.exe", version_args: &["--version"] };
pub const FFMPEG: Spec = Spec { name: "FFmpeg", exe: "ffmpeg.exe", version_args: &["-version"] };
pub const DENO: Spec = Spec { name: "Deno", exe: "deno.exe", version_args: &["--version"] };

/// Minimum Deno version supported by yt-dlp's EJS challenge solver.
const DENO_MIN: (u32, u32, u32) = (2, 3, 0);

pub fn resolve(spec: &Spec, configured: &str, managed_dir: &Path) -> Option<(PathBuf, Source)> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let p = PathBuf::from(configured);
        let p = if p.is_dir() { p.join(spec.exe) } else { p };
        // An explicit path that doesn't exist is reported, not silently replaced.
        return Some((p, Source::Configured));
    }
    let managed = managed_dir.join(spec.exe);
    if managed.is_file() {
        return Some((managed, Source::Managed));
    }
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|d| d.join(spec.exe))
        .find(|p| p.is_file())
        .map(|p| (p, Source::Path))
}

pub fn status(spec: &Spec, configured: &str, managed_dir: &Path) -> BinaryStatus {
    let mut st = BinaryStatus {
        name: spec.name,
        found: false,
        path: None,
        source: None,
        version: None,
        ok: false,
        note: None,
    };
    let Some((path, source)) = resolve(spec, configured, managed_dir) else {
        st.note = Some("Not found".into());
        return st;
    };
    st.path = Some(path.display().to_string());
    st.source = Some(source);
    if !path.is_file() {
        st.note = Some("Configured path does not exist".into());
        return st;
    }
    st.found = true;

    match run_capture(&path, spec.version_args, Duration::from_secs(15)) {
        Ok(out) => {
            st.version = parse_version(spec.name, &out);
            st.ok = st.version.is_some();
            if !st.ok {
                st.note = Some("Could not read version".into());
            }
        }
        Err(e) => st.note = Some(e),
    }

    if spec.name == DENO.name && st.ok {
        let too_old = st
            .version
            .as_deref()
            .and_then(parse_semver)
            .is_some_and(|v| v < DENO_MIN);
        if too_old {
            st.ok = false;
            st.note = Some(format!(
                "Deno {}.{}.{} or newer is required by yt-dlp",
                DENO_MIN.0, DENO_MIN.1, DENO_MIN.2
            ));
        }
    }
    st
}

pub fn engine_status(
    ytdlp_cfg: &str,
    ffmpeg_cfg: &str,
    deno_cfg: &str,
    managed_dir: &Path,
) -> EngineStatus {
    // Probe all three in parallel; each may take a moment on a cold start.
    std::thread::scope(|s| {
        let y = s.spawn(|| status(&YTDLP, ytdlp_cfg, managed_dir));
        let f = s.spawn(|| status(&FFMPEG, ffmpeg_cfg, managed_dir));
        let d = s.spawn(|| status(&DENO, deno_cfg, managed_dir));
        EngineStatus {
            ytdlp: y.join().expect("probe thread panicked"),
            ffmpeg: f.join().expect("probe thread panicked"),
            deno: d.join().expect("probe thread panicked"),
            managed_dir: managed_dir.display().to_string(),
        }
    })
}

/// Run a short-lived command without a console window and capture stdout.
pub fn run_capture(path: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut cmd = Command::new(path);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let child = cmd.spawn().map_err(|e| format!("Failed to start: {e}"))?;
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(out)) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).into_owned()),
        Ok(Ok(out)) => Err(format!(
            "Exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")
        )),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => {
            kill_pid(pid);
            Err("Timed out".into())
        }
    }
}

fn kill_pid(pid: u32) {
    let mut cmd = Command::new("taskkill");
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.status();
}

/// Extract a version string from `--version` output.
pub fn parse_version(name: &str, out: &str) -> Option<String> {
    let first = out.lines().map(str::trim).find(|l| !l.is_empty())?;
    match name {
        // "2025.11.12"
        "yt-dlp" => Some(first.to_string()),
        // "ffmpeg version N-119396-g48c0dba23b-20250428 Copyright (c) ..."
        "FFmpeg" => first.strip_prefix("ffmpeg version ")?.split_whitespace().next().map(String::from),
        // "deno 2.5.6 (stable, release, x86_64-pc-windows-msvc)"
        "Deno" => first.strip_prefix("deno ")?.split_whitespace().next().map(String::from),
        _ => Some(first.to_string()),
    }
}

pub fn parse_semver(v: &str) -> Option<(u32, u32, u32)> {
    let mut it = v.split(['.', '-', '+']).map(|p| p.parse::<u32>());
    Some((it.next()?.ok()?, it.next()?.ok()?, it.next().and_then(Result::ok).unwrap_or(0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("yt-dlp", "2025.11.12\n").as_deref(), Some("2025.11.12"));
        assert_eq!(
            parse_version("FFmpeg", "ffmpeg version N-119396-g48c0dba23b-20250428 Copyright (c) 2000-2025\nbuilt with gcc").as_deref(),
            Some("N-119396-g48c0dba23b-20250428")
        );
        assert_eq!(
            parse_version("FFmpeg", "ffmpeg version 7.1-full_build-www.gyan.dev Copyright").as_deref(),
            Some("7.1-full_build-www.gyan.dev")
        );
        assert_eq!(
            parse_version("Deno", "deno 2.5.6 (stable, release, x86_64-pc-windows-msvc)\nv8 14.0").as_deref(),
            Some("2.5.6")
        );
        assert_eq!(parse_version("Deno", "garbage"), None);
    }

    #[test]
    fn semver_compare() {
        assert_eq!(parse_semver("2.3.0"), Some((2, 3, 0)));
        assert_eq!(parse_semver("2.10"), Some((2, 10, 0)));
        assert!(parse_semver("2.2.9").unwrap() < DENO_MIN);
        assert!(parse_semver("2.10.0").unwrap() > DENO_MIN);
        assert_eq!(parse_semver("x.y"), None);
    }

    #[test]
    fn configured_missing_path_is_reported() {
        let st = status(&YTDLP, r"C:\definitely\not\here\yt-dlp.exe", Path::new(r"C:\nope"));
        assert!(!st.found);
        assert_eq!(st.source, Some(Source::Configured));
        assert_eq!(st.note.as_deref(), Some("Configured path does not exist"));
    }
}
