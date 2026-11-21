//! The download option model shared by the UI, presets and the engine.
//!
//! Field names are camelCase on the wire so `src/lib/types.ts` can mirror them 1:1.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Quality {
    #[default]
    #[serde(rename = "best")]
    Best,
    #[serde(rename = "2160")]
    P2160,
    #[serde(rename = "1440")]
    P1440,
    #[serde(rename = "1080")]
    P1080,
    #[serde(rename = "720")]
    P720,
    #[serde(rename = "480")]
    P480,
    #[serde(rename = "360")]
    P360,
}

impl Quality {
    /// Maximum height in pixels, or `None` for "best available".
    pub fn height(self) -> Option<u32> {
        match self {
            Quality::Best => None,
            Quality::P2160 => Some(2160),
            Quality::P1440 => Some(1440),
            Quality::P1080 => Some(1080),
            Quality::P720 => Some(720),
            Quality::P480 => Some(480),
            Quality::P360 => Some(360),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Container {
    #[default]
    Mp4,
    Mkv,
    Webm,
}

impl Container {
    pub fn as_str(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mkv => "mkv",
            Container::Webm => "webm",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VideoCodec {
    #[default]
    Any,
    H264,
    Vp9,
    Av1,
}

impl VideoCodec {
    /// Value for yt-dlp's `-S vcodec:<x>` sort field.
    pub fn sort_value(self) -> Option<&'static str> {
        match self {
            VideoCodec::Any => None,
            VideoCodec::H264 => Some("h264"),
            VideoCodec::Vp9 => Some("vp9"),
            VideoCodec::Av1 => Some("av01"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    #[default]
    Best,
    Mp3,
    M4a,
    Opus,
    Flac,
    Wav,
}

impl AudioFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            AudioFormat::Best => "best",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::M4a => "m4a",
            AudioFormat::Opus => "opus",
            AudioFormat::Flac => "flac",
            AudioFormat::Wav => "wav",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Subtitles {
    #[default]
    None,
    Embed,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SponsorBlock {
    #[default]
    Off,
    Mark,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Browser {
    Firefox,
    Edge,
    Chrome,
    Brave,
}

impl Browser {
    pub fn as_str(self) -> &'static str {
        match self {
            Browser::Firefox => "firefox",
            Browser::Edge => "edge",
            Browser::Chrome => "chrome",
            Browser::Brave => "brave",
        }
    }
}

pub const DEFAULT_FILENAME_TEMPLATE: &str = "%(title)s [%(id)s].%(ext)s";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DownloadOptions {
    pub mode: Mode,
    pub quality: Quality,
    pub container: Container,
    pub video_codec: VideoCodec,
    pub audio_format: AudioFormat,
    /// 0 (best) … 10 (worst); only used when re-encoding audio.
    pub audio_quality: u8,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    pub embed_chapters: bool,
    pub subtitles: Subtitles,
    pub auto_subs: bool,
    pub sub_langs: String,
    pub sponsor_block: SponsorBlock,
    pub no_playlist: bool,
    pub fragments: u32,
    pub retries: u32,
    pub rate_limit: String,
    pub cookies_from_browser: Option<Browser>,
    /// Empty = use the folder from settings.
    pub output_folder: String,
    pub filename_template: String,
    pub trim_filenames: Option<u32>,
    pub extra_args: String,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            mode: Mode::Video,
            quality: Quality::Best,
            container: Container::Mp4,
            video_codec: VideoCodec::Any,
            audio_format: AudioFormat::Best,
            audio_quality: 0,
            embed_thumbnail: true,
            embed_metadata: true,
            embed_chapters: true,
            subtitles: Subtitles::None,
            auto_subs: false,
            sub_langs: "en.*".into(),
            sponsor_block: SponsorBlock::Off,
            no_playlist: false,
            fragments: 4,
            retries: 10,
            rate_limit: String::new(),
            cookies_from_browser: None,
            output_folder: String::new(),
            filename_template: DEFAULT_FILENAME_TEMPLATE.into(),
            trim_filenames: Some(180),
            extra_args: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_partial_json_with_defaults() {
        let o: DownloadOptions =
            serde_json::from_str(r#"{"mode":"audio","quality":"720","audioFormat":"mp3"}"#)
                .unwrap();
        assert_eq!(o.mode, Mode::Audio);
        assert_eq!(o.quality, Quality::P720);
        assert_eq!(o.audio_format, AudioFormat::Mp3);
        assert_eq!(o.fragments, 4);
        assert_eq!(o.filename_template, DEFAULT_FILENAME_TEMPLATE);
    }

    #[test]
    fn round_trips_camel_case() {
        let json = serde_json::to_value(DownloadOptions::default()).unwrap();
        assert!(json.get("videoCodec").is_some());
        assert!(json.get("cookiesFromBrowser").is_some());
        assert_eq!(json["quality"], "best");
    }
}
