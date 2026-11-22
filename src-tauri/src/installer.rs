//! Download, verify and install the managed copies of yt-dlp, FFmpeg and Deno.
//!
//! Everything comes from fixed GitHub release URLs over HTTPS and is checked against the
//! SHA-256 checksum file published next to it; nothing is installed without a match.
//! Files are written next to the target and swapped in by rename, so a running yt-dlp keeps
//! working (Windows allows renaming a running .exe) and a failed download never leaves a
//! broken binary behind. Old copies that couldn't be deleted are removed on next start.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const EVENT_INSTALL: &str = "engine://install";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Component {
    Ytdlp,
    Ffmpeg,
    Deno,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    Nightly,
}

/// Where a component is downloaded from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub repo: &'static str,
    pub asset: &'static str,
    pub checksums: String,
}

pub fn source(component: Component, channel: Channel) -> Source {
    let release = |repo: &str, file: &str| {
        format!("https://github.com/{repo}/releases/latest/download/{file}")
    };
    match component {
        Component::Ytdlp => {
            let repo = match channel {
                Channel::Stable => "yt-dlp/yt-dlp",
                Channel::Nightly => "yt-dlp/yt-dlp-nightly-builds",
            };
            Source {
                repo,
                asset: "yt-dlp.exe",
                checksums: release(repo, "SHA2-256SUMS"),
            }
        }
        Component::Ffmpeg => Source {
            repo: "yt-dlp/FFmpeg-Builds",
            // The "shared" build is 91 MB instead of 202 MB: ffmpeg and ffprobe share DLLs.
            asset: "ffmpeg-master-latest-win64-gpl-shared.zip",
            checksums: release("yt-dlp/FFmpeg-Builds", "checksums.sha256"),
        },
        Component::Deno => Source {
            repo: "denoland/deno",
            asset: "deno-x86_64-pc-windows-msvc.zip",
            checksums: release("denoland/deno", "deno-x86_64-pc-windows-msvc.zip.sha256sum"),
        },
    }
}

impl Source {
    pub fn url(&self) -> String {
        format!(
            "https://github.com/{}/releases/latest/download/{}",
            self.repo, self.asset
        )
    }
}

/// Find the SHA-256 for `asset` in a checksum file. Handles `sha256sum` style lines
/// (`<hex>  <name>`) and single-hash files such as Deno's PowerShell `Get-FileHash` output.
pub fn expected_hash(checksums: &str, asset: &str) -> Option<String> {
    let is_hex64 = |t: &str| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit());
    for line in checksums.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(hash), Some(name)) = (parts.next(), parts.next())
            && is_hex64(hash)
            && name.trim_start_matches('*') == asset
        {
            return Some(hash.to_ascii_lowercase());
        }
    }
    let hashes: Vec<&str> = checksums
        .split(|c: char| !c.is_ascii_hexdigit())
        .filter(|t| is_hex64(t))
        .collect();
    match hashes.as_slice() {
        [only] => Some(only.to_ascii_lowercase()),
        _ => None,
    }
}

/// Compare dotted numeric versions ("2026.10.26", "2026.11.21.232659", "v2.9.8").
pub fn newer(latest: &str, installed: &str) -> bool {
    let nums = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split(['.', '-'])
            .map_while(|p| p.parse::<u64>().ok())
            .collect()
    };
    nums(latest) > nums(installed)
}

/// Nightly yt-dlp versions have a fourth part (time of build).
pub fn is_nightly_version(v: &str) -> bool {
    v.split('.').count() >= 4
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub component: Component,
    /// "downloading" | "verifying" | "installing" | "done" | "error"
    pub phase: &'static str,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub message: Option<String>,
}

fn client(redirects: bool) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("TubeDeck/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .redirect(if redirects {
            reqwest::redirect::Policy::limited(10)
        } else {
            reqwest::redirect::Policy::none()
        })
        .build()
        .map_err(|e| e.to_string())
}

/// An error with its full `source()` chain ("error sending request: …: connection reset").
fn error_chain(e: &dyn std::error::Error) -> String {
    let mut s = e.to_string();
    let mut src = e.source();
    while let Some(inner) = src {
        let text = inner.to_string();
        if !s.contains(&text) {
            s.push_str(": ");
            s.push_str(&text);
        }
        src = inner.source();
    }
    s
}

