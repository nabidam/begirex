//! Integration tests for T6 acceptance criteria — real yt-dlp child
//! processes, real DB files, real filesystem, driven through
//! `queue_manager` exactly as `ipc.rs` wires it in production.
//! Network-dependent, so `#[ignore]`d; run explicitly at demo gates
//! (CONVENTIONS "Tests"):
//!   cargo test --manifest-path src-tauri/Cargo.toml -- --ignored
//!
//! Same test asset as `engine_integration.rs`/`queue_scheduling.rs` for
//! consistency — see those files' header comments for why (tiny,
//! single-format, stable, no JS-runtime dependency).

use begirex_lib::engine_supervisor::{self, Emitter};
use begirex_lib::persistence::{self, Item};
use begirex_lib::progress_parser::ProgressTick;
use begirex_lib::queue_manager::{self, AddDownloadParams, BinaryPaths, BulkVerb};
use rusqlite::Connection;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TEST_URL: &str = "https://download.samplelib.com/mp4/sample-5s.mp4";
// Slows the ~1-2s download enough to give tests a real window to pause/
// cancel/reorder mid-flight before it finishes.
const THROTTLE: &str = "--limit-rate 200K";

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
        "begirex_lifecycle_it_{name}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[derive(Default)]
struct RecordingEmitter {
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
        std::thread::sleep(Duration::from_millis(150));
    }
}

struct Harness {
    db: Arc<Mutex<Connection>>,
    emitter: Arc<dyn Emitter>,
    binaries: BinaryPaths,
    registry: engine_supervisor::ActiveRegistry,
    work_dir: std::path::PathBuf,
}

impl Harness {
    fn new(name: &str) -> Self {
        let ytdlp_path = resolve_on_path("yt-dlp").expect("yt-dlp must be on PATH for this test");
        let ffmpeg_path = resolve_on_path("ffmpeg").expect("ffmpeg must be on PATH for this test");
        let work_dir = temp_dir(name);
        let db_path = work_dir.join("begirex.db");
        let conn = persistence::open_and_init(&db_path, work_dir.to_str().unwrap()).unwrap();
        Harness {
            db: Arc::new(Mutex::new(conn)),
            emitter: Arc::new(RecordingEmitter::default()),
            binaries: BinaryPaths {
                ytdlp_path,
                ffmpeg_path,
            },
            registry: Arc::new(Mutex::new(HashMap::new())),
            work_dir,
        }
    }

    fn add(&self, n_slots: i64, output_template: &str) -> Item {
        queue_manager::add_and_schedule(
            Arc::clone(&self.db),
            Arc::clone(&self.emitter),
            self.binaries.clone(),
            n_slots,
            AddDownloadParams {
                url: TEST_URL.to_string(),
                format_expr: "best".to_string(),
                output_dir: self.work_dir.to_str().unwrap().to_string(),
                output_template: output_template.to_string(),
                proxy: None,
                extra_args: Some(THROTTLE.to_string()),
                preset_id: None,
                playlist_id: None,
                title: None,
            },
            Arc::clone(&self.registry),
        )
        .unwrap()
    }

    fn cleanup(&self) {
        std::fs::remove_dir_all(&self.work_dir).ok();
    }
}

// --- AC1: pause freezes percent; resume continues from the paused offset ---

#[tokio::test]
#[ignore] // network + real yt-dlp process
async fn pause_freezes_progress_then_resume_continues_from_offset() {
    let h = Harness::new("ac1_pause_resume");
    let item = h.add(2, "sample_ac1.%(ext)s");

    // Let real progress accumulate before pausing.
    let mid = poll_item(&h.db, item.id, Duration::from_secs(30), |i| i.downloaded_bytes > 0);
    assert!(mid.downloaded_bytes > 0, "expected some progress before pausing");

    queue_manager::pause_item(Arc::clone(&h.db), Arc::clone(&h.emitter), Arc::clone(&h.registry), item.id)
        .await
        .unwrap();

    let paused = {
        let conn = h.db.lock().unwrap();
        persistence::get_item(&conn, item.id).unwrap()
    };
    assert_eq!(paused.stage, "paused");
    let paused_bytes = paused.downloaded_bytes;
    assert!(paused_bytes > 0);

    // Percent must stay frozen for a bit — no live process running.
    std::thread::sleep(Duration::from_millis(800));
    let still_paused = {
        let conn = h.db.lock().unwrap();
        persistence::get_item(&conn, item.id).unwrap()
    };
    assert_eq!(still_paused.stage, "paused");
    assert_eq!(still_paused.downloaded_bytes, paused_bytes);

    queue_manager::resume_item(
        Arc::clone(&h.db),
        Arc::clone(&h.emitter),
        h.binaries.clone(),
        2,
        Arc::clone(&h.registry),
        item.id,
    )
    .unwrap();

    let final_item = poll_item(&h.db, item.id, Duration::from_secs(60), |i| {
        matches!(i.stage.as_str(), "completed" | "error")
    });
    assert_eq!(
        final_item.stage, "completed",
        "resumed download should complete; error_message: {:?}",
        final_item.error_message
    );
    assert!(
        final_item.downloaded_bytes >= paused_bytes,
        "resumed bytes ({}) should be >= pre-pause bytes ({})",
        final_item.downloaded_bytes,
        paused_bytes
    );

    h.cleanup();
}

