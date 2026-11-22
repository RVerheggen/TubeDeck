//! Inspect a URL before downloading: is it a single video or a playlist, and what's in it?
//!
//! Runs `yt-dlp -J --flat-playlist`, which lists a playlist's entries without visiting each
//! video (a 136-item channel takes ~2 s). Also builds the per-item filename template used
//! when a playlist is added through the picker.

use super::{
    errors, progress,
    runner::{self, Tools},
    winjob::ProcessTree,
};
use crate::options::Browser;
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::io::AsyncReadExt;

const PROBE_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// 1-based position in the playlist.
    pub index: u64,
    pub id: Option<String>,
    pub title: String,
    pub url: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    /// The entry is itself a playlist (e.g. a channel's Playlists tab).
    pub is_playlist: bool,
    /// Private, deleted or otherwise not downloadable.
    pub unavailable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProbeResult {
    #[serde(rename_all = "camelCase")]
    Video {
        id: Option<String>,
        title: String,
        url: String,
        duration: Option<f64>,
        thumbnail: Option<String>,
        uploader: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Playlist {
        id: Option<String>,
        title: String,
        url: String,
        uploader: Option<String>,
        entries: Vec<Entry>,
    },
}

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|x| !x.is_empty())
        .map(String::from)
}

fn thumbnail(v: &Value) -> Option<String> {
    s(v, "thumbnail").or_else(|| {
        v.get("thumbnails")?
            .as_array()?
            .iter()
            .find_map(|t| s(t, "url"))
    })
}

const UNAVAILABLE_TITLES: &[&str] = &["[Private video]", "[Deleted video]", "[Unavailable video]"];
const UNAVAILABLE_STATES: &[&str] = &["private", "needs_auth", "subscriber_only", "premium_only"];

fn entry(v: &Value, index: u64) -> Entry {
    let id = s(v, "id");
    let ie_key = s(v, "ie_key").unwrap_or_default();
    let mut url = s(v, "url").or_else(|| s(v, "webpage_url"));
    // Some extractors give only an id; YouTube ids can be turned into a watch URL.
    if let (Some(u), Some(id)) = (&url, &id)
        && !u.contains("://")
        && ie_key == "Youtube"
    {
        url = Some(format!("https://www.youtube.com/watch?v={id}"));
    }
    let url = url.filter(|u| u.starts_with("http://") || u.starts_with("https://"));
    let title =
        s(v, "title").unwrap_or_else(|| id.clone().unwrap_or_else(|| format!("Item {index}")));
    let availability = s(v, "availability").unwrap_or_default();
    let unavailable = url.is_none()
        || UNAVAILABLE_TITLES.contains(&title.as_str())
        || UNAVAILABLE_STATES.contains(&availability.as_str());
    Entry {
        index,
        is_playlist: ie_key.ends_with("Tab") || s(v, "_type").as_deref() == Some("playlist"),
        duration: v.get("duration").and_then(Value::as_f64),
        thumbnail: thumbnail(v),
        id,
        title,
        url,
        unavailable,
    }
}

/// Parse `yt-dlp -J --flat-playlist` output.
pub fn parse(json: &str, source_url: &str) -> Result<ProbeResult, String> {
    let v: Value =
        serde_json::from_str(json).map_err(|e| format!("yt-dlp returned unreadable data: {e}"))?;
    if v.is_null() {
        return Err("yt-dlp found nothing to download at this URL".into());
    }
    let url = s(&v, "webpage_url").unwrap_or_else(|| source_url.to_string());
    let title = s(&v, "title").unwrap_or_else(|| url.clone());
    let uploader = s(&v, "uploader").or_else(|| s(&v, "channel"));

    match v.get("entries").and_then(Value::as_array) {
        Some(entries) => Ok(ProbeResult::Playlist {
            id: s(&v, "id"),
            title,
            url,
            uploader,
            entries: entries
                .iter()
                .enumerate()
                .map(|(i, e)| entry(e, i as u64 + 1))
                .collect(),
        }),
        None => Ok(ProbeResult::Video {
            id: s(&v, "id"),
            title,
            url,
            duration: v.get("duration").and_then(Value::as_f64),
            thumbnail: thumbnail(&v),
            uploader,
        }),
    }
}

/// Running probes by token, so the UI can cancel one.
#[derive(Default)]
pub struct Probes {
    running: Mutex<HashMap<String, Arc<ProcessTree>>>,
    canceled: Mutex<HashSet<String>>,
}

