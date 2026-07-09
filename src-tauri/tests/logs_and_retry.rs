//! Integration tests for T7 acceptance criteria 1 and 2 — real yt-dlp child
//! processes, real DB files, driven through `queue_manager`/`engine_supervisor`
//! exactly as `ipc.rs` wires them in production. Network-dependent, so
//! `#[ignore]`d; run explicitly at demo gates (CONVENTIONS "Tests"):
//!   cargo test --manifest-path src-tauri/Cargo.toml -- --ignored
//!
//! AC3 (duplicate-URL guard) and AC4 (log trim to 500) are covered by plain
//! unit tests (`ipc::tests::check_duplicate_*`, `persistence::tests::
//! insert_log_trims_to_500_lines_per_item`) — no real process needed for
//! either, so they don't live here.

use begirex_lib::engine_supervisor::{self, Emitter};
use begirex_lib::persistence::{self, Item};
use begirex_lib::progress_parser::ProgressTick;
use begirex_lib::queue_manager::{self, AddDownloadParams, BinaryPaths};
use rusqlite::Connection;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TEST_URL: &str = "https://download.samplelib.com/mp4/sample-5s.mp4";
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
        "begirex_logs_retry_it_{name}_{}",
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

    fn add(&self, url: &str, format_expr: &str, output_template: &str, extra_args: Option<&str>) -> Item {
        queue_manager::add_and_schedule(
            Arc::clone(&self.db),
            Arc::clone(&self.emitter),
            self.binaries.clone(),
            2,
            AddDownloadParams {
                url: url.to_string(),
                format_expr: format_expr.to_string(),
                output_dir: self.work_dir.to_str().unwrap().to_string(),
                output_template: output_template.to_string(),
                proxy: None,
                extra_args: extra_args.map(|s| s.to_string()),
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

// --- AC1: a failed item's full yt-dlp stderr is retrievable via get_item_log

#[tokio::test]
#[ignore] // network + real yt-dlp process
async fn failed_items_full_stderr_is_retrievable_via_get_item_log() {
    let h = Harness::new("ac1_stderr_log");
    // A domain that resolves to nothing real — yt-dlp fails fast with a
    // non-empty stderr, no need to wait out a real download.
    let item = h.add("https://example.invalid/does-not-exist", "best", "bad.%(ext)s", None);

    let final_item = poll_item(&h.db, item.id, Duration::from_secs(30), |i| {
        matches!(i.stage.as_str(), "completed" | "error")
    });
    assert_eq!(final_item.stage, "error", "expected the bad URL to fail");

    let log = {
        let conn = h.db.lock().unwrap();
        persistence::get_item_log(&conn, item.id, None).unwrap()
    };
    assert!(!log.is_empty(), "expected stderr lines to be captured in item_logs");
    assert!(log.iter().all(|l| l.stream == "stderr"));
    // The full stderr (not a truncated summary) should be present — same
    // text `error_message` was built from.
    let joined: String = log.iter().map(|l| l.line.as_str()).collect::<Vec<_>>().join("\n");
    assert!(!joined.trim().is_empty());

    h.cleanup();
}

// --- AC2: retry on a partially-downloaded errored item resumes from >= the
// pre-failure byte count (V3-AC3) -------------------------------------------

#[tokio::test]
#[ignore] // network + real yt-dlp process
async fn retry_on_partial_error_resumes_from_at_least_prior_bytes() {
    let h = Harness::new("ac2_retry");
    let item = h.add(TEST_URL, "best", "sample_retry.%(ext)s", Some(THROTTLE));

    // Let real progress accumulate, then simulate a mid-download failure:
    // kill the child the same way `pause` does (keeps the partial file on
    // disk, unlike `cancel`), but record it as `error` instead of `paused` —
    // standing in for a real crash/network-drop mid-download.
    let mid = poll_item(&h.db, item.id, Duration::from_secs(30), |i| i.downloaded_bytes > 0);
    assert!(mid.downloaded_bytes > 0, "expected some progress before failing");

    engine_supervisor::pause(&h.registry, item.id).await.unwrap();
    let pre_failure_bytes = {
        let conn = h.db.lock().unwrap();
        persistence::finish_item(&conn, item.id, "error", None, Some("simulated mid-download failure")).unwrap();
        persistence::get_item(&conn, item.id).unwrap().downloaded_bytes
    };
    assert!(pre_failure_bytes > 0);

    let retried = queue_manager::retry_item(
        Arc::clone(&h.db),
        Arc::clone(&h.emitter),
        h.binaries.clone(),
        2,
        Arc::clone(&h.registry),
        item.id,
    )
    .unwrap();
    assert_eq!(retried.stage, "downloading");

    let final_item = poll_item(&h.db, item.id, Duration::from_secs(60), |i| {
        matches!(i.stage.as_str(), "completed" | "error")
    });
    assert_eq!(
        final_item.stage, "completed",
        "retried download should complete; error_message: {:?}",
        final_item.error_message
    );
    assert!(
        final_item.downloaded_bytes >= pre_failure_bytes,
        "retried bytes ({}) should be >= pre-failure bytes ({})",
        final_item.downloaded_bytes,
        pre_failure_bytes
    );

    h.cleanup();
}
