//! Classify yt-dlp failures into actionable kinds and decide whether to retry automatically.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Network,
    Http403,
    RateLimited,
    ServerError,
    NotFound,
    BotCheck,
    AgeRestricted,
    Unavailable,
    GeoBlocked,
    Unsupported,
    FfmpegMissing,
    JsRuntime,
    DiskFull,
    Unknown,
}

impl ErrorKind {
    /// Failures that often succeed when simply tried again a bit later.
    pub fn is_transient(self) -> bool {
        matches!(
            self,
            ErrorKind::Network
                | ErrorKind::Http403
                | ErrorKind::RateLimited
                | ErrorKind::ServerError
        )
    }

    /// A short, actionable explanation for the UI.
    pub fn hint(self) -> Option<&'static str> {
        Some(match self {
            ErrorKind::Network => "Network problem. TubeDeck retries automatically.",
            ErrorKind::Http403 => {
                "The site refused the download. This is usually temporary; if it keeps happening, update yt-dlp."
            }
            ErrorKind::RateLimited => {
                "The site is rate-limiting you. Lower the number of simultaneous downloads or wait a while."
            }
            ErrorKind::ServerError => "The site had a server error. Try again later.",
            ErrorKind::NotFound => "The page was not found (HTTP 404). Check the URL.",
            ErrorKind::BotCheck => {
                "YouTube wants a sign-in check. Set \"Cookies from browser\" (Firefox works best) in Configure → Advanced."
            }
            ErrorKind::AgeRestricted => {
                "Age-restricted video. Use \"Cookies from browser\" with a signed-in account."
            }
            ErrorKind::Unavailable => "The video is private, removed or otherwise unavailable.",
            ErrorKind::GeoBlocked => "This video is not available in your region.",
            ErrorKind::Unsupported => "yt-dlp doesn't support this URL.",
            ErrorKind::FfmpegMissing => {
                "FFmpeg is required for this download. Install it or set its path in Settings → Engine."
            }
            ErrorKind::JsRuntime => {
                "YouTube's JavaScript challenge failed. Install Deno 2.3+ (Settings → Engine) and update yt-dlp."
            }
            ErrorKind::DiskFull => {
                "The disk is full. Free up space or choose another output folder."
            }
            ErrorKind::Unknown => return None,
        })
    }
}

/// Classify from yt-dlp's stderr tail. `ERROR:` lines are checked first, newest first.
pub fn classify(lines: &[String]) -> ErrorKind {
    let errors = lines.iter().rev().filter(|l| l.starts_with("ERROR"));
    let rest = lines.iter().rev().filter(|l| !l.starts_with("ERROR"));
    errors
        .chain(rest)
        .map(|l| classify_line(&l.to_lowercase()))
        .find(|k| *k != ErrorKind::Unknown)
        .unwrap_or(ErrorKind::Unknown)
}

fn classify_line(l: &str) -> ErrorKind {
    let has = |needles: &[&str]| needles.iter().any(|n| l.contains(n));
    if l.contains("confirm you") && l.contains("not a bot") {
        ErrorKind::BotCheck
    } else if has(&[
        "confirm your age",
        "age-restricted",
        "inappropriate for some users",
    ]) {
        ErrorKind::AgeRestricted
    } else if has(&["http error 403"]) {
        ErrorKind::Http403
    } else if has(&["http error 429", "too many requests"]) {
        ErrorKind::RateLimited
    } else if has(&[
        "http error 500",
        "http error 502",
        "http error 503",
        "http error 504",
    ]) {
        ErrorKind::ServerError
    } else if has(&["http error 404"]) {
        ErrorKind::NotFound
    } else if has(&[
        "not available in your country",
        "geo restricted",
        "geo-restricted",
        "geo restriction",
    ]) {
        ErrorKind::GeoBlocked
    } else if has(&[
        "private video",
        "video unavailable",
        "this video is unavailable",
        "has been removed",
        "has been terminated",
        "members-only",
    ]) {
        ErrorKind::Unavailable
    } else if has(&["unsupported url"]) {
        ErrorKind::Unsupported
    } else if has(&[
        "ffmpeg not found",
        "ffprobe and ffmpeg not found",
        "ffmpeg is not installed",
    ]) {
        ErrorKind::FfmpegMissing
    } else if has(&[
        "javascript runtime",
        "challenge solving failed",
        "signature solving failed",
    ]) {
        ErrorKind::JsRuntime
    } else if has(&["no space left on device", "errno 28"]) {
        ErrorKind::DiskFull
    } else if has(&[
        "timed out",
        "connection reset",
        "connection refused",
        "connection aborted",
        "getaddrinfo failed",
        "name resolution",
        "name or service not known",
        "network is unreachable",
        "incompleteread",
        "remote end closed connection",
        "unable to download webpage",
        "failed to resolve",
        "unable to connect",
    ]) {
        ErrorKind::Network
    } else {
        ErrorKind::Unknown
    }
}

