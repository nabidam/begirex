// App setup: opens/migrates/seeds the DB in app-data dir on launch, then runs
// the Tauri app. IPC commands (T1) are registered via `invoke_handler`.

pub mod binary_manager;
pub mod engine_supervisor;
pub mod error;
mod ipc;
pub mod persistence;
pub mod progress_parser;
pub mod queue_manager;
pub mod settings_service;

use engine_supervisor::Emitter;
use ipc::TauriEmitter;
use queue_manager::BinaryPaths;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// App-wide state managed by Tauri and borrowed by command handlers. Owns the
/// single DB connection opened at launch (per CONVENTIONS: ipc holds no
/// state itself, modules/state do).
///
/// `db` is `Arc<Mutex<..>>` (not just `Mutex<..>`) so `add_download` can
/// clone the connection handle into a `tauri::async_runtime::spawn`ed task
/// that outlives the command call — everything else still calls
/// `state.db.lock()` unchanged since `Arc` derefs to `Mutex`.
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            ipc::detect_binaries,
            ipc::set_binary_path,
            ipc::recheck_binaries,
            ipc::get_settings,
            ipc::update_settings,
            ipc::add_download,
            ipc::list_items,
            ipc::get_item,
        ])
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir must be resolvable");
            std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");

            let downloads_dir = app
                .path()
                .download_dir()
                .expect("downloads dir must be resolvable");

            let db_path = app_data_dir.join("begirex.db");
            let conn = persistence::open_and_init(&db_path, &downloads_dir.to_string_lossy())
                .expect("failed to open/migrate/seed database");

            let db = Arc::new(Mutex::new(conn));
            app.manage(AppState { db: Arc::clone(&db) });

            // T3 launch reconcile (ARCHITECTURE §8): any item left
            // `downloading`/`merging` from a prior crash gets paused (so
            // `list_items` shows correct checkpointed bytes right away) then
            // resumed via the same scheduler slot-refill uses. Spawned async
            // so a slow/failed binary lookup never blocks app startup.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let (n_slots, ytdlp_path, ffmpeg_path) = {
                    let conn = db.lock().expect("db mutex poisoned");
                    let settings = match settings_service::get_settings(&conn) {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                    let ytdlp_path = binary_manager::detect(&conn, &binary_manager::Which::Ytdlp)
                        .ok()
                        .and_then(|s| s.path);
                    let ffmpeg_path = binary_manager::detect(&conn, &binary_manager::Which::Ffmpeg)
                        .ok()
                        .and_then(|s| s.path);
                    (settings.default_concurrency, ytdlp_path, ffmpeg_path)
                };
                // No point reconciling if a binary isn't even resolvable yet
                // (e.g. first launch, S1 onboarding not done) — nothing to
                // respawn with; the dirty rows just stay as-is until the user
                // sets binary paths and relaunches, no data loss either way.
                let (ytdlp_path, ffmpeg_path) = match (ytdlp_path, ffmpeg_path) {
                    (Some(y), Some(f)) => (y, f),
                    _ => return,
                };
                let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app_handle));
                let binaries = BinaryPaths { ytdlp_path, ffmpeg_path };
                let _ = queue_manager::reconcile_and_resume(db, emitter, binaries, n_slots);
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
