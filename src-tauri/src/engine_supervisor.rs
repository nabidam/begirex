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
    /// T16 mid-session binary health (ARCHITECTURE §7.3 `binary_health`) —
    /// `which` is the wire value ("ytdlp"/"ffmpeg"); `path` is the last known
    /// path when `found` is false, `None` otherwise.
    fn emit_binary_health(&self, which: &str, found: bool, path: Option<&str>);
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

/// T16 pre-spawn health check (ARCHITECTURE §8): a cheap filesystem
/// existence check, not the deeper `--version` probe `binary_manager` runs
/// for the user-triggered "Re-check" — that would mean spawning an extra
/// process before every single download start. All this needs to catch is
/// "the file that used to be here is gone" (K1-AC7), which an existence
/// check already does.
fn binary_present(path: &str) -> bool {
    std::path::Path::new(path).is_file()
}

/// Returns the wire name ("ytdlp"/"ffmpeg") of the first resolved binary
/// path that's gone missing, if any.
fn missing_binary(params: &SpawnParams) -> Option<&'static str> {
    if !binary_present(&params.ytdlp_path) {
        Some("ytdlp")
    } else if !binary_present(&params.ffmpeg_path) {
        Some("ffmpeg")
    } else {
        None
    }
}

/// A resolved binary vanished mid-session (K1-AC7): `item_id` never actually
/// starts — its `downloading` flip (already written by the caller before
/// spawning) reverts to `queued` — and every other item this module still
/// holds a running child for is paused too, since it's just as stranded.
/// Emits `binary_health{found:false}` last so the frontend's banner appears
/// after the queue already reflects the paused state.
async fn abort_for_missing_binary(
    db: &Arc<Mutex<Connection>>,
    item_id: i64,
    which: &str,
    emitter: &Arc<dyn Emitter>,
    registry: &ActiveRegistry,
) {
    if let Ok(conn) = db.lock() {
        let _ = persistence::set_stage(&conn, item_id, "queued");
    }
    emitter.emit_stage_changed(item_id, "queued", None);

    let active_ids: Vec<i64> = registry.lock().unwrap().keys().copied().collect();
    for active_id in active_ids {
        if let Ok(true) = pause(registry, active_id).await {
            if let Ok(conn) = db.lock() {
                let _ = persistence::set_stage(&conn, active_id, "paused");
            }
            emitter.emit_stage_changed(active_id, "paused", None);
        }
    }

    emitter.emit_binary_health(which, false, None);
}

/// Spawns yt-dlp for `item_id`, streams stdout/stderr, checkpoints progress
/// to DB, and emits progress/stage_changed via `emitter`. On exit: code 0 →
/// `completed` + resolved `output_path`; non-zero → `error` +
/// `error_message` (verbatim stderr, never paraphrased per CONVENTIONS).
/// Never panics on child-process/DB paths — failures become `error` stage,
/// not a returned `Err`, so callers (ipc.rs's fire-and-forget spawn) don't
/// need to handle a failure path themselves; the DB row + emitted event are
/// the only signal. Checks both resolved binary paths still exist before
/// doing anything else (ARCHITECTURE §8 pre-spawn health check, T16).
pub async fn run_download(
    db: Arc<Mutex<Connection>>,
    item_id: i64,
    params: SpawnParams,
    emitter: Arc<dyn Emitter>,
    registry: ActiveRegistry,
) {
    if let Some(which) = missing_binary(&params) {
        abort_for_missing_binary(&db, item_id, which, &emitter, &registry).await;
        return;
    }

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

#[derive(Debug, Deserialize)]
struct YtDlpPlaylistEntryJson {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    webpage_url: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct YtDlpPlaylistJson {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    webpage_url: Option<String>,
    #[serde(default)]
    entries: Option<Vec<YtDlpPlaylistEntryJson>>,
}

/// One resolved row a playlist expands to: a URL plus its title, if yt-dlp's
/// flat listing provided one (T19, ARCHITECTURE §2 "queue_manager owns
/// playlist expansion").
#[derive(Debug, Clone)]
pub struct PlaylistEntry {
    pub url: String,
    pub title: Option<String>,
}

/// Result of expanding a submitted URL. `playlist_id` is `Some` only when
/// yt-dlp reported more than one entry (a real playlist, K2-AC3/V2-AC2);
/// a lone video always comes back as exactly one `PlaylistEntry` with
/// `playlist_id: None` so `add_download`'s caller never has to branch on
/// entry count itself.
#[derive(Debug, Clone)]
pub struct PlaylistExpansion {
    pub playlist_id: Option<String>,
    pub entries: Vec<PlaylistEntry>,
}

/// Runs `yt-dlp -J --flat-playlist` for `url` (T19, ARCHITECTURE §2
/// "queue_manager owns playlist expansion"). Flat mode keeps this cheap even
/// for large playlists — no per-video format probe, just id/title/url. A
/// lone video still round-trips through here (yt-dlp returns it with no
/// `entries` key, or a single-element one) so the expand-then-add path in
/// `queue_manager` stays uniform for both shapes.
pub async fn expand_playlist(ytdlp_path: &str, url: &str, proxy: Option<&str>) -> Result<PlaylistExpansion, AppError> {
    let mut cmd = Command::new(ytdlp_path);
    cmd.arg("-J").arg("--flat-playlist");
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
            message: "playlist expansion failed".into(),
            stderr: Some(stderr),
        });
    }

    parse_playlist_json(&String::from_utf8_lossy(&output.stdout), url)
}

