// App setup: opens/migrates/seeds the DB in app-data dir on launch, then runs
// the Tauri app. IPC commands (T1) are registered via `invoke_handler`.

pub mod binary_manager;
pub mod error;
mod ipc;
pub mod persistence;
pub mod settings_service;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;

/// App-wide state managed by Tauri and borrowed by command handlers. Owns the
/// single DB connection opened at launch (per CONVENTIONS: ipc holds no
/// state itself, modules/state do).
pub struct AppState {
    pub db: Mutex<Connection>,
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            ipc::detect_binaries,
            ipc::set_binary_path,
            ipc::recheck_binaries,
            ipc::get_settings,
            ipc::update_settings,
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
                db: Mutex::new(conn),
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