// --- AC2: cancel frees a slot, a queued item starts, partial file gone -----

#[tokio::test]
#[ignore] // network + real yt-dlp processes
async fn cancel_frees_slot_starts_queued_item_and_deletes_partial_file() {
    let h = Harness::new("ac2_cancel");
    let first = h.add(1, "sample_ac2_first.%(ext)s"); // N=1: this one starts downloading
    let second = h.add(1, "sample_ac2_second.%(ext)s"); // queued behind it

    let second_before = {
        let conn = h.db.lock().unwrap();
        persistence::get_item(&conn, second.id).unwrap()
    };
    assert_eq!(second_before.stage, "queued");

    // Wait until the first item has actually started writing a partial file.
    poll_item(&h.db, first.id, Duration::from_secs(30), |i| i.downloaded_bytes > 0);
    let expected_partial = h.work_dir.join("sample_ac2_first.mp4");

    queue_manager::cancel_item(
        Arc::clone(&h.db),
        Arc::clone(&h.emitter),
        h.binaries.clone(),
        1,
        Arc::clone(&h.registry),
        first.id,
    )
    .await
    .unwrap();

    let cancelled = {
        let conn = h.db.lock().unwrap();
        persistence::get_item(&conn, first.id).unwrap()
    };
    assert_eq!(cancelled.stage, "cancelled");

    // The freed slot lets the queued item start.
    let second_after = poll_item(&h.db, second.id, Duration::from_secs(10), |i| i.stage != "queued");
    assert_eq!(second_after.stage, "downloading");

    // Give the filesystem a beat to reflect the deletion, then confirm gone.
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !expected_partial.exists() && !std::path::Path::new(&format!("{}.part", expected_partial.display())).exists(),
        "expected the cancelled item's partial file to be gone from disk"
    );

    poll_item(&h.db, second.id, Duration::from_secs(60), |i| {
        matches!(i.stage.as_str(), "completed" | "error")
    });
    h.cleanup();
}

// --- AC3: remove deletes the row; it does not reappear after restart ------

#[tokio::test]
#[ignore] // network + real yt-dlp process
async fn remove_deletes_row_and_it_stays_gone_after_reopen() {
    let h = Harness::new("ac3_remove");
    let item = h.add(2, "sample_ac3.%(ext)s");
    let db_path = h.work_dir.join("begirex.db");

    queue_manager::remove_item(
        Arc::clone(&h.db),
        Arc::clone(&h.emitter),
        h.binaries.clone(),
        2,
        Arc::clone(&h.registry),
        item.id,
    )
    .await
    .unwrap();

    let conn = h.db.lock().unwrap();
    assert!(persistence::get_item(&conn, item.id).is_err());
    drop(conn);

    // Simulate a restart: fresh connection against the same DB file.
    let reopened = Connection::open(&db_path).unwrap();
    assert!(persistence::get_item(&reopened, item.id).is_err());
    let all = persistence::list_items(&reopened, None).unwrap();
    assert!(all.iter().all(|i| i.id != item.id));

    h.cleanup();
}

// --- AC4: reordering a queued item above another changes which starts next -