/// Pure JSON→`PlaylistExpansion` mapping (factored out of `expand_playlist`
/// so it's unit-testable without spawning a real process, same pattern as
/// `map_format`/`probe`).
fn parse_playlist_json(raw: &str, submitted_url: &str) -> Result<PlaylistExpansion, AppError> {
    let parsed: YtDlpPlaylistJson = serde_json::from_str(raw).map_err(|e| AppError::ProbeFailed {
        message: format!("could not parse yt-dlp output: {e}"),
        stderr: None,
    })?;

    match parsed.entries {
        Some(raw_entries) if raw_entries.len() > 1 => {
            // ponytail: some extractors' flat entries carry a bare video id in
            // `url` rather than a directly fetchable link — `webpage_url` (when
            // present) is preferred for exactly that reason. Upgrade path: a
            // per-extractor URL builder, if a site without `webpage_url` bites.
            let entries: Vec<PlaylistEntry> = raw_entries
                .into_iter()
                .filter_map(|e| {
                    let url = e.webpage_url.or(e.url)?;
                    Some(PlaylistEntry { url, title: e.title })
                })
                .collect();
            let playlist_id = parsed
                .id
                .or(parsed.webpage_url)
                .or_else(|| Some(submitted_url.to_string()));
            Ok(PlaylistExpansion { playlist_id, entries })
        }
        _ => Ok(PlaylistExpansion {
            playlist_id: None,
            entries: vec![PlaylistEntry {
                url: submitted_url.to_string(),
                title: parsed.title,
            }],
        }),
    }
}

/// A tiny fake yt-dlp `-J` payload covering the full height range (audio,
/// 480p..4320p) so any reasonable height/resolution filter still has
/// candidates to select from — `--load-info-json` runs `format_expr`
/// through yt-dlp's real selector parser against this local, offline stand-in
/// instead of a network probe (T11 "dry-parse via engine").
///
/// ponytail: an expression that's syntactically valid but matches none of
/// these stand-in formats (e.g. an exotic codec filter) still reports
/// `INVALID_FORMAT_EXPR` with yt-dlp's "Requested format is not available"
/// stderr — same failure shape a real download against an unmatching video
/// would hit, so it's an acceptable false-positive surface for a save-time
/// check; a wider stand-in format list is the upgrade path if this bites.
const DRY_PARSE_INFO_JSON: &str = r#"{
  "id": "dry-parse", "title": "dry-parse", "extractor": "generic",
  "extractor_key": "Generic", "webpage_url": "https://example.invalid/",
  "original_url": "https://example.invalid/",
  "formats": [
    {"format_id": "audio", "ext": "m4a", "vcodec": "none", "acodec": "mp4a.40.2", "url": "https://example.invalid/a"},
    {"format_id": "v480", "ext": "mp4", "vcodec": "avc1", "acodec": "none", "width": 854, "height": 480, "url": "https://example.invalid/v480"},
    {"format_id": "v1080", "ext": "webm", "vcodec": "vp9", "acodec": "none", "width": 1920, "height": 1080, "url": "https://example.invalid/v1080"},
    {"format_id": "v2160", "ext": "webm", "vcodec": "vp9", "acodec": "none", "width": 3840, "height": 2160, "url": "https://example.invalid/v2160"},
    {"format_id": "v4320", "ext": "webm", "vcodec": "vp9", "acodec": "none", "width": 7680, "height": 4320, "url": "https://example.invalid/v4320"},
    {"format_id": "best", "ext": "mp4", "vcodec": "avc1", "acodec": "mp4a.40.2", "width": 1920, "height": 1080, "url": "https://example.invalid/best"}
  ]
}"#;

