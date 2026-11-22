//! Single source of truth for turning [`DownloadOptions`] into yt-dlp arguments.
//!
//! The live command preview and the download engine both call [`user_args`], so what the
//! user sees is exactly what runs. The engine (M2) adds its own bookkeeping arguments
//! (progress template, temp dir, binary locations) on top of these.

use crate::options::*;
use serde::Serialize;

pub const PROGRAM: &str = "yt-dlp";
pub const URL_PLACEHOLDER: &str = "<URL>";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Warn,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Warning {
    pub level: Level,
    pub message: String,
}

impl Warning {
    fn new(level: Level, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltCommand {
    pub program: String,
    pub args: Vec<String>,
    /// Copy-pasteable PowerShell command line.
    pub display: String,
    pub warnings: Vec<Warning>,
}

/// Validate a single download URL. Only http(s) is accepted.
pub fn validate_url(url: &str) -> Result<(), String> {
    let u = url.trim();
    if u.is_empty() {
        return Err("URL cannot be empty".into());
    }
    let lower = u.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err("Only http:// and https:// URLs are supported".into());
    }
    if u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("URL contains whitespace or control characters".into());
    }
    Ok(())
}

/// Build the full command for `url` (or a placeholder when `url` is empty).
pub fn build(url: &str, opts: &DownloadOptions, default_folder: &str) -> BuiltCommand {
    let mut warnings = Vec::new();
    let url = url.trim();
    let url_arg = if url.is_empty() {
        URL_PLACEHOLDER.to_string()
    } else {
        if let Err(e) = validate_url(url) {
            warnings.push(Warning::new(Level::Danger, e));
        }
        url.to_string()
    };

    let mut args = user_args(opts, default_folder, &mut warnings);
    args.push("--".into());
    args.push(url_arg);

    let display = powershell_display(PROGRAM, &args);
    BuiltCommand {
        program: PROGRAM.into(),
        args,
        display,
        warnings,
    }
}

