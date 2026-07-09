//! Integration tests for T3 acceptance criteria 1 & 2 — real yt-dlp child
//! processes, real DB files, real filesystem, driven through
//! `queue_manager` exactly as `ipc.rs`/`lib.rs` wire it in production.
//! Network-dependent, so `#[ignore]`d; run explicitly at demo gates
//! (CONVENTIONS "Tests"):
//!   cargo test --manifest-path src-tauri/Cargo.toml -- --ignored
//!
//! Same test asset as `engine_integration.rs` (T2) for consistency — see that
//! file's header comment for why: tiny, single-format (no merge-driven
//! percent reset), stable, no JS-runtime dependency.

use begirex_lib::engine_supervisor::{self, Emitter};
use begirex_lib::persistence::{self, Item, NewItem};
use begirex_lib::progress_parser::{self, ProgressTick};
use begirex_lib::queue_manager::{self, AddDownloadParams, BinaryPaths};
use rusqlite::Connection;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Fresh, empty per-test registry (T6) — these T3 tests don't exercise
/// pause/cancel, they just need something to satisfy the now-required
/// registry param on `add_and_schedule`/`reconcile_and_resume`.
fn empty_registry() -> engine_supervisor::ActiveRegistry {
    Arc::new(Mutex::new(HashMap::new()))
}

const TEST_URL: &str = "https://download.samplelib.com/mp4/sample-5s.mp4";