/// Validates a format expression by running it through yt-dlp's real
/// selector parser via `--load-info-json` (no network call) — ARCHITECTURE
/// §2 "preset_service: dry-parse format expression on save",
/// engine_supervisor is the shared dependency because it owns all yt-dlp
/// invocation. A non-zero exit (bad syntax or no stand-in format matches)
/// becomes `INVALID_FORMAT_EXPR` carrying yt-dlp's stderr verbatim (§7.1).
pub async fn dry_parse_format(ytdlp_path: &str, format_expr: &str) -> Result<(), AppError> {
    // Unique scratch path per invocation: a process-id-only name was shared by
    // every concurrent validation, so one call could overwrite or delete the
    // JSON another's yt-dlp was still reading — yielding spurious
    // INVALID_FORMAT_EXPR. The monotonic counter disambiguates within the
    // process; we remove only this call's own file below.
    static DRY_PARSE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = DRY_PARSE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let info_json_path = std::env::temp_dir().join(format!(
        "begirex-dry-parse-{}-{}.json",
        std::process::id(),
        seq
    ));
    tokio::fs::write(&info_json_path, DRY_PARSE_INFO_JSON)
        .await
        .map_err(|e| AppError::InvalidFormatExpr {
            message: format!("failed to write dry-parse scratch file: {e}"),
            stderr: None,
        })?;

    let mut cmd = Command::new(ytdlp_path);
    cmd.arg("--load-info-json")
        .arg(&info_json_path)
        .arg("-f")
        .arg(format_expr)
        .arg("--simulate")
        .arg("--no-warnings");
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = cmd.output().await;
    let _ = tokio::fs::remove_file(&info_json_path).await;
    let output = output.map_err(|e| AppError::InvalidFormatExpr {
        message: format!("failed to run yt-dlp: {e}"),
        stderr: None,
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AppError::InvalidFormatExpr {
            message: "invalid format expression".into(),
            stderr: Some(stderr),
        });
    }
    Ok(())
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
        binary_health_events: StdMutex<Vec<(String, bool, Option<String>)>>,
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
        fn emit_binary_health(&self, which: &str, found: bool, path: Option<&str>) {
            self.binary_health_events
                .lock()
                .unwrap()
                .push((which.to_string(), found, path.map(|s| s.to_string())));
        }
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

    fn ytdlp_path() -> String {
        std::env::var("YTDLP_TEST_PATH").unwrap_or_else(|_| "yt-dlp".into())
    }

    #[tokio::test]
    async fn dry_parse_format_accepts_valid_expression() {
        dry_parse_format(&ytdlp_path(), "bv*[height<=2160]+ba/b")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn dry_parse_format_rejects_malformed_expression() {
        let err = dry_parse_format(&ytdlp_path(), "bv*[[[garbage")
            .await
            .unwrap_err();
        match err {
            AppError::InvalidFormatExpr { stderr, .. } => {
                assert!(stderr.unwrap_or_default().contains("Invalid format specification"));
            }
            other => panic!("expected InvalidFormatExpr, got {other:?}"),
        }
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

    // --- T19: playlist expansion ---------------------------------------------

    #[test]
    fn parse_playlist_json_expands_multi_entry_playlist_sharing_a_playlist_id() {
        let raw = r#"{
            "id": "PL123",
            "entries": [
                {"webpage_url": "https://example.invalid/watch?v=a", "title": "A"},
                {"url": "https://example.invalid/watch?v=b", "title": "B"}
            ]
        }"#;
        let expansion = parse_playlist_json(raw, "https://example.invalid/playlist?list=PL123").unwrap();
        assert_eq!(expansion.playlist_id, Some("PL123".to_string()));
        assert_eq!(expansion.entries.len(), 2);
        assert_eq!(expansion.entries[0].url, "https://example.invalid/watch?v=a");
        assert_eq!(expansion.entries[0].title, Some("A".to_string()));
        assert_eq!(expansion.entries[1].url, "https://example.invalid/watch?v=b");
    }

    #[test]
    fn parse_playlist_json_treats_lone_video_as_a_single_non_playlist_entry() {
        let raw = r#"{"title": "Solo video"}"#;
        let expansion = parse_playlist_json(raw, "https://example.invalid/watch?v=x").unwrap();
        assert_eq!(expansion.playlist_id, None);
        assert_eq!(expansion.entries.len(), 1);
        assert_eq!(expansion.entries[0].url, "https://example.invalid/watch?v=x");
        assert_eq!(expansion.entries[0].title, Some("Solo video".to_string()));
    }

    #[test]
    fn parse_playlist_json_treats_single_element_entries_as_non_playlist() {
        // A playlist URL that resolves to exactly one entry (e.g. a playlist of
        // one) should not get a `playlist_id` — matches "expansion only fires
        // for >1 entries" (T19-AC1).
        let raw = r#"{"id": "PL1", "entries": [{"url": "https://example.invalid/watch?v=a", "title": "A"}]}"#;
        let expansion = parse_playlist_json(raw, "https://example.invalid/playlist?list=PL1").unwrap();
        assert_eq!(expansion.playlist_id, None);
        assert_eq!(expansion.entries.len(), 1);
    }

    #[test]
    fn parse_playlist_json_skips_entries_with_no_resolvable_url() {
        let raw = r#"{
            "id": "PL9",
            "entries": [
                {"webpage_url": "https://example.invalid/watch?v=a", "title": "A"},
                {"title": "dead, no url or webpage_url"},
                {"url": "https://example.invalid/watch?v=c", "title": "C"}
            ]
        }"#;
        let expansion = parse_playlist_json(raw, "https://example.invalid/playlist?list=PL9").unwrap();
        assert_eq!(expansion.entries.len(), 2);
        assert_eq!(expansion.entries[0].url, "https://example.invalid/watch?v=a");
        assert_eq!(expansion.entries[1].url, "https://example.invalid/watch?v=c");
    }

    // --- T16: pre-spawn health check -----------------------------------------

    fn new_test_item(conn: &Connection, url: &str, stage: &str) -> persistence::Item {
        persistence::insert_item(
            conn,
            persistence::NewItem {
                url: url.into(),
                format_expr: "bv*+ba/b".into(),
                output_dir: "/tmp/out".into(),
                output_template: "%(title)s.%(ext)s".into(),
                proxy: None,
                extra_args: None,
                preset_id: None,
                stage: stage.into(),
                playlist_id: None,
                title: None,
            },
        )
        .unwrap()
    }

    fn spawn_params(ytdlp_path: &str) -> SpawnParams {
        SpawnParams {
            ytdlp_path: ytdlp_path.to_string(),
            ffmpeg_path: "/usr/bin/env".to_string(),
            url: "https://example.invalid/".to_string(),
            format_expr: "bv*+ba/b".to_string(),
            output_dir: "/tmp".to_string(),
            output_template: "%(title)s.%(ext)s".to_string(),
            proxy: None,
            extra_args: None,
        }
    }

    #[test]
    fn missing_binary_detects_gone_ytdlp_before_ffmpeg() {
        let params = spawn_params("/definitely/not/a/real/binary");
        assert_eq!(missing_binary(&params), Some("ytdlp"));
    }

    #[test]
    fn missing_binary_returns_none_when_both_paths_exist() {
        let params = spawn_params("/usr/bin/env");
        assert_eq!(missing_binary(&params), None);
    }

    #[tokio::test]
    async fn run_download_aborts_and_reverts_to_queued_when_ytdlp_missing() {
        // K1-AC7: a resolved binary vanishing mid-session must not spawn into
        // failure — the item goes back to `queued`, not `error`.
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        let item = new_test_item(&conn, "https://a", "downloading");
        let db = Arc::new(Mutex::new(conn));

        let emitter = Arc::new(RecordingEmitter::default());
        let registry: ActiveRegistry = Arc::new(Mutex::new(HashMap::new()));
        let params = spawn_params("/definitely/not/a/real/binary");

        run_download(
            Arc::clone(&db),
            item.id,
            params,
            emitter.clone(),
            Arc::clone(&registry),
        )
        .await;

        let reverted = persistence::get_item(&db.lock().unwrap(), item.id).unwrap();
        assert_eq!(reverted.stage, "queued");

        let stage_events = emitter.stage_events.lock().unwrap();
        assert!(stage_events.contains(&(item.id, "queued".to_string(), None)));

        let health_events = emitter.binary_health_events.lock().unwrap();
        assert_eq!(health_events.as_slice(), &[("ytdlp".to_string(), false, None)]);

        assert!(registry.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn run_download_pauses_other_active_children_when_binary_missing() {
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        let stalled = new_test_item(&conn, "https://a", "downloading");
        let already_running = new_test_item(&conn, "https://b", "downloading");
        let db = Arc::new(Mutex::new(conn));

        // Stand in for an already-spawned child (T6's `ActiveEntry`) with a
        // real, long-lived process so `pause` has something to actually kill.
        let child = tokio::process::Command::new("sleep")
            .arg("5")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let registry: ActiveRegistry = Arc::new(Mutex::new(HashMap::new()));
        registry.lock().unwrap().insert(
            already_running.id,
            ActiveEntry {
                child: Arc::new(AsyncMutex::new(child)),
                kill_reason: Arc::new(Mutex::new(None)),
                partial_paths: Arc::new(Mutex::new(Vec::new())),
            },
        );

        let emitter = Arc::new(RecordingEmitter::default());
        let params = spawn_params("/definitely/not/a/real/binary");

        run_download(
            Arc::clone(&db),
            stalled.id,
            params,
            emitter.clone(),
            Arc::clone(&registry),
        )
        .await;

        let paused = persistence::get_item(&db.lock().unwrap(), already_running.id).unwrap();
        assert_eq!(paused.stage, "paused");
        let stage_events = emitter.stage_events.lock().unwrap();
        assert!(stage_events.contains(&(already_running.id, "paused".to_string(), None)));
    }
}
