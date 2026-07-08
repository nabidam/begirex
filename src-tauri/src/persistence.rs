//! SQLite open/migrate/seed (ARCHITECTURE §3, §9). Owns the DB file lifecycle;
//! CRUD for items/presets/logs lands in later tasks (T1+).

use rusqlite::Connection;
use std::path::Path;

const MIGRATION_001: &str = include_str!("../migrations/001_init.sql");

/// Opens (creating if absent) the DB at `path`, runs migrations if needed, and
/// seeds default data on first run. Idempotent — safe to call every launch.
pub fn open_and_init(path: &Path, default_output_dir: &str) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    migrate(&conn)?;
    seed(&conn, default_output_dir)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let already_migrated: bool = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='items'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)?;
    if !already_migrated {
        conn.execute_batch(MIGRATION_001)?;
    }
    Ok(())
}

fn seed(conn: &Connection, default_output_dir: &str) -> rusqlite::Result<()> {
    let preset_count: i64 = conn.query_row("SELECT count(*) FROM presets", [], |row| row.get(0))?;
    if preset_count == 0 {
        let now = now_unix();
        conn.execute(
            "INSERT INTO presets (name, format_expr, output_template, is_default, created_at, updated_at)
             VALUES ('Default', 'bv*+ba/b', '%(title)s.%(ext)s', 1, ?1, ?1)",
            [now],
        )?;
    }

    let defaults: [(&str, &str); 4] = [
        ("default_concurrency", "2"),
        ("default_output_dir", default_output_dir),
        ("default_output_template", "%(title)s.%(ext)s"),
        ("build_flavor", "light"),
    ];
    for (key, value) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            [key, value],
        )?;
    }
    Ok(())
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Reads a single `settings` value; `None` if the key is unset (distinct from
/// an empty string). Business logic (validation, defaults) is the caller's.
pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .map(Some)
    .or_else(|err| match err {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

/// Upserts a single `settings` value.
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

/// Runs just the schema migration (no seed) against an in-memory connection,
/// for other modules' unit tests that need a real `settings` table without
/// pulling in full seed data.
#[cfg(test)]
pub(crate) fn migrate_for_test(conn: &Connection) {
    migrate(conn).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed(&conn, "/home/test/Downloads").unwrap();
        seed(&conn, "/home/test/Downloads").unwrap(); // second call must not re-seed

        let preset_names: Vec<String> = conn
            .prepare("SELECT name FROM presets")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(preset_names, vec!["Default".to_string()]);

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        // in-memory DBs report "memory" for journal_mode regardless of the PRAGMA
        // in the migration file (WAL requires a real file); verified against a
        // temp file below instead.
        let _ = journal_mode;

        let concurrency: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'default_concurrency'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(concurrency, "2");
    }

    #[test]
    fn wal_mode_on_real_file() {
        let dir = std::env::temp_dir().join(format!("begirex_test_{}", now_unix()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("begirex.db");

        let conn = open_and_init(&db_path, "/home/test/Downloads").unwrap();
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode, "wal");
        drop(conn);

        // Second open must not re-seed.
        let conn2 = open_and_init(&db_path, "/home/test/Downloads").unwrap();
        let preset_count: i64 = conn2.query_row("SELECT count(*) FROM presets", [], |row| row.get(0)).unwrap();
        assert_eq!(preset_count, 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
