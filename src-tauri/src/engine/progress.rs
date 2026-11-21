//! Parse the machine-readable lines TubeDeck asks yt-dlp to print, and turn them into
//! job progress. See `engine_args` in `runner.rs` for the templates that produce them.

use super::job::Progress;
use serde::Deserialize;

pub const INFO_PREFIX: &str = "TD_INFO ";
pub const PROG_PREFIX: &str = "TD_PROG ";
pub const PP_PREFIX: &str = "TD_PP ";
pub const FILE_PREFIX: &str = "TD_FILE ";

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct InfoLine {
    pub id: Option<String>,
    pub title: Option<String>,
    pub format_id: Option<String>,
    pub thumbnail: Option<String>,
    pub playlist_index: Option<u64>,
    pub n_entries: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ProgLine {
    pub status: Option<String>,
    pub downloaded_bytes: Option<f64>,
    pub total_bytes: Option<f64>,
    pub total_bytes_estimate: Option<f64>,
    pub speed: Option<f64>,
    pub eta: Option<f64>,
    pub fragment_index: Option<f64>,
    pub fragment_count: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Info(InfoLine),
    Progress {
        format_id: String,
        data: ProgLine,
    },
    PostProcess {
        name: String,
        status: String,
    },
    File(String),
    /// Anything else: goes to the job log.
    Other(String),
}

pub fn parse_line(raw: &str) -> Line {
    let line = raw.trim_end_matches(['\r', '\n']);
    if let Some(rest) = line.strip_prefix(INFO_PREFIX)
        && let Ok(info) = serde_json::from_str::<InfoLine>(rest)
    {
        return Line::Info(info);
    }
    if let Some(rest) = line.strip_prefix(PROG_PREFIX)
        && let Some((fmt, json)) = rest.split_once(' ')
        && let Ok(data) = serde_json::from_str::<ProgLine>(json)
    {
        return Line::Progress {
            format_id: fmt.to_string(),
            data,
        };
    }
    if let Some(rest) = line.strip_prefix(PP_PREFIX)
        && let Some((name, status)) = rest.split_once('|')
    {
        return Line::PostProcess {
            name: name.trim().to_string(),
            status: status.trim().to_string(),
        };
    }
    if let Some(rest) = line.strip_prefix(FILE_PREFIX).map(str::trim)
        && !rest.is_empty()
        && rest != "NA"
    {
        return Line::File(rest.to_string());
    }
    Line::Other(line.to_string())
}

/// Friendly name for a yt-dlp postprocessor.
pub fn postprocessor_label(name: &str) -> String {
    match name {
        "Merger" | "FFmpegMerger" => "Merging formats".into(),
        "ExtractAudio" | "FFmpegExtractAudio" => "Extracting audio".into(),
        "EmbedThumbnail" => "Embedding thumbnail".into(),
        "FFmpegMetadata" | "Metadata" => "Writing metadata".into(),
        "FFmpegEmbedSubtitle" | "EmbedSubtitle" => "Embedding subtitles".into(),
        "SponsorBlock" | "ModifyChapters" | "FFmpegSplitChapters" => "Applying chapters".into(),
        "FFmpegVideoConvertor" | "FFmpegVideoRemuxer" | "VideoConvertor" | "VideoRemuxer" => {
            "Converting".into()
        }
        "FFmpegThumbnailsConvertor" | "ThumbnailsConvertor" => "Converting thumbnail".into(),
        "MoveFiles" | "MoveFilesAfterDownload" => "Moving file".into(),
        "FixupM3u8"
        | "FFmpegFixupM3u8"
        | "FixupM4a"
        | "FFmpegFixupM4a"
        | "FixupStretched"
        | "FFmpegFixupStretched"
        | "FixupDuration"
        | "FFmpegFixupDuration"
        | "FixupTimestamp"
        | "FFmpegFixupTimestamp" => "Fixing container".into(),
        other => format!("Post-processing ({other})"),
    }
}

/// Tracks multi-stream (video + audio) and playlist position to compute overall progress.
#[derive(Debug, Default)]
pub struct Tracker {
    streams: usize,
    stream_index: usize,
    current_format: Option<String>,
    playlist: Option<(u64, u64)>,
}

impl Tracker {
    /// A new video is about to download.
    pub fn on_info(&mut self, info: &InfoLine) {
        self.streams = info
            .format_id
            .as_deref()
            .filter(|f| *f != "NA" && !f.is_empty())
            .map(|f| f.split('+').count())
            .unwrap_or(1)
            .max(1);
        self.stream_index = 0;
        self.current_format = None;
        self.playlist = match (info.playlist_index, info.n_entries) {
            (Some(i), Some(n)) if n > 1 => Some((i, n)),
            _ => None,
        };
    }

    pub fn on_progress(&mut self, format_id: &str, p: &ProgLine) -> (Progress, Option<String>) {
        let known_format = format_id != "NA" && !format_id.is_empty();
        if known_format {
            match &self.current_format {
                Some(cur) if cur != format_id => {
                    self.stream_index = (self.stream_index + 1).min(self.streams.max(1) - 1);
                    self.current_format = Some(format_id.to_string());
                }
                None => self.current_format = Some(format_id.to_string()),
                _ => {}
            }
        }

        let total = p
            .total_bytes
            .or(p.total_bytes_estimate)
            .filter(|t| *t > 0.0);
        let finished = p.status.as_deref() == Some("finished");
        let fraction = if finished {
            Some(1.0)
        } else if let (Some(d), Some(t)) = (p.downloaded_bytes, total) {
            Some((d / t).clamp(0.0, 1.0))
        } else if let (Some(i), Some(n)) = (p.fragment_index, p.fragment_count.filter(|n| *n > 0.0))
        {
            Some((i / n).clamp(0.0, 1.0))
        } else {
            None
        };

        let streams = self.streams.max(1) as f64;
        let percent =
            fraction.map(|f| ((self.stream_index as f64 + f) / streams * 100.0).min(100.0));

        let progress = Progress {
            percent,
            downloaded_bytes: p.downloaded_bytes.map(|v| v as u64),
            total_bytes: total.map(|v| v as u64),
            speed: p.speed.filter(|s| *s > 0.0),
            eta: p.eta.filter(|e| *e >= 0.0).map(|e| e as u64),
        };
        (progress, self.stage())
    }

    pub fn stage(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some((i, n)) = self.playlist {
            parts.push(format!("Item {i}/{n}"));
        }
        if self.streams > 1 {
            parts.push(format!("Stream {}/{}", self.stream_index + 1, self.streams));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}

/// Best error message from yt-dlp's stderr: the last `ERROR:` line, else the last line.
pub fn error_message(stderr_tail: &[String]) -> Option<String> {
    stderr_tail
        .iter()
        .rev()
        .find_map(|l| l.strip_prefix("ERROR: ").map(str::to_string))
        .or_else(|| {
            stderr_tail
                .iter()
                .rev()
                .find(|l| !l.trim().is_empty())
                .cloned()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_all_line_kinds() {
        let info = parse_line(
            r#"TD_INFO {"id": "jNQXAC9IVRw", "title": "Me at the zoo", "format_id": "137+140", "thumbnail": "https://i.ytimg.com/x.jpg"}"#,
        );
        match info {
            Line::Info(i) => {
                assert_eq!(i.title.as_deref(), Some("Me at the zoo"));
                assert_eq!(i.format_id.as_deref(), Some("137+140"));
            }
            other => panic!("unexpected {other:?}"),
        }

        let prog = parse_line(
            r#"TD_PROG 137 {"status": "downloading", "downloaded_bytes": 1024, "total_bytes": 4096, "speed": 512.5, "eta": 6}"#,
        );
        assert_eq!(
            prog,
            Line::Progress {
                format_id: "137".into(),
                data: ProgLine {
                    status: Some("downloading".into()),
                    downloaded_bytes: Some(1024.0),
                    total_bytes: Some(4096.0),
                    speed: Some(512.5),
                    eta: Some(6.0),
                    ..Default::default()
                }
            }
        );

        assert_eq!(
            parse_line("TD_PP Merger|started\r\n"),
            Line::PostProcess {
                name: "Merger".into(),
                status: "started".into()
            }
        );
        assert_eq!(
            parse_line(r"TD_FILE C:\Users\me\Downloads\Me at the zoo [jNQXAC9IVRw].mp4"),
            Line::File(r"C:\Users\me\Downloads\Me at the zoo [jNQXAC9IVRw].mp4".into())
        );
        assert_eq!(parse_line("TD_FILE NA"), Line::Other("TD_FILE NA".into()));
        assert_eq!(
            parse_line("[youtube] Extracting URL"),
            Line::Other("[youtube] Extracting URL".into())
        );
        // Malformed JSON falls back to a log line instead of being lost.
        assert!(matches!(parse_line("TD_PROG 137 {broken"), Line::Other(_)));
    }

    #[test]
    fn null_fields_are_tolerated() {
        let l = parse_line(
            r#"TD_PROG NA {"status": "downloading", "downloaded_bytes": 10, "total_bytes": null, "speed": null, "eta": null, "fragment_index": 3, "fragment_count": 12}"#,
        );
        let Line::Progress { data, .. } = l else {
            panic!()
        };
        let mut t = Tracker::default();
        t.on_info(&InfoLine::default());
        let (p, stage) = t.on_progress("NA", &data);
        assert_eq!(p.percent, Some(25.0));
        assert_eq!(p.total_bytes, None);
        assert_eq!(p.speed, None);
        assert_eq!(stage, None);
    }

    #[test]
    fn two_streams_combine_into_overall_percent() {
        let mut t = Tracker::default();
        t.on_info(&InfoLine {
            format_id: Some("137+140".into()),
            ..Default::default()
        });

        let half = ProgLine {
            downloaded_bytes: Some(50.0),
            total_bytes: Some(100.0),
            ..Default::default()
        };
        let (p, stage) = t.on_progress("137", &half);
        assert_eq!(p.percent, Some(25.0));
        assert_eq!(stage.as_deref(), Some("Stream 1/2"));

        let done = ProgLine {
            status: Some("finished".into()),
            ..Default::default()
        };
        assert_eq!(t.on_progress("137", &done).0.percent, Some(50.0));

        let (p, stage) = t.on_progress("140", &half);
        assert_eq!(p.percent, Some(75.0));
        assert_eq!(stage.as_deref(), Some("Stream 2/2"));

        // A third format id never pushes past the last stream.
        assert_eq!(t.on_progress("999", &done).0.percent, Some(100.0));
    }

    #[test]
    fn playlist_stage_and_reset_per_item() {
        let mut t = Tracker::default();
        t.on_info(&InfoLine {
            format_id: Some("22".into()),
            playlist_index: Some(3),
            n_entries: Some(10),
            ..Default::default()
        });
        let p = ProgLine {
            total_bytes_estimate: Some(200.0),
            downloaded_bytes: Some(50.0),
            ..Default::default()
        };
        let (prog, stage) = t.on_progress("22", &p);
        assert_eq!(prog.percent, Some(25.0));
        assert_eq!(prog.total_bytes, Some(200));
        assert_eq!(stage.as_deref(), Some("Item 3/10"));
    }

    #[test]
    fn picks_error_line() {
        let lines = vec![
            "WARNING: something minor".to_string(),
            "ERROR: [youtube] abc: Video unavailable".to_string(),
            "".to_string(),
        ];
        assert_eq!(
            error_message(&lines).as_deref(),
            Some("[youtube] abc: Video unavailable")
        );
        assert_eq!(
            error_message(&["boom".to_string()]).as_deref(),
            Some("boom")
        );
        assert_eq!(error_message(&[]), None);
    }

    /// Real output captured from yt-dlp 2026.10.26 with TubeDeck's templates
    /// (stdout lines, then the stderr lines; TD_PP arrives on stderr).
    #[test]
    fn replays_real_ytdlp_output() {
        let fixture = include_str!("fixtures/me-at-the-zoo.txt");
        let mut t = Tracker::default();
        let mut last_percent = None;
        let mut stages = Vec::new();
        let mut file = None;
        for raw in fixture.lines() {
            match parse_line(raw) {
                Line::Info(i) => {
                    assert_eq!(i.title.as_deref(), Some("Me at the zoo"));
                    t.on_info(&i);
                }
                Line::Progress { format_id, data } => {
                    let (p, stage) = t.on_progress(&format_id, &data);
                    if let Some(prev) = last_percent {
                        assert!(p.percent.unwrap() >= prev, "progress went backwards");
                    }
                    last_percent = p.percent;
                    stages.push(stage.unwrap());
                }
                Line::PostProcess { name, status } if status == "started" => {
                    stages.push(postprocessor_label(&name))
                }
                Line::PostProcess { .. } => {}
                Line::File(f) => file = Some(f),
                Line::Other(o) => panic!("unparsed fixture line: {o}"),
            }
        }
        assert_eq!(last_percent, Some(100.0));
        assert_eq!(
            stages,
            vec![
                "Stream 1/2",
                "Stream 1/2",
                "Stream 2/2",
                "Stream 2/2",
                "Merging formats",
                "Writing metadata",
                "Embedding thumbnail",
                "Moving file",
            ]
        );
        assert!(file.unwrap().ends_with("Me at the zoo [jNQXAC9IVRw].mp4"));
    }

    #[test]
    fn labels_postprocessors() {
        assert_eq!(postprocessor_label("Merger"), "Merging formats");
        assert_eq!(postprocessor_label("Unknown"), "Post-processing (Unknown)");
    }
}
