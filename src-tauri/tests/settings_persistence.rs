//! Integration test for T1 acceptance criterion 3: `update_settings` then
//! `get_settings` round-trips `global_proxy` through a real SQLite file,
//! verified by closing the connection and reopening it fresh — simulating a
//! process restart (T0's `wal_mode_on_real_file` test pattern).

use begirex_lib::persistence;
use begirex_lib::settings_service::{get_settings, update_settings, SettingsUpdate};

#[test]
fn global_proxy_survives_a_fresh_connection_open() {
    let dir = std::env::temp_dir().join(format!(
        "begirex_it_settings_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("begirex.db");

    let conn = persistence::open_and_init(&db_path, "/tmp/downloads").unwrap();
    let updated = update_settings(
        &conn,
        SettingsUpdate {
            global_proxy: Some("http://127.0.0.1:8080".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(updated.global_proxy.as_deref(), Some("http://127.0.0.1:8080"));
    drop(conn);

    // Fresh connection to the same file — simulates a process restart.
    // `open_and_init` is idempotent (T0), so this must not re-seed or wipe
    // the value just written.
    let conn2 = persistence::open_and_init(&db_path, "/tmp/downloads").unwrap();
    let fetched = get_settings(&conn2).unwrap();
    assert_eq!(fetched.global_proxy.as_deref(), Some("http://127.0.0.1:8080"));

    std::fs::remove_dir_all(&dir).ok();
}
