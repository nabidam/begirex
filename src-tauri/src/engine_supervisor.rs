//! Spawns one `yt-dlp` child per active item, streams+parses its
//! stdout/stderr into progress, checkpoints it, and emits events
//! (ARCHITECTURE §2, §5). Must NOT decide which item runs next (that's
//! queue_manager/T3) or write UI state directly.

use crate::error::AppError;
use crate::persistence;
use crate::progress_parser::{self, ProgressTick};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex as AsyncMutex;

/// Emits `progress`/`stage_changed` events (ARCHITECTURE §7.3). Kept as a
/// small trait rather than taking `tauri::AppHandle` directly, so this
/// module's core logic is unit-testable without a running Tauri app (T2
/// acceptance criterion 3) — production wires a `tauri::AppHandle`-backed
/// impl in ipc.rs; tests wire an in-memory recorder.
pub trait Emitter: Send + Sync {
    fn emit_progress(&self, item_id: i64, tick: &ProgressTick);
    fn emit_stage_changed(&self, item_id: i64, stage: &str, error_message: Option<&str>);
    fn emit_item_added(&self, item: &persistence::Item);
    fn emit_item_removed(&self, item_id: i64);
    fn emit_log_line(&self, item_id: i64, stream: &str, line: &str);
}

/// Why a still-running child was killed — distinguishes an intentional
/// pause/cancel from a real engine failure once `run_download_inner`'s loop
/// exits, so it doesn't overwrite the pause/cancel command's own DB write
/// with an `error` stage (ARCHITECTURE §2: engine_supervisor owns
/// pause/cancel/resume).
#[derive(Debug, Clone, Copy, PartialEq)]
enum KillReason {
    Paused,
    Cancelled,
}

/// One entry per currently-spawned child (T6 pause/cancel — ARCHITECTURE §2:
/// "hold the child handle"). `child` is behind an async mutex so pause/cancel
/// commands (running on a different task than the download loop) can signal
/// it; `kill_reason` is checked by the download loop itself right before it
/// would otherwise report `completed`/`error`; `partial_paths` collects every
/// "[download] Destination: …" line yt-dlp prints, so cancel can delete the
/// partial file(s) it was writing.
#[derive(Clone)]
pub struct ActiveEntry {
    child: Arc<AsyncMutex<Child>>,
    kill_reason: Arc<Mutex<Option<KillReason>>>,
    partial_paths: Arc<Mutex<Vec<String>>>,
}

/// Registry of currently-spawned children, keyed by item id. Shared between
/// `queue_manager` (which owns one, in `AppState`) and this module's
/// pause/cancel/run_download so a command handler on one task can kill a
/// child a `run_download` task on another task is supervising.
pub type ActiveRegistry = Arc<Mutex<HashMap<i64, ActiveEntry>>>;

/// Kills the running child for `item_id`, marking the kill as a pause so
/// `run_download`'s own exit handling doesn't report it as `error`. Returns
/// `Ok(false)` (a no-op) if the item has no running child — callers only
/// need to kill items currently `downloading`/`merging`.
pub async fn pause(registry: &ActiveRegistry, item_id: i64) -> Result<bool, AppError> {
    kill(registry, item_id, KillReason::Paused).await
}

/// Kills the running child for `item_id` (marking the kill as a cancel) and
/// deletes whatever partial file(s) it was writing. Returns `Ok(false)` if
/// the item has no running child.
pub async fn cancel(registry: &ActiveRegistry, item_id: i64) -> Result<bool, AppError> {
    // Grab a clone of the `Arc` *before* killing — `kill` awaits the child's
    // full exit, and the instant `run_download`'s wrapper observes that exit
    // it removes this item's entry from the registry (on a different task).
    // Looking the entry up again afterward would race that removal and could
    // read back an empty registry with the destination path already gone;
    // holding our own `Arc` to the same underlying `Vec` sidesteps the race
    // entirely — it stays readable regardless of when the entry is dropped.
    let partial_paths = registry
        .lock()
        .unwrap()
        .get(&item_id)
        .map(|e| Arc::clone(&e.partial_paths));

    let killed = kill(registry, item_id, KillReason::Cancelled).await?;
    if killed {
        let paths = partial_paths
            .map(|p| p.lock().unwrap().clone())
            .unwrap_or_default();
        for path in paths {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(format!("{path}.part"));
        }
    }
    Ok(killed)
}