impl Probes {
    pub fn cancel(&self, token: &str) {
        self.canceled.lock().unwrap().insert(token.to_string());
        if let Some(tree) = self.running.lock().unwrap().get(token) {
            let _ = tree.kill();
        }
    }
}

pub fn probe_args(url: &str, tools: &Tools, cookies: Option<Browser>) -> Vec<String> {
    let mut a: Vec<String> = ["-J", "--flat-playlist", "--color", "never"]
        .iter()
        .map(|x| x.to_string())
        .collect();
    if let Some(d) = &tools.deno {
        a.push("--js-runtimes".into());
        a.push(format!("deno:{}", d.display()));
    }
    if let Some(b) = cookies {
        a.push("--cookies-from-browser".into());
        a.push(b.as_str().into());
    }
    a.push("--".into());
    a.push(url.to_string());
    a
}

pub async fn run(
    probes: &Probes,
    token: &str,
    url: &str,
    tools: &Tools,
    cookies: Option<Browser>,
) -> Result<ProbeResult, String> {
    crate::args::validate_url(url)?;
    let args = probe_args(url, tools, cookies);
    let mut child = runner::command(&tools.ytdlp, &args)
        .spawn()
        .map_err(|e| format!("Could not start yt-dlp: {e}"))?;
    if let Some(tree) = runner::adopt_tree(&child) {
        probes
            .running
            .lock()
            .unwrap()
            .insert(token.to_string(), Arc::new(tree));
    }
    // Cancel may have arrived before the process tree was registered.
    if probes.canceled.lock().unwrap().contains(token)
        && let Some(t) = probes.running.lock().unwrap().get(token)
    {
        let _ = t.kill();
    }

    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");
    let work = async {
        let mut out = String::new();
        let mut err = String::new();
        let (r1, r2) = tokio::join!(
            stdout.read_to_string(&mut out),
            stderr.read_to_string(&mut err)
        );
        r1.map_err(|e| e.to_string())?;
        r2.map_err(|e| e.to_string())?;
        let status = child.wait().await.map_err(|e| e.to_string())?;
        Ok::<_, String>((status, out, err))
    };
    let result = tokio::time::timeout(PROBE_TIMEOUT, work).await;

    let tree = probes.running.lock().unwrap().remove(token);
    let canceled = probes.canceled.lock().unwrap().remove(token);
    if canceled {
        return Err("Canceled".into());
    }
    let (status, out, err) = match result {
        Ok(r) => r?,
        Err(_) => {
            if let Some(t) = tree {
                let _ = t.kill();
            }
            return Err(format!(
                "Inspecting the URL took longer than {} s",
                PROBE_TIMEOUT.as_secs()
            ));
        }
    };
    if !status.success() {
        let lines: Vec<String> = err.lines().map(String::from).collect();
        let msg = progress::error_message(&lines)
            .unwrap_or_else(|| format!("yt-dlp exited with {status}"));
        let hint = errors::classify(&lines)
            .hint()
            .map(|h| format!(" {h}"))
            .unwrap_or_default();
        return Err(format!("{msg}.{hint}").replace("..", "."));
    }
    parse(&out, url)
}

// ---- Playlist items -----------------------------------------------------------------

/// A playlist title made safe as a Windows folder name.
pub fn folder_name(title: &str) -> String {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut name: String = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.chars().count() > 80 {
        name = name.chars().take(80).collect();
    }
    let name = name.trim_end_matches(['.', ' ']).to_string();
    if name.is_empty() {
        return "Playlist".into();
    }
    if RESERVED.contains(&name.to_ascii_uppercase().as_str()) {
        return format!("{name}_");
    }
    name
}

