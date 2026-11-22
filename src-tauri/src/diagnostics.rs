//! Plain-text diagnostics report for bug reports. Paths under the user profile are
//! replaced with `%USERPROFILE%` so the report doesn't reveal the Windows user name.

use crate::{
    binaries::{BinaryStatus, EngineStatus},
    engine::job::{Job, JobStatus},
    settings::Settings,
};
use std::fmt::Write;

const MAX_FAILED_JOBS: usize = 3;
const JOB_LOG_TAIL: usize = 60;

pub fn report(
    version: &str,
    engine: &EngineStatus,
    settings: &Settings,
    jobs: &[Job],
    app_log: &[String],
    job_log: impl Fn(&str) -> Vec<String>,
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "TubeDeck diagnostics");
    let _ = writeln!(s, "====================");
    let _ = writeln!(
        s,
        "TubeDeck {version} on {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(s, "Generated (unix ms): {}", crate::engine::job::now_ms());

    let _ = writeln!(s, "\n## Engine");
    for b in [&engine.ytdlp, &engine.ffmpeg, &engine.deno] {
        let _ = writeln!(s, "{}", binary_line(b));
    }

    let _ = writeln!(s, "\n## Settings");
    let mut shown = settings.clone();
    // Not secret, but irrelevant noise for a bug report.
    shown.active_preset_id = None;
    let _ = writeln!(
        s,
        "{}",
        serde_json::to_string_pretty(&shown).unwrap_or_default()
    );

    let _ = writeln!(s, "\n## Queue ({} jobs)", jobs.len());
    for j in jobs {
        let _ = writeln!(
            s,
            "- {} {:?} {:.1}% attempts={} {}{}",
            j.id,
            j.status,
            j.progress.percent.unwrap_or(0.0),
            j.attempts,
            j.url,
            j.error
                .as_deref()
                .map(|e| format!("  ERROR: {e}"))
                .unwrap_or_default()
        );
    }

    let failed: Vec<&Job> = jobs
        .iter()
        .filter(|j| j.status == JobStatus::Failed)
        .rev()
        .take(MAX_FAILED_JOBS)
        .collect();
    for j in failed {
        let _ = writeln!(s, "\n## Log of failed job {} ({})", j.id, j.url);
        let lines = job_log(&j.id);
        for l in &lines[lines.len().saturating_sub(JOB_LOG_TAIL)..] {
            let _ = writeln!(s, "{l}");
        }
    }

    let _ = writeln!(s, "\n## Application log (last {} lines)", app_log.len());
    for l in app_log {
        let _ = writeln!(s, "{l}");
    }

    redact(&s, std::env::var("USERPROFILE").ok().as_deref())
}

fn binary_line(b: &BinaryStatus) -> String {
    format!(
        "{:<7} found={} ok={} version={} source={} path={}{}",
        b.name,
        b.found,
        b.ok,
        b.version.as_deref().unwrap_or("-"),
        b.source
            .map(|s| format!("{s:?}").to_lowercase())
            .unwrap_or_else(|| "-".into()),
        b.path.as_deref().unwrap_or("-"),
        b.note
            .as_deref()
            .map(|n| format!("  note: {n}"))
            .unwrap_or_default(),
    )
}

/// Replace the user profile path (in both `\` and escaped `\\` JSON form) with a placeholder.
pub fn redact(text: &str, user_profile: Option<&str>) -> String {
    let Some(profile) = user_profile.filter(|p| p.len() > 3) else {
        return text.to_string();
    };
    let escaped = profile.replace('\\', "\\\\");
    let out = replace_ascii_ci(text, &escaped, "%USERPROFILE%");
    replace_ascii_ci(&out, profile, "%USERPROFILE%")
}

/// Case-insensitive (ASCII) replace. ASCII lowercasing keeps byte offsets identical, so
/// match positions in the lowercased copy are valid in the original.
fn replace_ascii_ci(haystack: &str, needle: &str, with: &str) -> String {
    let lower = haystack.to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(haystack.len());
    let mut last = 0;
    for (i, _) in lower.match_indices(&needle_lower) {
        out.push_str(&haystack[last..i]);
        out.push_str(with);
        last = i + needle.len();
    }
    out.push_str(&haystack[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_profile_paths() {
        let p = r"C:\Users\Alice";
        let text =
            r#"path=C:\Users\Alice\Downloads x {"f":"C:\\Users\\Alice\\bin"} c:\users\alice\tmp"#;
        let out = redact(text, Some(p));
        assert!(!out.to_lowercase().contains("alice"), "{out}");
        assert!(out.contains(r"%USERPROFILE%\Downloads"));
        assert!(out.contains(r"%USERPROFILE%\\bin"));
        assert!(out.contains(r"%USERPROFILE%\tmp"));
        assert_eq!(redact("nothing here", Some(p)), "nothing here");
        assert_eq!(redact("abc", None), "abc");
    }
}