async fn kill(registry: &ActiveRegistry, item_id: i64, reason: KillReason) -> Result<bool, AppError> {
    let entry = registry.lock().unwrap().get(&item_id).cloned();
    let Some(entry) = entry else {
        return Ok(false);
    };
    *entry.kill_reason.lock().unwrap() = Some(reason);
    let mut child = entry.child.lock().await;
    child.start_kill().map_err(|e| AppError::ProcessError {
        message: format!("failed to kill child for item {item_id}: {e}"),
        stderr: None,
    })?;
    // Wait for the actual exit before returning: callers (cancel's partial-
    // file cleanup, resume's re-spawn) both need the guarantee that the old
    // process is truly gone and no longer writing to the output file —
    // `start_kill` alone only *requests* termination, it doesn't wait for it.
    let _ = child.wait().await;
    Ok(true)
}

/// Resolved spawn arguments for one item. Already defaulted/validated by the
/// ipc layer (CONVENTIONS: "validation at the trust boundary only" — this
/// module trusts its input).
pub struct SpawnParams {
    pub ytdlp_path: String,
    pub ffmpeg_path: String,
    pub url: String,
    pub format_expr: String,
    pub output_dir: String,
    pub output_template: String,
    pub proxy: Option<String>,
    pub extra_args: Option<String>,
}

// §8 durability: checkpoint on ticks that advance >=1% or >=2s since the
// last checkpoint, not on every parsed line.
const CHECKPOINT_MIN_PERCENT_DELTA: f64 = 1.0;
const CHECKPOINT_MIN_INTERVAL: Duration = Duration::from_secs(2);
// §7.3: progress events throttled to <=~10/sec/item. 150ms leaves headroom
// under that ceiling rather than emitting right up against it.
const EMIT_MIN_INTERVAL: Duration = Duration::from_millis(150);

/// Spawns yt-dlp for `item_id`, streams stdout/stderr, checkpoints progress
/// to DB, and emits progress/stage_changed via `emitter`. On exit: code 0 →
/// `completed` + resolved `output_path`; non-zero → `error` +
/// `error_message` (verbatim stderr, never paraphrased per CONVENTIONS).
/// Never panics on child-process/DB paths — failures become `error` stage,
/// not a returned `Err`, so callers (ipc.rs's fire-and-forget spawn) don't
/// need to handle a failure path themselves; the DB row + emitted event are
/// the only signal.
pub async fn run_download(
    db: Arc<Mutex<Connection>>,
    item_id: i64,
    params: SpawnParams,
    emitter: Arc<dyn Emitter>,
    registry: ActiveRegistry,
) {
    let result = run_download_inner(&db, item_id, &params, Arc::clone(&emitter), &registry).await;
    registry.lock().unwrap().remove(&item_id);
    if let Err(err) = result {
        let message = err.to_string();
        if let Ok(conn) = db.lock() {
            let _ = persistence::finish_item(&conn, item_id, "error", None, Some(&message));
        }
        emitter.emit_stage_changed(item_id, "error", Some(&message));
    }
}

