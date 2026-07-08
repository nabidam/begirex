//! Pure parsing of yt-dlp `--newline` stdout lines into progress ticks
//! (ARCHITECTURE §5.2). No I/O — takes a `&str`, returns a typed result.
//!
//! ponytail: we parse yt-dlp's default human-readable `--newline` output
//! (e.g. `[download]  12.3% of  2.72MiB at  5.30MiB/s ETA 00:00`) rather than
//! a custom `--progress-template`. Captured real lines in this sandbox
//! (`yt-dlp --newline <url>`) confirm the format below; a template would be
//! more machine-friendly but this needs zero extra CLI args and yt-dlp's
//! default format has been stable for years. Upgrade path if this ever
//! breaks on a yt-dlp version bump: switch to `--progress-template` with a
//! delimited format and a simpler split-based parser.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Downloading,
    Merging,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProgressTick {
    pub percent: f64,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub speed_bps: Option<i64>,
    pub eta_seconds: Option<i64>,
    pub stage: Stage,
}

/// Parses one stdout line from a running `yt-dlp --newline` process.
/// Returns `None` for lines that carry no progress info (extractor/info
/// lines, blank lines, etc).
///
/// Real captured fixtures used by the unit tests below (from an actual
/// `yt-dlp --newline` run against a real small video in this sandbox):
/// `[download]   0.5% of  218.53KiB at   43.99KiB/s ETA 00:04`
/// `[download] 100% of  218.53KiB in 00:00:00 at 266.58KiB/s` (final line, no ETA)
/// `[Merger] Merging formats into "test2.webm"`
pub fn parse_line(line: &str) -> Option<ProgressTick> {
    let line = line.trim();

    if line.starts_with("[Merger]") || line.starts_with("[ffmpeg] Merging") {
        return Some(ProgressTick {
            percent: 100.0,
            downloaded_bytes: None,
            total_bytes: None,
            speed_bps: None,
            eta_seconds: None,
            stage: Stage::Merging,
        });
    }

    let rest = line.strip_prefix("[download]")?.trim();

    // Two shapes seen in real output:
    //   "12.3% of  2.72MiB at  5.30MiB/s ETA 00:00"
    //   "100% of  218.53KiB in 00:00:00 at 266.58KiB/s"   (no ETA, has "in" instead of a leading percent-at)
    let percent_str = rest.split('%').next()?.trim();
    let percent: f64 = percent_str.parse().ok()?;

    let after_percent = rest.splitn(2, '%').nth(1)?.trim();
    let after_of = after_percent.strip_prefix("of")?.trim();

    // Split the "<size> at/in ..." remainder on whichever of " at "/" in "
    // comes first — the two real shapes are "of SIZE at SPEED ETA TIME" and
    // "of SIZE in TIME at SPEED" (final line), so picking " at " unconditionally
    // (as an earlier version of this code did) breaks the "in" shape.
    let at_idx = after_of.find(" at ");
    let in_idx = after_of.find(" in ");
    let (size_str, tail) = match (in_idx, at_idx) {
        (Some(i), Some(a)) if i < a => (&after_of[..i], after_of[i + 4..].trim()),
        (Some(i), None) => (&after_of[..i], after_of[i + 4..].trim()),
        (_, Some(a)) => (&after_of[..a], after_of[a + 4..].trim()),
        _ => (after_of, ""),
    };
    let total_bytes = parse_size(size_str.trim());

    // `tail` is either "5.30MiB/s ETA 00:00" (from the " at " shape) or
    // "00:00:00 at 266.58KiB/s" (from the " in " shape — time then speed).
    let (speed_str, eta_str) = if let Some(idx) = tail.find(" at ") {
        (tail[idx + 4..].trim(), None)
    } else if let Some(idx) = tail.find("ETA") {
        (tail[..idx].trim(), Some(tail[idx + 3..].trim()))
    } else {
        (tail, None)
    };
    let speed_bps = parse_speed(speed_str);
    let eta_seconds = eta_str.and_then(parse_eta);

    let downloaded_bytes = total_bytes.map(|t| ((percent / 100.0) * t as f64).round() as i64);

    Some(ProgressTick {
        percent,
        downloaded_bytes,
        total_bytes,
        speed_bps,
        eta_seconds,
        stage: Stage::Downloading,
    })
}