const ATTEMPTS: u32 = 3;

/// GET with retries for connection-level failures (seen in practice when several
/// downloads start at once). HTTP error statuses are not retried.
async fn get(http: &reqwest::Client, url: &str) -> Result<reqwest::Response, String> {
    let mut last = String::new();
    for attempt in 1..=ATTEMPTS {
        match http.get(url).send().await {
            Ok(resp) => return resp.error_for_status().map_err(|e| error_chain(&e)),
            Err(e) => {
                last = error_chain(&e);
                log::warn!("GET {url} failed (attempt {attempt}/{ATTEMPTS}): {last}");
                if attempt < ATTEMPTS {
                    tokio::time::sleep(Duration::from_millis(700 * u64::from(attempt))).await;
                }
            }
        }
    }
    Err(last)
}

/// Latest release tag of a GitHub repo, read from the `/releases/latest` redirect
/// (no API token or rate limit). Returns e.g. "2026.10.26" or "v2.9.8".
pub async fn latest_tag(repo: &str) -> Result<String, String> {
    let url = format!("https://github.com/{repo}/releases/latest");
    let http = client(false)?;
    let mut last = String::new();
    let mut resp = None;
    for attempt in 1..=ATTEMPTS {
        match http.get(&url).send().await {
            Ok(r) => {
                resp = Some(r);
                break;
            }
            Err(e) => {
                last = error_chain(&e);
                if attempt < ATTEMPTS {
                    tokio::time::sleep(Duration::from_millis(700 * u64::from(attempt))).await;
                }
            }
        }
    }
    let resp = resp.ok_or_else(|| format!("Could not reach GitHub: {last}"))?;
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| format!("GitHub didn't report a latest release for {repo}"))?;
    location
        .rsplit_once("/tag/")
        .map(|(_, tag)| tag.to_string())
        .ok_or_else(|| format!("Unexpected release URL: {location}"))
}

/// Private staging folder for one component's download and extraction.
fn staging_dir(bin_dir: &Path, component: Component) -> PathBuf {
    let name = match component {
        Component::Ytdlp => "ytdlp",
        Component::Ffmpeg => "ffmpeg",
        Component::Deno => "deno",
    };
    bin_dir.join(".downloads").join(name)
}