#[tokio::test]
#[ignore] // network + real yt-dlp processes
async fn reorder_queued_item_above_another_changes_which_starts_next() {
    let h = Harness::new("ac4_reorder");
    // N=1 so only the first add starts; the next two stay queued in add order.
    let running = h.add(1, "sample_ac4_running.%(ext)s");
    let a = h.add(1, "sample_ac4_a.%(ext)s");
    let b = h.add(1, "sample_ac4_b.%(ext)s");

    // Both a and b are queued, a before b.
    let (a_before, b_before) = {
        let conn = h.db.lock().unwrap();
        (
            persistence::get_item(&conn, a.id).unwrap(),
            persistence::get_item(&conn, b.id).unwrap(),
        )
    };
    assert_eq!(a_before.stage, "queued");
    assert_eq!(b_before.stage, "queued");
    assert!(a_before.queue_position < b_before.queue_position);

    // Move b above a.
    queue_manager::reorder_item(Arc::clone(&h.db), b.id, a_before.queue_position as i64).unwrap();

    let (a_after, b_after) = {
        let conn = h.db.lock().unwrap();
        (
            persistence::get_item(&conn, a.id).unwrap(),
            persistence::get_item(&conn, b.id).unwrap(),
        )
    };
    assert!(b_after.queue_position < a_after.queue_position, "b should now sort before a");

    // Free the slot: cancel the running item; the scheduler should now pick
    // b (now lowest-position queued), not a.
    queue_manager::cancel_item(
        Arc::clone(&h.db),
        Arc::clone(&h.emitter),
        h.binaries.clone(),
        1,
        Arc::clone(&h.registry),
        running.id,
    )
    .await
    .unwrap();

    let b_started = poll_item(&h.db, b.id, Duration::from_secs(10), |i| i.stage != "queued");
    assert_eq!(b_started.stage, "downloading", "b (reordered above a) should start next");
    let a_final = {
        let conn = h.db.lock().unwrap();
        persistence::get_item(&conn, a.id).unwrap()
    };
    assert_eq!(a_final.stage, "queued", "a should still be waiting");

    // Cleanup: cancel b to stop its child before removing the temp dir.
    queue_manager::cancel_item(
        Arc::clone(&h.db),
        Arc::clone(&h.emitter),
        h.binaries.clone(),
        1,
        Arc::clone(&h.registry),
        b.id,
    )
    .await
    .ok();
    h.cleanup();
}

// --- AC5: set_concurrency(0) is VALIDATION-rejected (guarded at the ipc ----
// trust boundary — this test proves the queue_manager side never kills an
// in-flight item on a decrease, and an increase fills a slot).

#[tokio::test]
#[ignore] // network + real yt-dlp processes
async fn decreasing_concurrency_never_kills_in_flight_item_increasing_fills_a_slot() {
    let h = Harness::new("ac5_concurrency");
    let running = h.add(2, "sample_ac5_running.%(ext)s");
    let queued = h.add(2, "sample_ac5_queued.%(ext)s");
    // Force a second item queued behind the running one.
    let queued2 = h.add(1, "sample_ac5_queued2.%(ext)s");
    let _ = queued; // first add started immediately at N=2; keep name for clarity

    // Decrease N to 1: the already-running item must not be killed.
    queue_manager::set_concurrency(
        Arc::clone(&h.db),
        Arc::clone(&h.emitter),
        h.binaries.clone(),
        Arc::clone(&h.registry),
        1,
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let running_after_decrease = {
        let conn = h.db.lock().unwrap();
        persistence::get_item(&conn, running.id).unwrap()
    };
    assert!(
        matches!(running_after_decrease.stage.as_str(), "downloading" | "merging" | "completed"),
        "decreasing N must not kill an in-flight item, got {}",
        running_after_decrease.stage
    );

    // Increase N back up: the still-queued item should get picked up.
    queue_manager::set_concurrency(
        Arc::clone(&h.db),
        Arc::clone(&h.emitter),
        h.binaries.clone(),
        Arc::clone(&h.registry),
        3,
    )
    .unwrap();
    let queued2_after = poll_item(&h.db, queued2.id, Duration::from_secs(15), |i| i.stage != "queued");
    assert_ne!(queued2_after.stage, "queued", "increasing N should immediately fill the freed-up slot");

    for id in [running.id, queued.id, queued2.id] {
        poll_item(&h.db, id, Duration::from_secs(60), |i| {
            matches!(i.stage.as_str(), "completed" | "error")
        });
    }
    h.cleanup();
}

// --- bulk_action: applies the verb to every id, best-effort ----------------

#[tokio::test]
#[ignore] // network + real yt-dlp processes
async fn bulk_pause_pauses_every_active_item() {
    let h = Harness::new("bulk_pause");
    let a = h.add(2, "sample_bulk_a.%(ext)s");
    let b = h.add(2, "sample_bulk_b.%(ext)s");

    poll_item(&h.db, a.id, Duration::from_secs(30), |i| i.downloaded_bytes > 0);
    poll_item(&h.db, b.id, Duration::from_secs(30), |i| i.downloaded_bytes > 0);

    let updated = queue_manager::bulk_action(
        Arc::clone(&h.db),
        Arc::clone(&h.emitter),
        h.binaries.clone(),
        2,
        Arc::clone(&h.registry),
        vec![a.id, b.id],
        BulkVerb::Pause,
    )
    .await;

    assert_eq!(updated.len(), 2);
    assert!(updated.iter().all(|i| i.stage == "paused"));

    let (a_final, b_final) = {
        let conn = h.db.lock().unwrap();
        (
            persistence::get_item(&conn, a.id).unwrap(),
            persistence::get_item(&conn, b.id).unwrap(),
        )
    };
    assert_eq!(a_final.stage, "paused");
    assert_eq!(b_final.stage, "paused");

    h.cleanup();
}
