// App setup: opens/migrates/seeds the DB in app-data dir on launch, then runs
// the Tauri app. IPC command registration starts in T1.

mod persistence;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
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
            persistence::open_and_init(&db_path, &downloads_dir.to_string_lossy())
                .expect("failed to open/migrate/seed database");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