/// Download, verify and install one component into `bin_dir`.
pub async fn install(
    component: Component,
    channel: Channel,
    bin_dir: &Path,
    mut progress: impl FnMut(InstallProgress),
) -> Result<(), String> {
    let src = source(component, channel);
    // Each component gets its own staging folder: "Install everything" runs the three
    // installs in parallel, and a shared folder was deleted by whichever finished first
    // while another was still about to write into it (os error 3).
    let staging = staging_dir(bin_dir, component);
    let _ = fs::remove_dir_all(&staging); // leftovers from an interrupted attempt
    fs::create_dir_all(&staging)
        .map_err(|e| format!("Could not create {}: {e}", staging.display()))?;
    let report = |phase, downloaded, total, message: Option<String>| InstallProgress {
        component,
        phase,
        downloaded,
        total,
        message,
    };

    let http = client(true)?;
    let sums = get(&http, &src.checksums)
        .await
        .map_err(|e| format!("Could not download the checksum file: {e}"))?
        .text()
        .await
        .map_err(|e| error_chain(&e))?;
    let expected = expected_hash(&sums, src.asset).ok_or_else(|| {
        format!(
            "No checksum for {} was published; not installing an unverified file",
            src.asset
        )
    })?;

    let mut resp = get(&http, &src.url())
        .await
        .map_err(|e| format!("Download failed: {e}"))?;
    let total = resp.content_length();
    let part = staging.join(format!("{}.part", src.asset));
    let mut file =
        fs::File::create(&part).map_err(|e| format!("Could not write {}: {e}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut last = Instant::now() - Duration::from_secs(1);
    progress(report("downloading", 0, total, None));
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("Download interrupted: {}", error_chain(&e)))?
    {
        hasher.update(&chunk);
        file.write_all(&chunk)
            .map_err(|e| format!("Could not write download: {e}"))?;
        downloaded += chunk.len() as u64;
        if last.elapsed() >= Duration::from_millis(200) {
            last = Instant::now();
            progress(report("downloading", downloaded, total, None));
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);

    progress(report("verifying", downloaded, total, None));
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual != expected {
        let _ = fs::remove_file(&part);
        return Err(format!(
            "Checksum mismatch for {} (expected {expected}, got {actual}); the file was discarded",
            src.asset
        ));
    }

    progress(report("installing", downloaded, total, None));
    let result = match component {
        Component::Ytdlp => swap_file(&part, &bin_dir.join("yt-dlp.exe")),
        Component::Deno => {
            let exe = staging.join("deno.exe.new");
            extract_files(&part, |name| (name == "deno.exe").then(|| exe.clone()))?;
            swap_file(&exe, &bin_dir.join("deno.exe"))
        }
        Component::Ffmpeg => {
            let dir = staging.join("ffmpeg.new");
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let count = extract_files(&part, |name| {
                let (parent, file) = name.rsplit_once('/')?;
                let wanted = parent.ends_with("bin")
                    && (file.ends_with(".dll") || file == "ffmpeg.exe" || file == "ffprobe.exe");
                wanted.then(|| dir.join(file))
            })?;
            if !dir.join("ffmpeg.exe").is_file() || !dir.join("ffprobe.exe").is_file() {
                return Err(format!(
                    "The FFmpeg archive didn't contain ffmpeg.exe and ffprobe.exe ({count} files found)"
                ));
            }
            swap_dir(&dir, &bin_dir.join("ffmpeg"))
        }
    };
    // Remove only this install's own folder; the shared parent is tidied at startup.
    let _ = fs::remove_dir_all(&staging);
    result?;
    progress(report("done", downloaded, total, None));
    Ok(())
}

/// Extract zip entries for which `target(name)` returns a destination path.
/// Entry names are only used to pick files; output paths always come from `target`,
/// so a malicious archive can't write outside the staging folder.
fn extract_files(
    zip_path: &Path,
    target: impl Fn(&str) -> Option<PathBuf>,
) -> Result<usize, String> {
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive =
        zip::ZipArchive::new(io::BufReader::new(file)).map_err(|e| format!("Bad archive: {e}"))?;
    let mut count = 0;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Bad archive entry: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|e| format!("Bad archive entry name: {e}"))?
            .replace('\\', "/");
        let Some(dest) = target(&name) else { continue };
        let mut out = fs::File::create(&dest)
            .map_err(|e| format!("Could not write {}: {e}", dest.display()))?;
        io::copy(&mut entry, &mut out).map_err(|e| format!("Could not extract {name}: {e}"))?;
        count += 1;
    }
    Ok(count)
}

fn old_name(path: &Path) -> PathBuf {
    let stamp = crate::engine::job::now_ms();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{name}.old-{stamp}"))
}

/// Replace `dest` with `new`: rename the current file aside (works even while it runs),
/// move the new one in, then try to delete the old one.
fn swap_file(new: &Path, dest: &Path) -> Result<(), String> {
    let old = old_name(dest);
    if dest.exists() {
        fs::rename(dest, &old).map_err(|e| format!("Could not replace {}: {e}", dest.display()))?;
    }
    if let Err(e) = fs::rename(new, dest) {
        let _ = fs::rename(&old, dest); // put the previous version back
        return Err(format!("Could not install {}: {e}", dest.display()));
    }
    let _ = fs::remove_file(&old);
    Ok(())
}