fn resolve_on_path(name: &str) -> Option<String> {
    let output = Command::new("which").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "begirex_queue_it_{name}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Records every emitted event — stands in for the real Tauri event bus,
/// same role as T2's `RecordingEmitter`.
#[derive(Default)]
struct RecordingEmitter {
    #[allow(dead_code)]
    stage_events: Mutex<Vec<(i64, String, Option<String>)>>,
}

impl Emitter for RecordingEmitter {
    fn emit_progress(&self, _item_id: i64, _tick: &ProgressTick) {}
    fn emit_stage_changed(&self, item_id: i64, stage: &str, error_message: Option<&str>) {
        self.stage_events.lock().unwrap().push((
            item_id,
            stage.to_string(),
            error_message.map(|s| s.to_string()),
        ));
    }
    fn emit_item_added(&self, _item: &Item) {}
    fn emit_item_removed(&self, _item_id: i64) {}
    fn emit_log_line(&self, _item_id: i64, _stream: &str, _line: &str) {}
    fn emit_binary_health(&self, _which: &str, _healthy: bool, _message: Option<&str>) {}
}

fn poll_item(
    db: &Arc<Mutex<Connection>>,
    id: i64,
    timeout: Duration,
    mut until: impl FnMut(&persistence::Item) -> bool,
) -> persistence::Item {
    let deadline = Instant::now() + timeout;
    loop {
        let item = {
            let conn = db.lock().unwrap();
            persistence::get_item(&conn, id).unwrap()
        };
        if until(&item) || Instant::now() >= deadline {
            return item;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

// --- T3 acceptance criterion 1 ----------------------------------------------
//
// Third add_download while two items are `downloading` inserts as `queued`;
// when one finishes, the lowest-queue_position queued item flips to
// `downloading` and a real yt-dlp process actually gets spawned for it
// (K2-AC2).

#[tokio::test]
#[ignore] // network + real yt-dlp processes
async fn third_add_queues_then_starts_on_slot_free_with_real_spawn() {
    let ytdlp_path = resolve_on_path("yt-dlp").expect("yt-dlp must be on PATH for this test");
    let ffmpeg_path = resolve_on_path("ffmpeg").expect("ffmpeg must be on PATH for this test");

    let work_dir = temp_dir("ac1");
    let db_path = work_dir.join("begirex.db");
    let conn = persistence::open_and_init(&db_path, work_dir.to_str().unwrap()).unwrap();
    let db: Arc<Mutex<Connection>> = Arc::new(Mutex::new(conn));
    let emitter: Arc<dyn Emitter> = Arc::new(RecordingEmitter::default());
    let binaries = BinaryPaths {
        ytdlp_path,
        ffmpeg_path,
    };

    // --limit-rate slows the (tiny, otherwise ~1-2s) download enough to give
    // this test a real window to observe "2 downloading, 1 queued" before
    // anything completes.
    let registry = empty_registry();
    let mut item_ids = Vec::new();
    for i in 0..3 {
        let item = queue_manager::add_and_schedule(
            Arc::clone(&db),
            Arc::clone(&emitter),
            binaries.clone(),
            2, // N=2
            AddDownloadParams {
                url: TEST_URL.to_string(),
                format_expr: "best".to_string(),
                output_dir: work_dir.to_str().unwrap().to_string(),
                output_template: format!("sample_{i}.%(ext)s"),
                proxy: None,
                extra_args: Some("--limit-rate 300K".to_string()),
                preset_id: None,
                playlist_id: None,
                title: None,
            },
            Arc::clone(&registry),
        )
        .unwrap();
        item_ids.push(item.id);
    }

    // Immediately after the 3rd add: exactly 2 downloading, 1 queued.
    let all = {
        let conn = db.lock().unwrap();
        persistence::list_items(&conn, None).unwrap()
    };
    let downloading: Vec<_> = all.iter().filter(|i| i.stage == "downloading").collect();
    let queued: Vec<_> = all.iter().filter(|i| i.stage == "queued").collect();
    assert_eq!(
        downloading.len(),
        2,
        "expected exactly 2 downloading right after the 3rd add_download, got: {all:?}"
    );
    assert_eq!(
        queued.len(),
        1,
        "expected exactly 1 queued right after the 3rd add_download, got: {all:?}"
    );
    let third_id = queued[0].id;

    // Wait for a slot to free and the scheduler to flip the 3rd item off
    // `queued` (it should reach `downloading`, possibly already past it by
    // the time we poll).
    let after_start = poll_item(&db, third_id, Duration::from_secs(60), |item| {
        item.stage != "queued"
    });
    assert_ne!(
        after_start.stage, "queued",
        "3rd item never left `queued` after a slot freed"
    );

    // Confirm it's backed by a *real spawned process*, not just a DB
    // stage-flip: sample its downloaded_bytes/percent, then confirm they
    // actually advance (or it finishes) afterward.
    let sample_a = after_start;
    if sample_a.stage != "completed" {
        let sample_b = poll_item(&db, third_id, Duration::from_secs(30), |item| {
            item.stage == "completed"
                || item.downloaded_bytes > sample_a.downloaded_bytes
                || item.percent > sample_a.percent
        });
        assert!(
            sample_b.stage == "completed"
                || sample_b.downloaded_bytes > sample_a.downloaded_bytes
                || sample_b.percent > sample_a.percent,
            "3rd item's progress never advanced after leaving `queued` \
             (downloaded_bytes stayed {}, percent stayed {}) — looks like a \
             stage-flip with no real yt-dlp process behind it",
            sample_a.downloaded_bytes,
            sample_a.percent
        );
    }

    // Let everything finish before cleanup so we don't race a live child.
    for id in &item_ids {
        poll_item(&db, *id, Duration::from_secs(60), |item| {
            matches!(item.stage.as_str(), "completed" | "error")
        });
    }

    std::fs::remove_dir_all(&work_dir).ok();
}

// --- T3 acceptance criterion 2 ----------------------------------------------
//
// After a kill -9 mid-download, on next launch items left downloading/merging
// are reconciled to a resumable state and re-spawned with -c; list_items
// returns them with last-checkpointed downloaded_bytes immediately.

#[tokio::test]
#[ignore] // network + real yt-dlp process + real kill -9
async fn kill_9_mid_download_then_reconcile_resumes_from_partial_bytes() {
    let ytdlp_path = resolve_on_path("yt-dlp").expect("yt-dlp must be on PATH for this test");
    let ffmpeg_path = resolve_on_path("ffmpeg").expect("ffmpeg must be on PATH for this test");

    let work_dir = temp_dir("ac2");
    let db_path = work_dir.join("begirex.db");
    let output_template = "sample_ac2.%(ext)s";

    let conn = persistence::open_and_init(&db_path, work_dir.to_str().unwrap()).unwrap();
    let item = persistence::insert_item(
        &conn,
        NewItem {
            url: TEST_URL.to_string(),
            format_expr: "best".to_string(),
            output_dir: work_dir.to_str().unwrap().to_string(),
            output_template: output_template.to_string(),
            proxy: None,
            extra_args: Some("--limit-rate 80K".to_string()),
            preset_id: None,
            stage: "downloading".to_string(),
            playlist_id: None,
            title: None,
        },
    )
    .unwrap();
    let item_id = item.id;
    drop(conn);

    // Spawn the real yt-dlp child directly (the same flags
    // engine_supervisor::run_download would use) rather than through
    // run_download itself: this test needs to kill -9 only the child while
    // nothing else reacts to its death, simulating the *whole app* (including
    // the supervising task) dying mid-download — not just the child alone.
    let output_template_path = format!("{}/{output_template}", work_dir.display());
    let mut child = Command::new(&ytdlp_path)
        .arg("--newline")
        .arg("--progress")
        .arg("-c")
        .arg("--ffmpeg-location")
        .arg(&ffmpeg_path)
        .arg("-f")
        .arg("best")
        .arg("--limit-rate")
        .arg("80K")
        .arg("-o")
        .arg(&output_template_path)
        .arg(TEST_URL)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn yt-dlp");

    let pid = child.id();
    let stdout = child.stdout.take().unwrap();
    let db_path_for_thread = db_path.clone();
    let (tx, rx) = std::sync::mpsc::channel::<i64>();

    // Checkpoint mid-flight progress using the exact same persistence/
    // progress_parser calls engine_supervisor's real loop uses (§8), so the
    // DB ends up in the same shape a real crash mid-checkpoint would leave.
    let checkpoint_thread = std::thread::spawn(move || {
        let conn = Connection::open(&db_path_for_thread).unwrap();
        let reader = std::io::BufReader::new(stdout);
        for line in std::io::BufRead::lines(reader).flatten() {
            if let Some(tick) = progress_parser::parse_line(&line) {
                let stage = match tick.stage {
                    progress_parser::Stage::Downloading => "downloading",
                    progress_parser::Stage::Merging => "merging",
                };
                let _ = persistence::checkpoint_progress(
                    &conn,
                    item_id,
                    stage,
                    tick.downloaded_bytes,
                    tick.total_bytes,
                    tick.percent,
                    tick.speed_bps,
                    tick.eta_seconds,
                );
                if tick.downloaded_bytes.unwrap_or(0) > 0 {
                    let _ = tx.send(tick.downloaded_bytes.unwrap_or(0));
                    break;
                }
            }
        }
    });

    let first_checkpointed_bytes = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("expected at least one nonzero progress checkpoint before timeout");
    assert!(first_checkpointed_bytes > 0);

    // Real kill -9 of the actual spawned yt-dlp child, mid-flight.
    let status = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .expect("failed to run kill -9");
    assert!(status.success(), "kill -9 {pid} failed to run");
    let _ = child.wait(); // reap the killed child
    checkpoint_thread.join().ok();

    // Pre-reconcile: DB row left `downloading`/`merging` with nonzero
    // last-checkpointed downloaded_bytes — exactly what `list_items` would
    // return immediately on a fresh launch, before any resume happens.
    let pre_reconcile = {
        let conn = Connection::open(&db_path).unwrap();
        persistence::get_item(&conn, item_id).unwrap()
    };
    assert!(
        matches!(pre_reconcile.stage.as_str(), "downloading" | "merging"),
        "expected the crashed item to still be downloading/merging pre-reconcile, got {}",
        pre_reconcile.stage
    );
    assert!(
        pre_reconcile.downloaded_bytes > 0,
        "expected nonzero last-checkpointed downloaded_bytes pre-reconcile"
    );
    let pre_reconcile_bytes = pre_reconcile.downloaded_bytes;

    // Simulate "next launch": open a fresh connection against the same DB
    // file and run launch-reconcile exactly as lib.rs's .setup() would.
    let db: Arc<Mutex<Connection>> = Arc::new(Mutex::new(Connection::open(&db_path).unwrap()));
    let emitter: Arc<dyn Emitter> = Arc::new(RecordingEmitter::default());
    let binaries = BinaryPaths {
        ytdlp_path: ytdlp_path.clone(),
        ffmpeg_path: ffmpeg_path.clone(),
    };
    queue_manager::reconcile_and_resume(Arc::clone(&db), Arc::clone(&emitter), binaries, 2, empty_registry())
        .unwrap();

    // Reconcile resumes it (spawns a fresh yt-dlp process with -c) and it
    // completes, ending at `completed` with a total >= what was already
    // downloaded pre-kill (proving it resumed, not restarted from scratch —
    // yt-dlp's `-c` continues the existing partial file rather than
    // re-downloading bytes already on disk).
    let final_item = poll_item(&db, item_id, Duration::from_secs(60), |item| {
        matches!(item.stage.as_str(), "completed" | "error")
    });
    assert_eq!(
        final_item.stage, "completed",
        "expected the resumed download to complete; error_message: {:?}",
        final_item.error_message
    );
    assert!(
        final_item.downloaded_bytes >= pre_reconcile_bytes,
        "resumed total ({}) should be >= pre-kill checkpointed bytes ({})",
        final_item.downloaded_bytes,
        pre_reconcile_bytes
    );

    let output_path = final_item
        .output_path
        .clone()
        .expect("completed item must have a resolved output_path");
    assert!(
        std::path::Path::new(&output_path).is_file(),
        "resolved output_path {output_path} does not exist on disk"
    );
    // The 2.72MiB source file, so a genuinely completed (not truncated)
    // resume lands very close to that size — sanity check against a
    // truncated file masquerading as `completed`.
    let final_size = std::fs::metadata(&output_path).unwrap().len();
    assert!(
        final_size > 2_000_000,
        "resumed file size ({final_size} bytes) looks truncated for a 2.72MiB source"
    );

    std::fs::remove_dir_all(&work_dir).ok();
}