const BACKOFF_SECS: [u64; 5] = [5, 20, 60, 120, 300];

/// Delay before the next automatic retry, or `None` if the job should fail now.
/// `attempts` is the number of automatic retries already made.
pub fn retry_delay(kind: ErrorKind, attempts: u32, max_attempts: u32) -> Option<Duration> {
    if !kind.is_transient() || attempts >= max_attempts {
        return None;
    }
    let base = BACKOFF_SECS[(attempts as usize).min(BACKOFF_SECS.len() - 1)];
    let secs = if kind == ErrorKind::RateLimited {
        base * 3
    } else {
        base
    };
    Some(Duration::from_secs(secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(lines: &[&str]) -> ErrorKind {
        classify(&lines.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn classifies_common_errors() {
        assert_eq!(
            kind(&["ERROR: unable to download video data: HTTP Error 403: Forbidden"]),
            ErrorKind::Http403
        );
        assert_eq!(
            kind(&["ERROR: [youtube] zzz: This video is unavailable"]),
            ErrorKind::Unavailable
        );
        assert_eq!(
            kind(&[
                "ERROR: [youtube] x: Sign in to confirm you’re not a bot. Use --cookies-from-browser"
            ]),
            ErrorKind::BotCheck
        );
        assert_eq!(
            kind(&["ERROR: [youtube] x: Sign in to confirm your age."]),
            ErrorKind::AgeRestricted
        );
        assert_eq!(
            kind(&["ERROR: Unsupported URL: https://example.com/"]),
            ErrorKind::Unsupported
        );
        assert_eq!(
            kind(&[
                "ERROR: [generic] Unable to download webpage: <urlopen error [Errno 11001] getaddrinfo failed> (caused by TransportError)"
            ]),
            ErrorKind::Network
        );
        assert_eq!(
            kind(&["ERROR: Postprocessing: ffprobe and ffmpeg not found. Please install"]),
            ErrorKind::FfmpegMissing
        );
        assert_eq!(
            kind(&["ERROR: unable to write data: [Errno 28] No space left on device"]),
            ErrorKind::DiskFull
        );
        assert_eq!(
            kind(&["ERROR: HTTP Error 429: Too Many Requests"]),
            ErrorKind::RateLimited
        );
        assert_eq!(
            kind(&["ERROR: HTTP Error 503: Service Unavailable"]),
            ErrorKind::ServerError
        );
        assert_eq!(kind(&["ERROR: something brand new"]), ErrorKind::Unknown);
        assert_eq!(kind(&[]), ErrorKind::Unknown);
    }

    #[test]
    fn error_lines_win_over_warnings() {
        assert_eq!(
            kind(&[
                "WARNING: [youtube] Unable to download webpage: timed out",
                "ERROR: [youtube] abc: Private video"
            ]),
            ErrorKind::Unavailable
        );
        // A warning still helps when the ERROR line itself is generic.
        assert_eq!(
            kind(&["WARNING: Connection reset by peer", "ERROR: giving up"]),
            ErrorKind::Network
        );
    }

    #[test]
    fn retry_policy() {
        assert_eq!(
            retry_delay(ErrorKind::Network, 0, 3),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            retry_delay(ErrorKind::Http403, 1, 3),
            Some(Duration::from_secs(20))
        );
        assert_eq!(
            retry_delay(ErrorKind::RateLimited, 0, 3),
            Some(Duration::from_secs(15))
        );
        assert_eq!(retry_delay(ErrorKind::Network, 3, 3), None);
        assert_eq!(retry_delay(ErrorKind::Network, 0, 0), None);
        assert_eq!(retry_delay(ErrorKind::Unavailable, 0, 3), None);
        assert_eq!(retry_delay(ErrorKind::Unknown, 0, 3), None);
        assert_eq!(
            retry_delay(ErrorKind::ServerError, 9, 10),
            Some(Duration::from_secs(300))
        );
    }

    #[test]
    fn every_known_kind_has_a_hint() {
        for k in [
            ErrorKind::Network,
            ErrorKind::BotCheck,
            ErrorKind::DiskFull,
            ErrorKind::JsRuntime,
        ] {
            assert!(k.hint().is_some());
        }
        assert!(ErrorKind::Unknown.hint().is_none());
    }
}