fn swap_dir(new: &Path, dest: &Path) -> Result<(), String> {
    let old = old_name(dest);
    if dest.exists() {
        fs::rename(dest, &old).map_err(|e| {
            format!(
                "Could not replace {}: {e}. If downloads are running, try again when they finish.",
                dest.display()
            )
        })?;
    }
    if let Err(e) = fs::rename(new, dest) {
        let _ = fs::rename(&old, dest);
        return Err(format!("Could not install {}: {e}", dest.display()));
    }
    let _ = fs::remove_dir_all(&old);
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentUpdate {
    pub installed: Option<String>,
    pub latest: Option<String>,
    /// True when TubeDeck manages this copy (so it can update it).
    pub managed: bool,
    pub update_available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub ytdlp: ComponentUpdate,
    pub deno: ComponentUpdate,
    pub ffmpeg: ComponentUpdate,
    pub checked_at: u64,
}

fn component_update(
    status: &crate::binaries::BinaryStatus,
    latest: Result<String, String>,
    channel_mismatch: impl Fn(&str) -> bool,
) -> ComponentUpdate {
    let managed = status.source == Some(crate::binaries::Source::Managed);
    let installed = status.version.clone();
    let (latest, error) = match latest {
        Ok(t) => (Some(t.trim_start_matches('v').to_string()), None),
        Err(e) => (None, Some(e)),
    };
    let update_available = match (&installed, &latest) {
        (Some(i), Some(l)) => managed && (newer(l, i) || channel_mismatch(i)),
        _ => false,
    };
    ComponentUpdate {
        installed,
        latest,
        managed,
        update_available,
        error,
    }
}

/// Compare installed versions with the latest releases.
pub async fn check(status: &crate::binaries::EngineStatus, channel: Channel) -> UpdateInfo {
    let (y, d) = tokio::join!(
        latest_tag(source(Component::Ytdlp, channel).repo),
        latest_tag("denoland/deno")
    );
    UpdateInfo {
        // Switching from nightly back to stable counts as an update even if "older".
        ytdlp: component_update(&status.ytdlp, y, |i| {
            channel == Channel::Stable && is_nightly_version(i)
        }),
        deno: component_update(&status.deno, d, |_| false),
        // FFmpeg's builds are rolling ("latest"); offer a reinstall instead of versions.
        ffmpeg: ComponentUpdate {
            installed: status.ffmpeg.version.clone(),
            latest: None,
            managed: status.ffmpeg.source == Some(crate::binaries::Source::Managed),
            update_available: false,
            error: None,
        },
        checked_at: crate::engine::job::now_ms(),
    }
}

/// Remove leftovers from earlier installs (`*.old-*` and the staging folder).
pub fn cleanup(bin_dir: &Path) {
    let _ = fs::remove_dir_all(bin_dir.join(".downloads"));
    if let Ok(entries) = fs::read_dir(bin_dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().contains(".old-") {
                let p = e.path();
                let _ = if p.is_dir() {
                    fs::remove_dir_all(&p)
                } else {
                    fs::remove_file(&p)
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_hashes_in_both_formats() {
        let sums = "82d438ad3d7eeb6576d0a772c24bc13a7a40bda66fb11d4cc1c89c1b1aeee8ea  ffmpeg-master-latest-win64-gpl.zip\n\
                    3032be6ee290de253bec1fe57d7bb3aa5d711244813b3d43625d750e66ab2d11  ffmpeg-master-latest-win64-gpl-shared.zip\n";
        assert_eq!(
            expected_hash(sums, "ffmpeg-master-latest-win64-gpl-shared.zip").as_deref(),
            Some("3032be6ee290de253bec1fe57d7bb3aa5d711244813b3d43625d750e66ab2d11")
        );
        // Two different hashes and no matching name: refuse rather than guess.
        assert_eq!(expected_hash(sums, "other.zip"), None);

        let deno = "\r\nAlgorithm : SHA256\r\nHash      : CBA84B26E0DF5AE7F74FC66B86A95B866872A86292EBD84FA1E959927EDD93BF\r\nPath      : C:\\a\\deno\\deno-x86_64-pc-windows-msvc.zip\r\n";
        assert_eq!(
            expected_hash(deno, "deno-x86_64-pc-windows-msvc.zip").as_deref(),
            Some("cba84b26e0df5ae7f74fc66b86a95b866872a86292ebd84fa1e959927edd93bf")
        );
        assert_eq!(expected_hash("garbage", "x"), None);
        assert!(
            expected_hash(
                "f5389d3698ecde4ef06f4e6b1ce0294fe9ac1396228c8990b8d3416d6d177a7d *yt-dlp.exe",
                "yt-dlp.exe"
            )
            .is_some()
        );
    }

    #[test]
    fn compares_versions() {
        assert!(newer("2026.11.02", "2026.10.26"));
        assert!(!newer("2026.10.26", "2026.10.26"));
        assert!(newer("2026.11.21.232659", "2026.10.26"));
        assert!(!newer("2026.10.26", "2026.11.21.232659"));
        assert!(newer("v2.10.0", "2.9.8"));
        assert!(!newer("v2.9.8", "2.9.8"));
        assert!(is_nightly_version("2026.11.21.232659"));
        assert!(!is_nightly_version("2026.10.26"));
    }

    #[test]
    fn sources_are_https_github_and_channel_aware() {
        for c in [Component::Ytdlp, Component::Ffmpeg, Component::Deno] {
            let s = source(c, Channel::Stable);
            assert!(
                s.url().starts_with("https://github.com/")
                    && s.checksums.starts_with("https://github.com/")
            );
        }
        assert_eq!(
            source(Component::Ytdlp, Channel::Nightly).repo,
            "yt-dlp/yt-dlp-nightly-builds"
        );
    }

    /// Real downloads (≈155 MB): all three installs in parallel into a temp folder, twice,
    /// like "Install everything". Run with `cargo test -- --ignored parallel_install_real`.
    #[tokio::test]
    #[ignore = "downloads from GitHub"]
    async fn parallel_install_real_network() {
        let bin = std::env::temp_dir().join(format!("tubedeck-install-{}", std::process::id()));
        let _ = fs::remove_dir_all(&bin);
        fs::create_dir_all(&bin).unwrap();
        for round in 1..=2 {
            let run = |c| install(c, Channel::Stable, &bin, |_| {});
            let (y, f, d) = tokio::join!(
                run(Component::Ytdlp),
                run(Component::Ffmpeg),
                run(Component::Deno)
            );
            assert!(
                y.is_ok() && f.is_ok() && d.is_ok(),
                "round {round}: {y:?} {f:?} {d:?}"
            );
            assert!(bin.join("yt-dlp.exe").is_file() && bin.join("deno.exe").is_file());
            assert!(
                bin.join("ffmpeg").join("ffmpeg.exe").is_file()
                    && bin.join("ffmpeg").join("ffprobe.exe").is_file()
            );
            let leftovers: Vec<_> = fs::read_dir(bin.join(".downloads"))
                .map(|r| r.flatten().map(|e| e.file_name()).collect())
                .unwrap_or_default();
            assert!(
                leftovers.is_empty(),
                "round {round}: staging leftovers {leftovers:?}"
            );
        }
        let _ = fs::remove_dir_all(&bin);
    }

    /// Regression: "Install everything" runs installs in parallel. One install finishing
    /// must not remove the folder another install is about to write its download into.
    #[test]
    fn parallel_installs_do_not_share_staging() {
        let bin = std::env::temp_dir().join(format!("tubedeck-staging-{}", std::process::id()));
        let _ = fs::remove_dir_all(&bin);
        let ytdlp = staging_dir(&bin, Component::Ytdlp);
        let deno = staging_dir(&bin, Component::Deno);
        assert_ne!(ytdlp, deno);
        fs::create_dir_all(&ytdlp).unwrap();
        fs::create_dir_all(&deno).unwrap(); // deno created its folder, no file yet
        fs::remove_dir_all(&ytdlp).unwrap(); // yt-dlp finishes and cleans up
        fs::write(deno.join("deno.zip.part"), b"x").expect("deno can still write its download");
        let _ = fs::remove_dir_all(&bin);
    }

    #[test]
    fn swap_replaces_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!("tubedeck-swap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("tool.exe");
        fs::write(&dest, "old").unwrap();
        let new = dir.join("tool.new");
        fs::write(&new, "new").unwrap();
        swap_file(&new, &dest).unwrap();
        assert_eq!(fs::read_to_string(&dest).unwrap(), "new");
        assert!(!new.exists());
        // Leftovers are removed by cleanup.
        fs::write(dir.join("tool.exe.old-1"), "x").unwrap();
        fs::create_dir_all(dir.join(".downloads")).unwrap();
        cleanup(&dir);
        assert!(!dir.join("tool.exe.old-1").exists() && !dir.join(".downloads").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
