// App setup: opens/migrates/seeds the DB in app-data dir on launch, then runs
// the Tauri app. IPC commands (T1) are registered via `invoke_handler`.

pub mod binary_manager;
pub mod engine_supervisor;
pub mod error;
mod ipc;
pub mod persistence;
pub mod progress_parser;
pub mod settings_service;

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

            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
