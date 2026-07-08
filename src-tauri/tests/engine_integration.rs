//! Integration tests for T2 acceptance criteria — real yt-dlp child process,
//! real DB file, real filesystem. Network-dependent, so `#[ignore]`d; run
//! explicitly at demo gates (CONVENTIONS "Tests"):
//!   cargo test --manifest-path src-tauri/Cargo.toml -- --ignored
//!
//! Test asset choice: `https://download.samplelib.com/mp4/sample-5s.mp4` via
//! yt-dlp's generic extractor (not a YouTube URL). Reasons, confirmed in this
//! sandbox before writing these tests:
//! - Tiny (2.72MiB) and completes in ~1-2s — fast demo.
//! - Single-format direct file: no separate video+audio streams to merge, so
//!   yt-dlp's `--newline` percent goes 0%->100% exactly once and stays
//!   genuinely monotonic for the whole item lifecycle. A YouTube bv+ba
//!   download instead does two full 0%->100% passes (video, then audio,
//!   *then* merges) — percent resets between them, which would make a
//!   literal "monotonically increasing across the whole run" assertion
//!   false for a reason that has nothing to do with our parser/supervisor
//!   being wrong. The merge-detection path itself is covered by a real
//!   captured `[Merger]` fixture in progress_parser's unit tests instead
//!   (see src/progress_parser.rs), where it can be tested in isolation.
//! - Stable/always-available and doesn't depend on YouTube's extractor
//!   staying unbroken (this sandbox's yt-dlp needs a JS runtime for many
//!   YouTube videos and none is installed — confirmed via `which deno`).

use begirex_lib::engine_supervisor::{self, Emitter, SpawnParams};
use begirex_lib::persistence::{self, Item, NewItem};
use begirex_lib::progress_parser::ProgressTick;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Fresh, empty per-test registry (T6) — these T2 tests don't exercise
/// pause/cancel, they just need something to satisfy `run_download`'s
/// now-required registry param.
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
        "begirex_engine_it_{name}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Records every emitted event — stands in for the real Tauri event bus
/// (T2 acceptance criterion 3: engine_supervisor's emit logic is decoupled
/// from `tauri::AppHandle` behind the `Emitter` trait specifically so this
/// kind of plain in-memory recorder can drive it in tests).
#[derive(Default)]
struct RecordingEmitter {
    progress_percents: Mutex<Vec<f64>>,
    stage_events: Mutex<Vec<(String, Option<String>)>>,
}

impl Emitter for RecordingEmitter {
    fn emit_progress(&self, _item_id: i64, tick: &ProgressTick) {
        self.progress_percents.lock().unwrap().push(tick.percent);
    }
    fn emit_stage_changed(&self, _item_id: i64, stage: &str, error_message: Option<&str>) {
        self.stage_events
            .lock()
            .unwrap()
            .push((stage.to_string(), error_message.map(|s| s.to_string())));
    }
    fn emit_item_added(&self, _item: &Item) {}
    fn emit_item_removed(&self, _item_id: i64) {}
    fn emit_log_line(&self, _item_id: i64, _stream: &str, _line: &str) {}
}