async fn run_download_inner(
    db: &Arc<Mutex<Connection>>,
    item_id: i64,
    params: &SpawnParams,
    emitter: Arc<dyn Emitter>,
    registry: &ActiveRegistry,
) -> Result<(), AppError> {
    let output_template_path = format!("{}/{}", params.output_dir, params.output_template);

    let mut cmd = Command::new(&params.ytdlp_path);
    cmd.arg("--newline")
        // yt-dlp suppresses progress reporting entirely when stdout isn't a
        // TTY (confirmed in this sandbox: `--newline` alone produced zero
        // `[download]` lines over a pipe) — `--progress` forces it on
        // regardless. Without this flag the whole progress pipeline is silent.
        .arg("--progress")
        .arg("-c") // resume-capable (ARCHITECTURE §5.1)
        .arg("--ffmpeg-location")
        .arg(&params.ffmpeg_path)
        .arg("-f")
        .arg(&params.format_expr)
        .arg("-o")
        .arg(&output_template_path)
        // ponytail: `--print after_move:filepath` is yt-dlp's documented,
        // supported way to learn the final resolved path after
        // post-processing/rename — confirmed working in this sandbox
        // against a real download. Simpler than re-deriving the template
        // ourselves (which would require re-implementing yt-dlp's output
        // template engine).
        .arg("--print")
        .arg("after_move:filepath")
        // T6 cancel/remove need the partial file's path to delete it.
        // Confirmed in this sandbox: yt-dlp's usual "[download] Destination:
        // …" status line is suppressed entirely once any `--print` flag is
        // present, so a dedicated `before_dl` print (prefixed so it can't be
        // confused with the `after_move` line above, since both are bare
        // lines with no "[" prefix) is the reliable way to learn it instead.
        // `%(filepath)s` isn't resolved yet at `before_dl` (prints "NA");
        // `%(_filename)s` is (also confirmed against a real download).
        .arg("--print")
        .arg("before_dl:BEGIREX_PARTIAL_PATH:%(_filename)s");

    if let Some(proxy) = params.proxy.as_deref().filter(|p| !p.is_empty()) {
        cmd.arg("--proxy").arg(proxy);
    }
    if let Some(extra) = params.extra_args.as_deref() {
        for arg in extra.split_whitespace() {
            cmd.arg(arg);
        }
    }
    cmd.arg(&params.url);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| AppError::ProcessError {
        message: format!("failed to spawn yt-dlp: {e}"),
        stderr: None,
    })?;

    let stdout = child.stdout.take().expect("stdout piped at spawn");
    let stderr = child.stderr.take().expect("stderr piped at spawn");

    // Registered *after* taking the pipes (still needs the owned `child`
    // value below) so pause/cancel can find and kill this child (T6 —
    // ARCHITECTURE §2 "hold the child handle").
    let kill_reason: Arc<Mutex<Option<KillReason>>> = Arc::new(Mutex::new(None));
    let partial_paths: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let child_handle = Arc::new(AsyncMutex::new(child));
    registry.lock().unwrap().insert(
        item_id,
        ActiveEntry {
            child: Arc::clone(&child_handle),
            kill_reason: Arc::clone(&kill_reason),
            partial_paths: Arc::clone(&partial_paths),
        },
    );

    // Drain stderr concurrently (into item_logs, verbatim) while the main
    // task drains stdout progress — avoids the two pipes deadlocking each
    // other if one fills its OS buffer while we're blocked reading the other.
    let db_for_stderr = Arc::clone(db);
    let emitter_for_stderr = Arc::clone(&emitter);
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut collected = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(conn) = db_for_stderr.lock() {
                let _ = persistence::insert_log(&conn, item_id, "stderr", &line);
            }
            emitter_for_stderr.emit_log_line(item_id, "stderr", &line);
            collected.push(line);
        }
        collected
    });

    let mut current_stage = "downloading".to_string();
    let mut last_checkpoint_percent = -1.0_f64;
    let mut last_checkpoint_at = Instant::now() - CHECKPOINT_MIN_INTERVAL;
    let mut last_emit_at = Instant::now() - EMIT_MIN_INTERVAL;
    let mut resolved_output_path: Option<String> = None;

    let mut stdout_lines = BufReader::new(stdout).lines();
    while let Ok(Some(text)) = stdout_lines.next_line().await {
        if kill_reason.lock().unwrap().is_some() {
            // Pause/cancel already requested — stop acting on further output
            // (the process is dying); the pause/cancel command owns the DB
            // write and emitted event from here.
            break;
        }
        if let Some(dest) = text.trim().strip_prefix("BEGIREX_PARTIAL_PATH:") {
            partial_paths.lock().unwrap().push(dest.to_string());
            continue;
        }
        if let Some(tick) = progress_parser::parse_line(&text) {
            let stage_str = match tick.stage {
                progress_parser::Stage::Downloading => "downloading",
                progress_parser::Stage::Merging => "merging",
            };
            if stage_str != current_stage {
                current_stage = stage_str.to_string();
                emitter.emit_stage_changed(item_id, &current_stage, None);
            }

            let now = Instant::now();
            let percent_delta = (tick.percent - last_checkpoint_percent).abs();
            if percent_delta >= CHECKPOINT_MIN_PERCENT_DELTA
                || now.duration_since(last_checkpoint_at) >= CHECKPOINT_MIN_INTERVAL
            {
                if let Ok(conn) = db.lock() {
                    let _ = persistence::checkpoint_progress(
                        &conn,
                        item_id,
                        &current_stage,
                        tick.downloaded_bytes,
                        tick.total_bytes,
                        tick.percent,
                        tick.speed_bps,
                        tick.eta_seconds,
                    );
                }
                last_checkpoint_percent = tick.percent;
                last_checkpoint_at = now;
            }

            if now.duration_since(last_emit_at) >= EMIT_MIN_INTERVAL {
                emitter.emit_progress(item_id, &tick);
                last_emit_at = now;
            }
        } else {
            // Non-progress stdout line. `--print after_move:filepath` emits
            // the resolved path as a bare line (no "[" prefix) exactly once,
            // after the file lands in its final location.
            let trimmed = text.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('[') {
                resolved_output_path = Some(trimmed.to_string());
            }
        }
    }

    let stderr_lines = stderr_task.await.unwrap_or_default();
    let status = {
        let mut child = child_handle.lock().await;
        child.wait().await.map_err(|e| AppError::ProcessError {
            message: format!("failed waiting on yt-dlp: {e}"),
            stderr: None,
        })?
    };

    // An intentional pause/cancel already made its own DB write + emitted
    // event (queue_manager's pause_item/cancel_item) — this exit is expected,
    // not a completion or a failure, so don't overwrite that with
    // completed/error.
    if kill_reason.lock().unwrap().is_some() {
        return Ok(());
    }

    if status.success() {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::finish_item(&conn, item_id, "completed", resolved_output_path.as_deref(), None)?;
        drop(conn);
        emitter.emit_stage_changed(item_id, "completed", None);
    } else {
        // Verbatim stderr per CONVENTIONS — never paraphrase yt-dlp's own
        // error text. Use the full captured stderr tail; fall back to a
        // generic message only if yt-dlp produced no stderr at all.
        let error_message = if stderr_lines.is_empty() {
            format!("yt-dlp exited with status {status}")
        } else {
            stderr_lines.join("\n")
        };
        let conn = db.lock().expect("db mutex poisoned");
        persistence::finish_item(&conn, item_id, "error", None, Some(&error_message))?;
        drop(conn);
        emitter.emit_stage_changed(item_id, "error", Some(&error_message));
    }

    Ok(())
}

