//! Built-in and user presets. Built-ins live in code; user presets in `presets.json`.

use crate::options::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub builtin: bool,
    pub options: DownloadOptions,
}

fn builtin(id: &str, name: &str, description: &str, options: DownloadOptions) -> Preset {
    Preset {
        id: format!("builtin-{id}"),
        name: name.into(),
        description: description.into(),
        builtin: true,
        options,
    }
}

pub fn builtins() -> Vec<Preset> {
    let d = DownloadOptions::default;
    vec![
        builtin(
            "best-video",
            "Best video (MP4)",
            "Highest available quality, merged to MP4 with thumbnail, metadata and chapters.",
            d(),
        ),
        builtin(
            "1080p-h264",
            "1080p H.264",
            "Up to 1080p with H.264 video for maximum player compatibility.",
            DownloadOptions { quality: Quality::P1080, video_codec: VideoCodec::H264, ..d() },
        ),
        builtin(
            "audio-mp3",
            "Audio MP3",
            "Extract audio as high-quality MP3 with cover art.",
            DownloadOptions {
                mode: Mode::Audio,
                audio_format: AudioFormat::Mp3,
                audio_quality: 0,
                ..d()
            },
        ),
        builtin(
            "audio-opus",
            "Audio Opus",
            "Best audio stream kept as Opus; small files, excellent quality.",
            DownloadOptions { mode: Mode::Audio, audio_format: AudioFormat::Opus, ..d() },
        ),
        builtin(
            "archive",
            "Archive (MKV + subs)",
            "Best quality in MKV with embedded subtitles, chapters, metadata and SponsorBlock chapters.",
            DownloadOptions {
                container: Container::Mkv,
                subtitles: Subtitles::Embed,
                auto_subs: true,
                sponsor_block: SponsorBlock::Mark,
                ..d()
            },
        ),
    ]
}

/// Merge built-ins with user presets (built-ins first).
pub fn all(user: &[Preset]) -> Vec<Preset> {
    let mut v = builtins();
    v.extend(user.iter().filter(|p| !p.builtin).cloned());
    v
}

/// Insert or replace a user preset. Returns an error for built-in ids.
pub fn upsert(user: &mut Vec<Preset>, mut preset: Preset) -> Result<Preset, String> {
    let name = preset.name.trim();
    if name.is_empty() {
        return Err("Preset name cannot be empty".into());
    }
    if name.chars().count() > 60 {
        return Err("Preset name is too long (max 60 characters)".into());
    }
    preset.name = name.to_string();
    if preset.id.starts_with("builtin-") {
        return Err("Built-in presets can't be changed; save a copy instead".into());
    }
    if preset.id.is_empty() {
        preset.id = new_id();
    }
    preset.builtin = false;
    match user.iter_mut().find(|p| p.id == preset.id) {
        Some(existing) => *existing = preset.clone(),
        None => user.push(preset.clone()),
    }
    Ok(preset)
}

pub fn remove(user: &mut Vec<Preset>, id: &str) -> Result<(), String> {
    if id.starts_with("builtin-") {
        return Err("Built-in presets can't be deleted".into());
    }
    let before = user.len();
    user.retain(|p| p.id != id);
    if user.len() == before {
        return Err("Preset not found".into());
    }
    Ok(())
}

fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("user-{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_preset(id: &str, name: &str) -> Preset {
        Preset {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            builtin: false,
            options: DownloadOptions::default(),
        }
    }

    #[test]
    fn builtin_ids_are_unique_and_prefixed() {
        let b = builtins();
        let mut ids: Vec<_> = b.iter().map(|p| p.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), b.len());
        assert!(b.iter().all(|p| p.builtin && p.id.starts_with("builtin-")));
    }

    #[test]
    fn upsert_assigns_id_and_replaces() {
        let mut user = Vec::new();
        let p = upsert(&mut user, user_preset("", "  Mine  ")).unwrap();
        assert!(p.id.starts_with("user-"));
        assert_eq!(p.name, "Mine");
        upsert(&mut user, Preset { name: "Renamed".into(), ..p.clone() }).unwrap();
        assert_eq!(user.len(), 1);
        assert_eq!(user[0].name, "Renamed");
    }

    #[test]
    fn builtins_are_protected() {
        let mut user = Vec::new();
        assert!(upsert(&mut user, user_preset("builtin-audio-mp3", "x")).is_err());
        assert!(upsert(&mut user, user_preset("", " ")).is_err());
        assert!(remove(&mut user, "builtin-audio-mp3").is_err());
        assert!(remove(&mut user, "missing").is_err());
    }
}