/// Arguments derived from the user's options, without the trailing `-- <url>`.
pub fn user_args(
    opts: &DownloadOptions,
    default_folder: &str,
    warnings: &mut Vec<Warning>,
) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let push = |a: &mut Vec<String>, flag: &str, value: Option<&str>| {
        a.push(flag.to_string());
        if let Some(v) = value {
            a.push(v.to_string());
        }
    };

    // --- Format selection -------------------------------------------------------------
    match opts.mode {
        Mode::Video => {
            push(&mut a, "-f", Some("bv*+ba/b"));
            let mut sort: Vec<String> = Vec::new();
            if let Some(h) = opts.quality.height() {
                sort.push(format!("res:{h}"));
            }
            if let Some(vc) = opts.video_codec.sort_value() {
                sort.push(format!("vcodec:{vc}"));
            }
            if opts.container == Container::Mp4 {
                sort.push("ext:mp4:m4a".into());
            }
            if !sort.is_empty() {
                push(&mut a, "-S", Some(&sort.join(",")));
            }
            push(
                &mut a,
                "--merge-output-format",
                Some(opts.container.as_str()),
            );

            if opts.container == Container::Webm && opts.video_codec == VideoCodec::H264 {
                warnings.push(Warning::new(
                    Level::Warn,
                    "WebM cannot hold H.264 video; yt-dlp will pick another codec or re-mux may fail. Use MP4 or MKV for H.264.",
                ));
            }
        }
        Mode::Audio => {
            push(&mut a, "-f", Some("ba/b"));
            push(&mut a, "-x", None);
            if opts.audio_format != AudioFormat::Best {
                push(&mut a, "--audio-format", Some(opts.audio_format.as_str()));
            }
            if matches!(
                opts.audio_format,
                AudioFormat::Mp3 | AudioFormat::M4a | AudioFormat::Opus
            ) {
                let q = opts.audio_quality.min(10);
                push(&mut a, "--audio-quality", Some(&q.to_string()));
            }
        }
    }

    // --- Embedding --------------------------------------------------------------------
    if opts.embed_thumbnail {
        let unsupported = match opts.mode {
            Mode::Video => opts.container == Container::Webm,
            Mode::Audio => opts.audio_format == AudioFormat::Wav,
        };
        if unsupported {
            warnings.push(Warning::new(
                Level::Info,
                "Thumbnail embedding isn't supported for this container; skipped.",
            ));
        } else {
            push(&mut a, "--embed-thumbnail", None);
        }
    }
    if opts.embed_metadata {
        push(&mut a, "--embed-metadata", None);
    }
    if opts.embed_chapters {
        push(&mut a, "--embed-chapters", None);
    }

    // --- Subtitles --------------------------------------------------------------------
    if opts.subtitles != Subtitles::None {
        let langs = opts.sub_langs.trim();
        let langs_ok = !langs.is_empty()
            && langs
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".*,-_".contains(c));
        let langs = if langs_ok {
            langs
        } else {
            warnings.push(Warning::new(
                Level::Warn,
                "Subtitle languages look invalid; using \"en.*\". Example: en.*,nl",
            ));
            "en.*"
        };

        let embed_in_audio = opts.subtitles == Subtitles::Embed && opts.mode == Mode::Audio;
        if embed_in_audio {
            warnings.push(Warning::new(
                Level::Info,
                "Subtitles can only be embedded in video files; skipped in audio mode.",
            ));
        } else {
            push(&mut a, "--write-subs", None);
            if opts.auto_subs {
                push(&mut a, "--write-auto-subs", None);
            }
            push(&mut a, "--sub-langs", Some(langs));
            if opts.subtitles == Subtitles::Embed {
                push(&mut a, "--embed-subs", None);
            }
        }
    }

    // --- SponsorBlock -----------------------------------------------------------------
    match opts.sponsor_block {
        SponsorBlock::Off => {}
        SponsorBlock::Mark => push(&mut a, "--sponsorblock-mark", Some("all")),
        SponsorBlock::Remove => push(&mut a, "--sponsorblock-remove", Some("default")),
    }

    // --- Playlist ---------------------------------------------------------------------
    if opts.no_playlist {
        push(&mut a, "--no-playlist", None);
    }

    // --- Network ----------------------------------------------------------------------
    let fragments = opts.fragments.clamp(1, 16);
    if fragments > 1 {
        push(
            &mut a,
            "--concurrent-fragments",
            Some(&fragments.to_string()),
        );
    }
    let retries = opts.retries.min(100);
    if retries != 10 {
        push(&mut a, "--retries", Some(&retries.to_string()));
    }
    let rate = opts.rate_limit.trim();
    if !rate.is_empty() {
        if is_valid_rate(rate) {
            push(&mut a, "--limit-rate", Some(rate));
        } else {
            warnings.push(Warning::new(
                Level::Warn,
                format!("Rate limit \"{rate}\" is not valid (examples: 500K, 4.2M); ignored."),
            ));
        }
    }
    if let Some(browser) = opts.cookies_from_browser {
        push(&mut a, "--cookies-from-browser", Some(browser.as_str()));
        if browser != Browser::Firefox {
            warnings.push(Warning::new(
                Level::Info,
                "Reading cookies from Chromium-based browsers often fails on Windows while the browser is open. Firefox is the most reliable.",
            ));
        }
    }

    // --- Output -----------------------------------------------------------------------
    let folder = if opts.output_folder.trim().is_empty() {
        default_folder.trim()
    } else {
        opts.output_folder.trim()
    };
    if folder.is_empty() {
        warnings.push(Warning::new(
            Level::Warn,
            "No output folder set; files would go to the current directory.",
        ));
    } else {
        push(&mut a, "-P", Some(folder));
    }

    let template = opts.filename_template.trim();
    let template = if template.is_empty() {
        DEFAULT_FILENAME_TEMPLATE
    } else {
        template
    };
    if looks_absolute(template) {
        warnings.push(Warning::new(
            Level::Warn,
            "The filename template is an absolute path, so the output folder is ignored.",
        ));
    }
    push(&mut a, "-o", Some(template));
    if let Some(n) = opts.trim_filenames.filter(|n| *n > 0) {
        push(&mut a, "--trim-filenames", Some(&n.to_string()));
    }

    // --- Custom arguments -------------------------------------------------------------
    match split_args(&opts.extra_args) {
        Ok(extra) => {
            warnings.extend(check_extra_args(&extra));
            a.extend(extra);
        }
        Err(e) => warnings.push(Warning::new(
            Level::Danger,
            format!("Custom arguments not applied: {e}"),
        )),
    }

    a
}