#[tokio::test]
#[ignore] // network + real yt-dlp process
async fn add_download_completes_with_monotonic_progress_and_file_on_disk() {
    let ytdlp_path = resolve_on_path("yt-dlp").expect("yt-dlp must be on PATH for this test");
    let ffmpeg_path = resolve_on_path("ffmpeg").expect("ffmpeg must be on PATH for this test");

    let work_dir = temp_dir("happy_path");
    let db_path = work_dir.join("begirex.db");
    let conn = persistence::open_and_init(&db_path, work_dir.to_str().unwrap()).unwrap();

    let item = persistence::insert_item(
        &conn,
        NewItem {
            url: TEST_URL.to_string(),
            format_expr: "best".to_string(),
            output_dir: work_dir.to_str().unwrap().to_string(),
            output_template: "sample.%(ext)s".to_string(),
            proxy: None,
            extra_args: None,
            preset_id: None,
            stage: "downloading".to_string(),
        },
    )
    .unwrap();
    let db = Arc::new(Mutex::new(conn));

    let emitter = Arc::new(RecordingEmitter::default());
    engine_supervisor::run_download(
        Arc::clone(&db),
        item.id,
        SpawnParams {
            ytdlp_path,
            ffmpeg_path,
            url: TEST_URL.to_string(),
            format_expr: "best".to_string(),
            output_dir: work_dir.to_str().unwrap().to_string(),
            output_template: "sample.%(ext)s".to_string(),
            proxy: None,
            extra_args: None,
        },
        Arc::clone(&emitter) as Arc<dyn Emitter>,
        empty_registry(),
    )
    .await;

    // (a) percent increases monotonically across observed progress events.
    let percents = emitter.progress_percents.lock().unwrap().clone();
    assert!(!percents.is_empty(), "expected at least one progress event");
    for pair in percents.windows(2) {
        assert!(
            pair[1] >= pair[0],
            "percent must not decrease: {:?} -> {:?} in {:?}",
            pair[0],
            pair[1],
            percents
        );
    }

    // (b) final stage is `completed` (via both the emitted event and the DB row).
    let stage_events = emitter.stage_events.lock().unwrap();
    assert_eq!(
        stage_events.last().map(|(stage, _)| stage.as_str()),
        Some("completed")
    );
    drop(stage_events);

    let conn = db.lock().unwrap();
    let final_item = persistence::get_item(&conn, item.id).unwrap();
    assert_eq!(final_item.stage, "completed");

    // (c) the output file actually exists on disk at the resolved path.
    let output_path = final_item
        .output_path
        .clone()
        .expect("completed item must have a resolved output_path");
    assert!(
        std::path::Path::new(&output_path).is_file(),
        "resolved output_path {output_path} does not exist on disk"
    );
    assert!(std::fs::metadata(&output_path).unwrap().len() > 0);

    drop(conn);
    std::fs::remove_dir_all(&work_dir).ok();
}

#[tokio::test]
#[ignore] // network + real yt-dlp process
async fn invalid_format_expr_yields_error_stage_with_real_stderr() {
    let ytdlp_path = resolve_on_path("yt-dlp").expect("yt-dlp must be on PATH for this test");
    let ffmpeg_path = resolve_on_path("ffmpeg").expect("ffmpeg must be on PATH for this test");

    let work_dir = temp_dir("invalid_format");
    let db_path = work_dir.join("begirex.db");
    let conn = persistence::open_and_init(&db_path, work_dir.to_str().unwrap()).unwrap();

    // Nonsense format selector yt-dlp will reject outright.
    let bogus_format = "bestvideo[height<=99999999]+bogus_selector_xyz";

    let item = persistence::insert_item(
        &conn,
        NewItem {
            url: TEST_URL.to_string(),
            format_expr: bogus_format.to_string(),
            output_dir: work_dir.to_str().unwrap().to_string(),
            output_template: "sample.%(ext)s".to_string(),
            proxy: None,
            extra_args: None,
            preset_id: None,
            stage: "downloading".to_string(),
        },
    )
    .unwrap();
    let db = Arc::new(Mutex::new(conn));

    let emitter = Arc::new(RecordingEmitter::default());
    engine_supervisor::run_download(
        Arc::clone(&db),
        item.id,
        SpawnParams {
            ytdlp_path,
            ffmpeg_path,
            url: TEST_URL.to_string(),
            format_expr: bogus_format.to_string(),
            output_dir: work_dir.to_str().unwrap().to_string(),
            output_template: "sample.%(ext)s".to_string(),
            proxy: None,
            extra_args: None,
        },
        Arc::clone(&emitter) as Arc<dyn Emitter>,
        empty_registry(),
    )
    .await;

    let stage_events = emitter.stage_events.lock().unwrap();
    let (last_stage, last_error) = stage_events.last().expect("expected a stage_changed event");
    assert_eq!(last_stage, "error");
    let emitted_message = last_error
        .clone()
        .expect("error stage_changed must carry error_message");
    // Real yt-dlp stderr text, not a paraphrase (CONVENTIONS: "never
    // paraphrase yt-dlp"). Confirmed once in this sandbox that this is
    // yt-dlp's actual wording for an unsatisfiable format selector.
    assert!(
        emitted_message.contains("Requested format is not available"),
        "expected real yt-dlp stderr text, got: {emitted_message}"
    );
    drop(stage_events);

    let conn = db.lock().unwrap();
    let final_item = persistence::get_item(&conn, item.id).unwrap();
    assert_eq!(final_item.stage, "error");
    assert!(final_item
        .error_message
        .as_deref()
        .unwrap()
        .contains("Requested format is not available"));

    drop(conn);
    std::fs::remove_dir_all(&work_dir).ok();
}