/// Parses sizes like "2.72MiB", "218.53KiB", "1.00GiB" into bytes.
fn parse_size(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("unknown") || s.is_empty() {
        return None;
    }
    let (num_part, unit) = split_number_unit(s)?;
    let multiplier: f64 = match unit.to_ascii_uppercase().as_str() {
        "B" => 1.0,
        "KIB" => 1024.0,
        "MIB" => 1024.0 * 1024.0,
        "GIB" => 1024.0 * 1024.0 * 1024.0,
        "TIB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    let num: f64 = num_part.parse().ok()?;
    Some((num * multiplier).round() as i64)
}

/// Parses speeds like "5.30MiB/s", "Unknown B/s" into bytes/sec.
fn parse_speed(s: &str) -> Option<i64> {
    let s = s.trim().strip_suffix("/s")?.trim();
    parse_size(s)
}

fn split_number_unit(s: &str) -> Option<(&str, &str)> {
    let idx = s.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    Some((&s[..idx], &s[idx..]))
}

/// Parses "00:04" / "01:02:03" (or "Unknown") into total seconds.
fn parse_eta(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("unknown") || s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split(':').collect();
    let mut seconds: i64 = 0;
    for part in parts {
        seconds = seconds * 60 + part.parse::<i64>().ok()?;
    }
    Some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_captured_progress_line_with_eta() {
        // Captured verbatim from `yt-dlp --newline` against
        // https://www.youtube.com/watch?v=jNQXAC9IVRw in this sandbox.
        let line = "[download]   0.5% of  218.53KiB at   43.99KiB/s ETA 00:04";
        let tick = parse_line(line).unwrap();
        assert_eq!(tick.stage, Stage::Downloading);
        assert!((tick.percent - 0.5).abs() < 1e-9);
        assert_eq!(tick.total_bytes, Some(223775)); // 218.53 * 1024, rounded
        assert_eq!(tick.speed_bps, Some(45046)); // 43.99 * 1024, rounded
        assert_eq!(tick.eta_seconds, Some(4));
        assert_eq!(tick.downloaded_bytes, Some(1119)); // 0.5% of 223775
    }

    #[test]
    fn parses_real_captured_final_progress_line_without_eta() {
        // Also captured verbatim from the same real run (last line before
        // yt-dlp moves on to the next format / merge phase).
        let line = "[download] 100% of  218.53KiB in 00:00:00 at 266.58KiB/s";
        let tick = parse_line(line).unwrap();
        assert_eq!(tick.stage, Stage::Downloading);
        assert!((tick.percent - 100.0).abs() < 1e-9);
        assert_eq!(tick.total_bytes, Some(223775));
        assert_eq!(tick.eta_seconds, None);
        assert_eq!(tick.speed_bps, Some(272978)); // 266.58 * 1024, rounded
    }

    #[test]
    fn merge_phase_line_maps_to_merging_stage() {
        // Captured verbatim: real yt-dlp run that needed a bv+ba merge.
        let line = "[Merger] Merging formats into \"test2.webm\"";
        let tick = parse_line(line).unwrap();
        assert_eq!(tick.stage, Stage::Merging);
        assert!((tick.percent - 100.0).abs() < 1e-9);
    }

    #[test]
    fn non_progress_lines_return_none() {
        assert!(parse_line("[youtube] Extracting URL: https://...").is_none());
        assert!(parse_line("").is_none());
        assert!(parse_line("[info] jNQXAC9IVRw: Downloading 1 format(s): 395+251").is_none());
    }

    #[test]
    fn unknown_speed_and_eta_parse_to_none() {
        let line = "[download]   0.0% of    2.72MiB at  Unknown B/s ETA Unknown";
        let tick = parse_line(line).unwrap();
        assert_eq!(tick.speed_bps, None);
        assert_eq!(tick.eta_seconds, None);
        assert_eq!(tick.total_bytes, Some(2852127));
    }
}