fn is_valid_rate(s: &str) -> bool {
    let num = s.strip_suffix(['K', 'M', 'G', 'k', 'm', 'g']).unwrap_or(s);
    !num.is_empty()
        && num.chars().all(|c| c.is_ascii_digit() || c == '.')
        && num.chars().filter(|c| *c == '.').count() <= 1
        && num.parse::<f64>().map(|v| v > 0.0).unwrap_or(false)
}

fn looks_absolute(t: &str) -> bool {
    let b = t.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
        || t.starts_with("\\\\")
        || t.starts_with('/')
}

/// Split a user-typed argument string. Whitespace separates arguments; `"..."` and `'...'`
/// group. Backslashes are literal so Windows paths like `C:\tools\ffmpeg` survive.
pub fn split_args(input: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;

    for c in input.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                in_token = true;
            }
            None if c.is_whitespace() => {
                if in_token {
                    out.push(std::mem::take(&mut cur));
                    in_token = false;
                }
            }
            None => {
                cur.push(c);
                in_token = true;
            }
        }
    }
    if let Some(q) = quote {
        return Err(format!("unclosed {q} quote"));
    }
    if in_token {
        out.push(cur);
    }
    Ok(out)
}

/// Flags that make yt-dlp run arbitrary programs or load code.
const DANGEROUS: &[&str] = &[
    "--exec",
    "--exec-before-download",
    "--netrc-cmd",
    "--plugin-dirs",
    "--config-locations",
];

/// Flags TubeDeck manages itself; overriding them breaks the queue or progress display.
const MANAGED: &[&str] = &[
    "-o",
    "--output",
    "-P",
    "--paths",
    "-O",
    "--print",
    "--print-to-file",
    "--progress-template",
    "-q",
    "--quiet",
    "--no-progress",
    "--newline",
    "-s",
    "--simulate",
    "--skip-download",
    "-j",
    "--dump-json",
    "-J",
    "--dump-single-json",
    "--ffmpeg-location",
    "--js-runtimes",
    "--no-js-runtimes",
    "-a",
    "--batch-file",
    "--encoding",
    "-U",
    "--update",
    "--update-to",
];

fn check_extra_args(args: &[String]) -> Vec<Warning> {
    let mut w = Vec::new();
    for arg in args {
        let flag = arg.split('=').next().unwrap_or(arg);
        if DANGEROUS.contains(&flag) {
            w.push(Warning::new(
                Level::Danger,
                format!("{flag} lets yt-dlp run other programs or load code. Only use it if you wrote it yourself."),
            ));
        } else if MANAGED.contains(&flag) {
            w.push(Warning::new(
                Level::Warn,
                format!(
                    "{flag} is managed by TubeDeck and may break downloads or progress reporting."
                ),
            ));
        } else if arg == "--" {
            w.push(Warning::new(
                Level::Warn,
                "\"--\" in custom arguments would turn the following arguments into URLs.",
            ));
        }
    }
    w
}

/// Quote one argument for PowerShell 7. Plain tokens stay bare; anything else is
/// single-quoted with embedded quotes doubled.
pub fn powershell_quote(arg: &str) -> String {
    let safe = !arg.is_empty()
        && !arg.starts_with('@')
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_+=:./\\".contains(c));
    if safe {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "''"))
    }
}