/// Filename template for one playlist item: `<folder>/<NN> - <template>`.
/// Literal text is escaped for yt-dlp's output template syntax (`%` → `%%`).
pub fn item_template(template: &str, folder: Option<&str>, number: Option<(u64, u64)>) -> String {
    let mut prefix = String::new();
    if let Some(f) = folder {
        prefix.push_str(&f.replace('%', "%%"));
        prefix.push('/');
    }
    if let Some((index, count)) = number {
        let width = count.max(index).to_string().len().max(2);
        prefix.push_str(&format!("{index:0width$} - "));
    }
    format!("{prefix}{template}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_real_playlist() {
        let r = parse(include_str!("fixtures/probe-playlist.json"), "https://x").unwrap();
        let ProbeResult::Playlist {
            title,
            entries,
            uploader,
            url,
            ..
        } = r
        else {
            panic!("expected playlist")
        };
        assert_eq!(title, "Blender Studio Logs");
        assert_eq!(uploader.as_deref(), Some("Blender Studio"));
        assert!(url.contains("list=PLav47HAVZMjkxzzLDqYpTCosbsWjosUN0"));
        assert_eq!(entries.len(), 3);
        let e = &entries[0];
        assert_eq!(e.index, 1);
        assert_eq!(
            e.url.as_deref(),
            Some("https://www.youtube.com/watch?v=Q1fMMYyt0I4")
        );
        assert_eq!(e.duration, Some(855.0));
        assert!(
            e.thumbnail
                .as_deref()
                .unwrap()
                .starts_with("https://i.ytimg.com/")
        );
        assert!(!e.is_playlist && !e.unavailable);
        assert_eq!(entries[2].index, 3);
    }

    #[test]
    fn parses_nested_playlists() {
        let r = parse(include_str!("fixtures/probe-nested.json"), "https://x").unwrap();
        let ProbeResult::Playlist { entries, .. } = r else {
            panic!()
        };
        assert!(
            entries
                .iter()
                .all(|e| e.is_playlist && e.url.as_deref().unwrap().contains("list="))
        );
    }

    #[test]
    fn parses_single_video() {
        let r = parse(include_str!("fixtures/probe-single.json"), "https://x").unwrap();
        let ProbeResult::Video {
            id,
            title,
            url,
            duration,
            thumbnail,
            uploader,
        } = r
        else {
            panic!("expected video")
        };
        assert_eq!(id.as_deref(), Some("jNQXAC9IVRw"));
        assert_eq!(title, "Me at the zoo");
        assert_eq!(url, "https://www.youtube.com/watch?v=jNQXAC9IVRw");
        assert_eq!(duration, Some(19.0));
        assert!(thumbnail.unwrap().starts_with("https://i.ytimg.com/"));
        assert_eq!(uploader.as_deref(), Some("jawed"));
    }

    #[test]
    fn marks_unavailable_and_fixes_bare_ids() {
        let json = r#"{"_type":"playlist","title":"T","entries":[
            {"id":"aaa","title":"[Private video]","url":"https://www.youtube.com/watch?v=aaa","ie_key":"Youtube"},
            {"id":"bbb","title":"B","url":"bbb","ie_key":"Youtube"},
            {"id":"ccc","title":"C","ie_key":"Other"},
            {"id":"ddd","title":"D","url":"https://e.com/d","availability":"needs_auth"}
        ]}"#;
        let ProbeResult::Playlist { entries, .. } = parse(json, "https://x").unwrap() else {
            panic!()
        };
        assert!(entries[0].unavailable);
        assert_eq!(
            entries[1].url.as_deref(),
            Some("https://www.youtube.com/watch?v=bbb")
        );
        assert!(!entries[1].unavailable);
        assert!(entries[2].unavailable && entries[2].url.is_none());
        assert!(entries[3].unavailable);
    }

    #[test]
    fn rejects_null_and_garbage() {
        assert!(parse("null", "u").is_err());
        assert!(parse("{oops", "u").is_err());
    }

    #[test]
    fn folder_names_are_windows_safe() {
        assert_eq!(folder_name("Blender Studio Logs"), "Blender Studio Logs");
        assert_eq!(folder_name("A/B: C? <D>|\"E\"*"), "A_B_ C_ _D___E__");
        assert_eq!(folder_name("  trailing dots... "), "trailing dots");
        assert_eq!(folder_name("CON"), "CON_");
        assert_eq!(folder_name("..."), "Playlist");
        assert_eq!(folder_name(&"x".repeat(200)).chars().count(), 80);
        assert_eq!(folder_name("Mixed\twhite\n space"), "Mixed_white_ space");
    }

    #[test]
    fn item_templates() {
        let t = "%(title)s [%(id)s].%(ext)s";
        assert_eq!(
            item_template(t, Some("Logs"), Some((3, 32))),
            "Logs/03 - %(title)s [%(id)s].%(ext)s"
        );
        assert_eq!(
            item_template(t, Some("100% Fun"), None),
            "100%% Fun/%(title)s [%(id)s].%(ext)s"
        );
        assert_eq!(
            item_template(t, None, Some((7, 1500))),
            "0007 - %(title)s [%(id)s].%(ext)s"
        );
        assert_eq!(item_template(t, None, None), t);
    }
}