// --- T9: probe (S3/S4) ------------------------------------------------------

/// One entry of `probe`'s output (ARCHITECTURE §7.2 `Format` shape). `id` is
/// yt-dlp's `format_id`, the only field that round-trips into a format
/// expression (e.g. `137+140`).
#[derive(Debug, Clone, Serialize)]
pub struct ProbeFormat {
    pub id: String,
    pub resolution: Option<String>,
    pub ext: String,
    pub fps: Option<f64>,
    pub filesize: Option<i64>,
    pub codec: Option<String>,
    pub note: Option<String>,
    /// True when this format carries its own audio track (yt-dlp `acodec` !=
    /// `none`/absent) regardless of whether it's also video — i.e. a video
    /// row with `has_audio: true` is already muxed and needs no `+audio_id`
    /// merge (S4's "free-merge" filter, T10).
    pub has_audio: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeResult {
    pub title: String,
    pub formats: Vec<ProbeFormat>,
}

/// Subset of yt-dlp's `-J` JSON this module actually reads — the real
/// payload has dozens more fields we don't need (ponytail: no full schema).
#[derive(Debug, Deserialize)]
struct YtDlpFormatJson {
    format_id: String,
    ext: Option<String>,
    resolution: Option<String>,
    height: Option<i64>,
    width: Option<i64>,
    fps: Option<f64>,
    filesize: Option<i64>,
    filesize_approx: Option<i64>,
    vcodec: Option<String>,
    acodec: Option<String>,
    format_note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct YtDlpProbeJson {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    formats: Vec<YtDlpFormatJson>,
}

/// Runs `yt-dlp -J` (dump metadata as JSON, no download) for `url` and maps
/// its `formats` array into our `ProbeFormat` shape (ARCHITECTURE §5 "probe =
/// `-J` run", §7.2 `probe_formats`). A one-shot call, not a supervised
/// download, so unlike `run_download` it doesn't register in `ActiveRegistry`
/// — nothing here can be paused/cancelled by T6.
pub async fn probe(ytdlp_path: &str, url: &str, proxy: Option<&str>) -> Result<ProbeResult, AppError> {
    let mut cmd = Command::new(ytdlp_path);
    cmd.arg("-J").arg("--no-playlist");
    if let Some(proxy) = proxy.filter(|p| !p.is_empty()) {
        cmd.arg("--proxy").arg(proxy);
    }
    cmd.arg(url);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = cmd.output().await.map_err(|e| AppError::ProbeFailed {
        message: format!("failed to run yt-dlp: {e}"),
        stderr: None,
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AppError::ProbeFailed {
            message: "probe failed".into(),
            stderr: Some(stderr),
        });
    }

    let parsed: YtDlpProbeJson = serde_json::from_str(&String::from_utf8_lossy(&output.stdout))
        .map_err(|e| AppError::ProbeFailed {
            message: format!("could not parse yt-dlp output: {e}"),
            stderr: None,
        })?;

    let formats = parsed.formats.into_iter().map(map_format).collect();

    Ok(ProbeResult {
        title: parsed.title.unwrap_or_default(),
        formats,
    })
}

/// Pure mapping from one yt-dlp `-J` format entry to our `ProbeFormat`
/// (factored out of `probe` so it's unit-testable without spawning a real
/// process).
fn map_format(f: YtDlpFormatJson) -> ProbeFormat {
    // `vcodec`/`acodec` are absent entirely (not just "none") for some
    // generic-extractor direct-file formats (confirmed in this sandbox
    // against a plain .mp4 URL) — a present `height`/`width` is a more
    // reliable video signal than `vcodec` alone in that case.
    let is_video =
        f.height.is_some() || f.width.is_some() || f.vcodec.as_deref().is_some_and(|v| v != "none");
    let is_audio = f.acodec.as_deref().is_some_and(|a| a != "none");
    // yt-dlp almost always sets `resolution` itself (as `WIDTHxHEIGHT` or
    // `"audio only"`) — this only fills the gap on the rare format missing
    // it, matching yt-dlp's own `WIDTHxHEIGHT` convention rather than
    // inventing a new one the frontend would have to special-case.
    let resolution = f.resolution.or_else(|| {
        if is_video {
            match (f.width, f.height) {
                (Some(w), Some(h)) => Some(format!("{w}x{h}")),
                (None, Some(h)) => Some(format!("{h}p")),
                _ => None,
            }
        } else if is_audio {
            Some("audio only".to_string())
        } else {
            None
        }
    });
    let codec = if is_video {
        f.vcodec.filter(|c| c != "none")
    } else if is_audio {
        f.acodec.filter(|c| c != "none")
    } else {
        None
    };
    ProbeFormat {
        id: f.format_id,
        resolution,
        ext: f.ext.unwrap_or_default(),
        fps: f.fps,
        filesize: f.filesize.or(f.filesize_approx),
        codec,
        note: f.format_note,
        has_audio: is_audio,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// In-memory recorder — the "test harness that records what would be
    /// emitted" called for by T2 acceptance criterion 3, standing in for a
    /// real Tauri event bus.
    #[derive(Default)]
    struct RecordingEmitter {
        progress_events: StdMutex<Vec<(i64, f64)>>,
        stage_events: StdMutex<Vec<(i64, String, Option<String>)>>,
    }

    impl Emitter for RecordingEmitter {
        fn emit_progress(&self, item_id: i64, tick: &ProgressTick) {
            self.progress_events
                .lock()
                .unwrap()
                .push((item_id, tick.percent));
        }
        fn emit_stage_changed(&self, item_id: i64, stage: &str, error_message: Option<&str>) {
            self.stage_events.lock().unwrap().push((
                item_id,
                stage.to_string(),
                error_message.map(|s| s.to_string()),
            ));
        }
        fn emit_item_added(&self, _item: &persistence::Item) {}
        fn emit_item_removed(&self, _item_id: i64) {}
        fn emit_log_line(&self, _item_id: i64, _stream: &str, _line: &str) {}
    }

    #[test]
    fn recording_emitter_captures_progress_and_stage_events() {
        // Exercises the injectable-emitter seam itself (the structural
        // decision this module makes to stay testable without AppHandle) —
        // the full spawn path is covered for real by
        // tests/engine_integration.rs against a live yt-dlp process.
        let emitter = RecordingEmitter::default();
        let tick = progress_parser::parse_line(
            "[download]   0.5% of  218.53KiB at   43.99KiB/s ETA 00:04",
        )
        .unwrap();
        emitter.emit_progress(1, &tick);
        emitter.emit_stage_changed(1, "downloading", None);
        emitter.emit_stage_changed(1, "error", Some("boom"));

        assert_eq!(emitter.progress_events.lock().unwrap().len(), 1);
        assert_eq!(emitter.stage_events.lock().unwrap().len(), 2);
        assert_eq!(
            emitter.stage_events.lock().unwrap()[1].2.as_deref(),
            Some("boom")
        );
    }

    fn ytdlp_format(
        vcodec: Option<&str>,
        acodec: Option<&str>,
        resolution: Option<&str>,
        width: Option<i64>,
        height: Option<i64>,
    ) -> YtDlpFormatJson {
        YtDlpFormatJson {
            format_id: "137".into(),
            ext: Some("mp4".into()),
            resolution: resolution.map(String::from),
            height,
            width,
            fps: Some(30.0),
            filesize: Some(1_400_000_000),
            filesize_approx: None,
            vcodec: vcodec.map(String::from),
            acodec: acodec.map(String::from),
            format_note: None,
        }
    }

    #[test]
    fn map_format_prefers_vcodec_and_explicit_resolution_for_video() {
        let f = ytdlp_format(Some("avc1"), Some("none"), Some("1920x1080"), Some(1920), Some(1080));
        let mapped = map_format(f);
        assert_eq!(mapped.resolution.as_deref(), Some("1920x1080"));
        assert_eq!(mapped.codec.as_deref(), Some("avc1"));
    }

    #[test]
    fn map_format_derives_widthxheight_resolution_when_missing() {
        let f = ytdlp_format(Some("vp9"), Some("none"), None, Some(1280), Some(720));
        let mapped = map_format(f);
        assert_eq!(mapped.resolution.as_deref(), Some("1280x720"));
    }

    #[test]
    fn map_format_marks_audio_only_and_uses_acodec() {
        let f = ytdlp_format(Some("none"), Some("aac"), None, None, None);
        let mapped = map_format(f);
        assert_eq!(mapped.resolution.as_deref(), Some("audio only"));
        assert_eq!(mapped.codec.as_deref(), Some("aac"));
    }

    #[test]
    fn map_format_treats_missing_vcodec_with_height_as_video() {
        // Real generic-extractor formats (e.g. archive.org derivatives) omit
        // `vcodec` entirely rather than reporting "none" — confirmed in this
        // sandbox — so `height`/`width` alone must still mark it as video.
        let f = ytdlp_format(None, None, Some("427x240"), Some(427), Some(240));
        let mapped = map_format(f);
        assert_eq!(mapped.resolution.as_deref(), Some("427x240"));
        assert_eq!(mapped.codec, None);
    }

    #[test]
    fn map_format_marks_has_audio_false_for_video_only_and_true_for_muxed() {
        let video_only = ytdlp_format(Some("avc1"), Some("none"), Some("1920x1080"), Some(1920), Some(1080));
        assert!(!map_format(video_only).has_audio);

        let muxed = ytdlp_format(Some("avc1"), Some("mp4a"), Some("1920x1080"), Some(1920), Some(1080));
        assert!(map_format(muxed).has_audio);

        let audio_only = ytdlp_format(Some("none"), Some("aac"), None, None, None);
        assert!(map_format(audio_only).has_audio);
    }
}