pub fn powershell_display(program: &str, args: &[String]) -> String {
    let mut s = String::from(program);
    for a in args {
        s.push(' ');
        if a == URL_PLACEHOLDER {
            s.push_str(a);
        } else {
            s.push_str(&powershell_quote(a));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const FOLDER: &str = r"C:\Users\me\Downloads\TubeDeck";

    fn args_of(o: &DownloadOptions) -> (Vec<String>, Vec<Warning>) {
        let mut w = Vec::new();
        (user_args(o, FOLDER, &mut w), w)
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn default_video() {
        let (a, w) = args_of(&DownloadOptions::default());
        assert_eq!(
            a,
            s(&[
                "-f",
                "bv*+ba/b",
                "-S",
                "ext:mp4:m4a",
                "--merge-output-format",
                "mp4",
                "--embed-thumbnail",
                "--embed-metadata",
                "--embed-chapters",
                "--concurrent-fragments",
                "4",
                "-P",
                FOLDER,
                "-o",
                DEFAULT_FILENAME_TEMPLATE,
                "--trim-filenames",
                "180",
            ])
        );
        assert!(w.is_empty());
    }

    #[test]
    fn capped_h264_mkv() {
        let o = DownloadOptions {
            quality: Quality::P1080,
            video_codec: VideoCodec::H264,
            container: Container::Mkv,
            embed_thumbnail: false,
            embed_metadata: false,
            embed_chapters: false,
            fragments: 1,
            trim_filenames: None,
            ..Default::default()
        };
        let (a, _) = args_of(&o);
        assert_eq!(
            a,
            s(&[
                "-f",
                "bv*+ba/b",
                "-S",
                "res:1080,vcodec:h264",
                "--merge-output-format",
                "mkv",
                "-P",
                FOLDER,
                "-o",
                DEFAULT_FILENAME_TEMPLATE,
            ])
        );
    }

    #[test]
    fn audio_mp3() {
        let o = DownloadOptions {
            mode: Mode::Audio,
            audio_format: AudioFormat::Mp3,
            audio_quality: 2,
            embed_chapters: false,
            ..Default::default()
        };
        let (a, _) = args_of(&o);
        assert_eq!(
            &a[..7],
            &s(&[
                "-f",
                "ba/b",
                "-x",
                "--audio-format",
                "mp3",
                "--audio-quality",
                "2"
            ])[..]
        );
        assert!(a.contains(&"--embed-thumbnail".to_string()));
        assert!(!a.contains(&"--merge-output-format".to_string()));
    }

    #[test]
    fn audio_best_has_no_format_or_quality() {
        let o = DownloadOptions {
            mode: Mode::Audio,
            ..Default::default()
        };
        let (a, _) = args_of(&o);
        assert!(!a.contains(&"--audio-format".to_string()));
        assert!(!a.contains(&"--audio-quality".to_string()));
    }

    #[test]
    fn webm_skips_thumbnail_with_info() {
        let o = DownloadOptions {
            container: Container::Webm,
            ..Default::default()
        };
        let (a, w) = args_of(&o);
        assert!(!a.contains(&"--embed-thumbnail".to_string()));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].level, Level::Info);
    }

    #[test]
    fn subtitles_embed_and_auto() {
        let o = DownloadOptions {
            subtitles: Subtitles::Embed,
            auto_subs: true,
            sub_langs: "en.*,nl".into(),
            ..Default::default()
        };
        let (a, _) = args_of(&o);
        let i = a.iter().position(|x| x == "--write-subs").unwrap();
        assert_eq!(
            &a[i..i + 5],
            &s(&[
                "--write-subs",
                "--write-auto-subs",
                "--sub-langs",
                "en.*,nl",
                "--embed-subs"
            ])[..]
        );
    }

    #[test]
    fn invalid_rate_is_ignored() {
        let o = DownloadOptions {
            rate_limit: "fast".into(),
            ..Default::default()
        };
        let (a, w) = args_of(&o);
        assert!(!a.contains(&"--limit-rate".to_string()));
        assert_eq!(w[0].level, Level::Warn);
        assert!(is_valid_rate("500K"));
        assert!(is_valid_rate("4.2M"));
        assert!(is_valid_rate("1000"));
        assert!(!is_valid_rate("1.2.3M"));
        assert!(!is_valid_rate("0"));
    }

    #[test]
    fn empty_folder_falls_back_to_default_or_warns() {
        let mut w = Vec::new();
        let a = user_args(&DownloadOptions::default(), "", &mut w);
        assert!(!a.contains(&"-P".to_string()));
        assert_eq!(w.len(), 1);

        let o = DownloadOptions {
            output_folder: r"D:\Media".into(),
            ..Default::default()
        };
        let (a, _) = args_of(&o);
        let i = a.iter().position(|x| x == "-P").unwrap();
        assert_eq!(a[i + 1], r"D:\Media");
    }

    #[test]
    fn split_keeps_windows_paths_and_quotes() {
        assert_eq!(
            split_args(
                r#"--ffmpeg-location C:\tools\ffmpeg --match-filter "duration > 60" -x 'a b'"#
            )
            .unwrap(),
            s(&[
                "--ffmpeg-location",
                r"C:\tools\ffmpeg",
                "--match-filter",
                "duration > 60",
                "-x",
                "a b"
            ])
        );
        assert_eq!(split_args(r#"--a="" b"#).unwrap(), s(&["--a=", "b"]));
        assert_eq!(split_args("   ").unwrap(), Vec::<String>::new());
        assert!(split_args(r#"--x "open"#).is_err());
    }

    #[test]
    fn extra_args_warnings() {
        let o = DownloadOptions {
            extra_args: r#"--exec "calc" --output=foo.mp4 --no-mtime"#.into(),
            ..Default::default()
        };
        let (a, w) = args_of(&o);
        assert!(a.ends_with(&s(&["--exec", "calc", "--output=foo.mp4", "--no-mtime"])));
        assert_eq!(
            w.iter().map(|x| x.level).collect::<Vec<_>>(),
            vec![Level::Danger, Level::Warn]
        );
    }

    #[test]
    fn unclosed_quote_drops_extra_args() {
        let o = DownloadOptions {
            extra_args: r#"--foo "bar"#.into(),
            ..Default::default()
        };
        let (a, w) = args_of(&o);
        assert!(!a.contains(&"--foo".to_string()));
        assert_eq!(w[0].level, Level::Danger);
    }

    #[test]
    fn build_appends_separator_and_url() {
        let b = build(
            "https://www.youtube.com/watch?v=abc&t=1",
            &DownloadOptions::default(),
            FOLDER,
        );
        assert_eq!(
            &b.args[b.args.len() - 2..],
            &s(&["--", "https://www.youtube.com/watch?v=abc&t=1"])[..]
        );
        assert!(b.display.starts_with("yt-dlp -f 'bv*+ba/b' "));
        assert!(
            b.display
                .ends_with(" -- 'https://www.youtube.com/watch?v=abc&t=1'")
        );
        assert!(b.warnings.is_empty());
    }

    #[test]
    fn build_placeholder_and_bad_url() {
        let b = build("", &DownloadOptions::default(), FOLDER);
        assert!(b.display.ends_with(" -- <URL>"));

        let b = build("file:///etc/passwd", &DownloadOptions::default(), FOLDER);
        assert_eq!(b.warnings[0].level, Level::Danger);
    }

    #[test]
    fn validate_url_rules() {
        assert!(validate_url("https://youtu.be/x").is_ok());
        assert!(validate_url("HTTP://example.com").is_ok());
        assert!(validate_url("").is_err());
        assert!(validate_url("ftp://x").is_err());
        assert!(validate_url("-o evil").is_err());
        assert!(validate_url("https://a b").is_err());
    }

    #[test]
    fn powershell_quoting() {
        assert_eq!(powershell_quote("--embed-metadata"), "--embed-metadata");
        assert_eq!(powershell_quote(r"C:\Users\me"), r"C:\Users\me");
        assert_eq!(powershell_quote(r"C:\My Videos"), r"'C:\My Videos'");
        assert_eq!(
            powershell_quote("res:1080,vcodec:h264"),
            "'res:1080,vcodec:h264'"
        );
        assert_eq!(powershell_quote("it's"), "'it''s'");
        assert_eq!(powershell_quote("@x"), "'@x'");
        assert_eq!(powershell_quote(""), "''");
        assert_eq!(powershell_quote("$(calc)"), "'$(calc)'");
    }
}
