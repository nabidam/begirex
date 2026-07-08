//! SQLite open/migrate/seed (ARCHITECTURE §3, §9). Owns the DB file lifecycle;
//! CRUD for items/presets/logs lands in later tasks (T1+).
//!
//! T2 adds: item CRUD (insert/get/list) and progress checkpoint writes. Pure
//! CRUD + queries only — no business rules (spawn-if-<2-active scheduling
//! logic lives in ipc.rs's `add_download`, not here).

use crate::error::AppError;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

/// One row of the `items` table (ARCHITECTURE §3), serialized verbatim for
/// `list_items`/`get_item`/`add_download` IPC responses.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Item {
    pub id: i64,
    pub url: String,
    pub playlist_id: Option<String>,
    pub title: Option<String>,
    pub stage: String,
    pub format_expr: String,
    pub output_dir: String,
    pub output_template: String,
    pub proxy: Option<String>,
    pub extra_args: Option<String>,
    pub preset_id: Option<i64>,
    pub total_bytes: Option<i64>,
    pub downloaded_bytes: i64,
    pub percent: f64,
    pub speed_bps: Option<i64>,
    pub eta_seconds: Option<i64>,
    pub resume_capable: bool,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
    pub queue_position: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Fields needed to insert a new item; the rest (`id`, timestamps,
/// `queue_position`, progress columns) are computed/defaulted here.
pub struct NewItem {
    pub url: String,
    pub format_expr: String,
    pub output_dir: String,
    pub output_template: String,
    pub proxy: Option<String>,
    pub extra_args: Option<String>,
    pub preset_id: Option<i64>,
    pub stage: String, // "downloading" or "queued", decided by the caller (spawn-if-<2-active)
}

const ITEM_COLUMNS: &str = "id, url, playlist_id, title, stage, format_expr, output_dir,
     output_template, proxy, extra_args, preset_id, total_bytes, downloaded_bytes,
     percent, speed_bps, eta_seconds, resume_capable, output_path, error_message,
     queue_position, created_at, updated_at";

fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: row.get(0)?,
        url: row.get(1)?,
        playlist_id: row.get(2)?,
        title: row.get(3)?,
        stage: row.get(4)?,
        format_expr: row.get(5)?,
        output_dir: row.get(6)?,
        output_template: row.get(7)?,
        proxy: row.get(8)?,
        extra_args: row.get(9)?,
        preset_id: row.get(10)?,
        total_bytes: row.get(11)?,
        downloaded_bytes: row.get(12)?,
        percent: row.get(13)?,
        speed_bps: row.get(14)?,
        eta_seconds: row.get(15)?,
        resume_capable: row.get::<_, i64>(16)? != 0,
        output_path: row.get(17)?,
        error_message: row.get(18)?,
        queue_position: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
    })
}

/// Inserts a new item, appended to the end of the queue
/// (`queue_position` = max existing + 1, or 0 if empty).
pub fn insert_item(conn: &Connection, new: NewItem) -> Result<Item, AppError> {
    let now = now_unix();
    let next_position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(queue_position) + 1, 0) FROM items",
        [],
        |row| row.get(0),
    )?;

    conn.execute(
        "INSERT INTO items (url, playlist_id, title, stage, format_expr, output_dir,
            output_template, proxy, extra_args, preset_id, total_bytes, downloaded_bytes,
            percent, speed_bps, eta_seconds, resume_capable, output_path, error_message,
            queue_position, created_at, updated_at)
         VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, 0, 0, NULL, NULL, 1, NULL, NULL, ?9, ?10, ?10)",
        rusqlite::params![
            new.url,
            new.stage,
            new.format_expr,
            new.output_dir,
            new.output_template,
            new.proxy,
            new.extra_args,
            new.preset_id,
            next_position,
            now,
        ],
    )?;

    let id = conn.last_insert_rowid();
    get_item(conn, id)
}

pub fn get_item(conn: &Connection, id: i64) -> Result<Item, AppError> {
    conn.query_row(
        &format!("SELECT {ITEM_COLUMNS} FROM items WHERE id = ?1"),
        [id],
        row_to_item,
    )
    .optional()?
    .ok_or_else(|| AppError::DbError {
        message: format!("item {id} not found"),
    })
}

/// Lists items, optionally filtered to a single `stage` value ("all" or
/// `None` = no filter), ordered by queue position.
pub fn list_items(conn: &Connection, filter: Option<&str>) -> Result<Vec<Item>, AppError> {
    let items = match filter {
        Some(stage) if stage != "all" => conn
            .prepare(&format!(
                "SELECT {ITEM_COLUMNS} FROM items WHERE stage = ?1 ORDER BY queue_position"
            ))?
            .query_map([stage], row_to_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        _ => conn
            .prepare(&format!(
                "SELECT {ITEM_COLUMNS} FROM items ORDER BY queue_position"
            ))?
            .query_map([], row_to_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?,
    };
    Ok(items)
}

/// Counts items currently occupying an active download slot
/// (`downloading` or `merging`) — the input to "spawn if < 2 active".
pub fn count_active_items(conn: &Connection) -> Result<i64, AppError> {
    Ok(conn.query_row(
        "SELECT count(*) FROM items WHERE stage IN ('downloading', 'merging')",
        [],
        |row| row.get(0),
    )?)
}

/// Checkpoints a progress tick (§8 durability): bytes/percent/speed/eta,
/// stage, and `updated_at`. Called on ticks that cross the throttle
/// threshold, not on every parsed line.
#[allow(clippy::too_many_arguments)]
pub fn checkpoint_progress(
    conn: &Connection,
    id: i64,
    stage: &str,
    downloaded_bytes: Option<i64>,
    total_bytes: Option<i64>,
    percent: f64,
    speed_bps: Option<i64>,
    eta_seconds: Option<i64>,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE items SET stage = ?1, downloaded_bytes = COALESCE(?2, downloaded_bytes),
            total_bytes = COALESCE(?3, total_bytes), percent = ?4, speed_bps = ?5,
            eta_seconds = ?6, updated_at = ?7
         WHERE id = ?8",
        rusqlite::params![
            stage,
            downloaded_bytes,
            total_bytes,
            percent,
            speed_bps,
            eta_seconds,
            now_unix(),
            id
        ],
    )?;
    Ok(())
}

/// Sets the terminal stage (`completed`/`error`/`cancelled`) plus
/// `output_path`/`error_message` as applicable.
pub fn finish_item(
    conn: &Connection,
    id: i64,
    stage: &str,
    output_path: Option<&str>,
    error_message: Option<&str>,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE items SET stage = ?1, output_path = COALESCE(?2, output_path),
            error_message = ?3, updated_at = ?4 WHERE id = ?5",
        rusqlite::params![stage, output_path, error_message, now_unix(), id],
    )?;
    Ok(())
}

/// Sets `stage` only, leaving every other column untouched — the generic
/// stage-transition write used by queue_manager's scheduler (`queued` ->
/// `downloading` on slot-refill, `paused` -> `downloading` on reconcile
/// resume). Terminal transitions with output_path/error_message still go
/// through `finish_item`.
pub fn set_stage(conn: &Connection, id: i64, stage: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE items SET stage = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![stage, now_unix(), id],
    )?;
    Ok(())
}

/// Finds items left `downloading`/`merging` — crash-dirty because the yt-dlp
/// supervising process died with the app (§8 durability). Ordered by
/// `queue_position` so launch-reconcile resumes in a stable, predictable
/// order (T3).
pub fn find_dirty_items(conn: &Connection) -> Result<Vec<Item>, AppError> {
    let items = conn
        .prepare(&format!(
            "SELECT {ITEM_COLUMNS} FROM items WHERE stage IN ('downloading', 'merging')
             ORDER BY queue_position"
        ))?
        .query_map([], row_to_item)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(items)
}

/// Bulk-transitions every crash-dirty (`downloading`/`merging`) item to
/// `paused` (§8's literal wording) so `list_items` shows correct
/// last-checkpointed bytes immediately, before the scheduler decides which
/// of them to resume.
pub fn pause_dirty_items(conn: &Connection) -> Result<(), AppError> {
    conn.execute(
        "UPDATE items SET stage = 'paused', updated_at = ?1 WHERE stage IN ('downloading', 'merging')",
        [now_unix()],
    )?;
    Ok(())
}

/// Deletes an item row (T6 `remove_item` — K2-AC8). `item_logs` rows cascade
/// via the FK's `ON DELETE CASCADE`.
pub fn delete_item(conn: &Connection, id: i64) -> Result<(), AppError> {
    conn.execute("DELETE FROM items WHERE id = ?1", [id])?;
    Ok(())
}

/// Renumbers `queue_position` to `0..len` following `ordered_ids`'s order
/// (T6 `reorder_item` — K2-AC9). Caller computes the new order; this just
/// writes it.
pub fn reorder_items(conn: &Connection, ordered_ids: &[i64]) -> Result<(), AppError> {
    let now = now_unix();
    for (position, id) in ordered_ids.iter().enumerate() {
        conn.execute(
            "UPDATE items SET queue_position = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![position as i64, now, id],
        )?;
    }
    Ok(())
}

/// Ring-buffer cap per item (ARCHITECTURE §3 "trimmed to last K lines/item").
const LOG_LINES_PER_ITEM: i64 = 500;

/// Appends one captured stdout/stderr line to `item_logs`, then trims that
/// item's log back down to `LOG_LINES_PER_ITEM` (oldest first) — keeps the
/// table bounded regardless of how chatty a given yt-dlp run is.
pub fn insert_log(conn: &Connection, item_id: i64, stream: &str, line: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO item_logs (item_id, ts, stream, line) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![item_id, now_unix(), stream, line],
    )?;
    conn.execute(
        "DELETE FROM item_logs WHERE item_id = ?1 AND id NOT IN (
             SELECT id FROM item_logs WHERE item_id = ?1 ORDER BY id DESC LIMIT ?2
         )",
        rusqlite::params![item_id, LOG_LINES_PER_ITEM],
    )?;
    Ok(())
}

/// One row of `item_logs`, ordered oldest-first for `get_item_log` (S5 log
/// disclosure reads top-to-bottom).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LogLine {
    pub ts: i64,
    pub stream: String,
    pub line: String,
}

/// Reads back an item's stored log lines, oldest first. `tail` (if given)
/// returns only the last N lines (still chronological in the result) —
/// `None` returns everything currently retained (already capped at
/// `LOG_LINES_PER_ITEM` by `insert_log`).
pub fn get_item_log(conn: &Connection, item_id: i64, tail: Option<i64>) -> Result<Vec<LogLine>, AppError> {
    let mut rows = match tail {
        Some(n) => conn
            .prepare(
                "SELECT ts, stream, line FROM item_logs WHERE item_id = ?1
                 ORDER BY id DESC LIMIT ?2",
            )?
            .query_map(rusqlite::params![item_id, n.max(0)], |row| {
                Ok(LogLine {
                    ts: row.get(0)?,
                    stream: row.get(1)?,
                    line: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        None => conn
            .prepare("SELECT ts, stream, line FROM item_logs WHERE item_id = ?1 ORDER BY id ASC")?
            .query_map([item_id], |row| {
                Ok(LogLine {
                    ts: row.get(0)?,
                    stream: row.get(1)?,
                    line: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?,
    };
    if tail.is_some() {
        rows.reverse(); // was newest-first (DESC LIMIT); restore chronological order
    }
    Ok(rows)
}

/// Finds an existing item for `url` that isn't in a terminal-success/terminal-
/// cancel stage (T7 duplicate guard — ARCHITECTURE §7.2 `add_download`
/// `DUPLICATE_URL`). `completed`/`cancelled` don't block a re-add; every
/// other stage (including `error`, so a failed item can't be silently
/// re-queued as a duplicate without the user noticing) does.
pub fn find_active_item_by_url(conn: &Connection, url: &str) -> Result<Option<Item>, AppError> {
    Ok(conn
        .query_row(
            &format!("SELECT {ITEM_COLUMNS} FROM items WHERE url = ?1 AND stage NOT IN ('completed', 'cancelled') LIMIT 1"),
            [url],
            row_to_item,
        )
        .optional()?)
}

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

    fn new_test_item(url: &str, stage: &str) -> NewItem {
        NewItem {
            url: url.into(),
            format_expr: "bv*+ba/b".into(),
            output_dir: "/tmp/out".into(),
            output_template: "%(title)s.%(ext)s".into(),
            proxy: None,
            extra_args: None,
            preset_id: None,
            stage: stage.into(),
        }
    }

    #[test]
    fn insert_item_appends_queue_position() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);

        let first = insert_item(&conn, new_test_item("https://a", "downloading")).unwrap();
        let second = insert_item(&conn, new_test_item("https://b", "queued")).unwrap();

        assert_eq!(first.queue_position, 0);
        assert_eq!(second.queue_position, 1);
        assert_eq!(second.stage, "queued");
        assert_eq!(second.downloaded_bytes, 0);
        assert!(second.resume_capable);
    }

    #[test]
    fn get_item_returns_db_error_for_missing_id() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let err = get_item(&conn, 999).unwrap_err();
        assert!(matches!(err, AppError::DbError { .. }));
    }

    #[test]
    fn list_items_filters_by_stage() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        insert_item(&conn, new_test_item("https://a", "downloading")).unwrap();
        insert_item(&conn, new_test_item("https://b", "queued")).unwrap();

        let all = list_items(&conn, None).unwrap();
        assert_eq!(all.len(), 2);

        let queued = list_items(&conn, Some("queued")).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].url, "https://b");
    }

    #[test]
    fn count_active_items_counts_downloading_and_merging_only() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        insert_item(&conn, new_test_item("https://a", "downloading")).unwrap();
        insert_item(&conn, new_test_item("https://b", "merging")).unwrap();
        insert_item(&conn, new_test_item("https://c", "queued")).unwrap();

        assert_eq!(count_active_items(&conn).unwrap(), 2);
    }

    #[test]
    fn checkpoint_progress_updates_bytes_percent_and_stage() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let item = insert_item(&conn, new_test_item("https://a", "downloading")).unwrap();

        checkpoint_progress(&conn, item.id, "downloading", Some(1024), Some(4096), 25.0, Some(512), Some(6))
            .unwrap();

        let fetched = get_item(&conn, item.id).unwrap();
        assert_eq!(fetched.downloaded_bytes, 1024);
        assert_eq!(fetched.total_bytes, Some(4096));
        assert!((fetched.percent - 25.0).abs() < 1e-9);
        assert_eq!(fetched.speed_bps, Some(512));
        assert_eq!(fetched.eta_seconds, Some(6));
    }

    #[test]
    fn finish_item_sets_completed_stage_and_output_path() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let item = insert_item(&conn, new_test_item("https://a", "downloading")).unwrap();

        finish_item(&conn, item.id, "completed", Some("/tmp/out/video.mp4"), None).unwrap();

        let fetched = get_item(&conn, item.id).unwrap();
        assert_eq!(fetched.stage, "completed");
        assert_eq!(fetched.output_path.as_deref(), Some("/tmp/out/video.mp4"));
    }

    #[test]
    fn set_stage_updates_stage_only() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let item = insert_item(&conn, new_test_item("https://a", "queued")).unwrap();

        set_stage(&conn, item.id, "downloading").unwrap();

        let fetched = get_item(&conn, item.id).unwrap();
        assert_eq!(fetched.stage, "downloading");
    }

    #[test]
    fn find_dirty_items_returns_downloading_and_merging_ordered_by_position() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        insert_item(&conn, new_test_item("https://a", "downloading")).unwrap();
        insert_item(&conn, new_test_item("https://b", "queued")).unwrap();
        insert_item(&conn, new_test_item("https://c", "merging")).unwrap();

        let dirty = find_dirty_items(&conn).unwrap();
        assert_eq!(dirty.len(), 2);
        assert_eq!(dirty[0].url, "https://a");
        assert_eq!(dirty[1].url, "https://c");
    }

    #[test]
    fn pause_dirty_items_transitions_downloading_and_merging_only() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let a = insert_item(&conn, new_test_item("https://a", "downloading")).unwrap();
        let b = insert_item(&conn, new_test_item("https://b", "merging")).unwrap();
        let c = insert_item(&conn, new_test_item("https://c", "queued")).unwrap();

        pause_dirty_items(&conn).unwrap();

        assert_eq!(get_item(&conn, a.id).unwrap().stage, "paused");
        assert_eq!(get_item(&conn, b.id).unwrap().stage, "paused");
        assert_eq!(get_item(&conn, c.id).unwrap().stage, "queued");
    }

    #[test]
    fn delete_item_removes_row_and_cascades_logs() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let item = insert_item(&conn, new_test_item("https://a", "queued")).unwrap();
        insert_log(&conn, item.id, "stderr", "boom").unwrap();

        delete_item(&conn, item.id).unwrap();

        assert!(get_item(&conn, item.id).is_err());
        let log_count: i64 = conn
            .query_row("SELECT count(*) FROM item_logs WHERE item_id = ?1", [item.id], |r| r.get(0))
            .unwrap();
        assert_eq!(log_count, 0);
    }

    #[test]
    fn reorder_items_renumbers_by_given_order() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let a = insert_item(&conn, new_test_item("https://a", "queued")).unwrap();
        let b = insert_item(&conn, new_test_item("https://b", "queued")).unwrap();
        let c = insert_item(&conn, new_test_item("https://c", "queued")).unwrap();

        // Move c to the front.
        reorder_items(&conn, &[c.id, a.id, b.id]).unwrap();

        assert_eq!(get_item(&conn, c.id).unwrap().queue_position, 0);
        assert_eq!(get_item(&conn, a.id).unwrap().queue_position, 1);
        assert_eq!(get_item(&conn, b.id).unwrap().queue_position, 2);
    }

    #[test]
    fn insert_log_trims_to_500_lines_per_item() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let item = insert_item(&conn, new_test_item("https://a", "downloading")).unwrap();

        for i in 0..2000 {
            insert_log(&conn, item.id, "stderr", &format!("line {i}")).unwrap();
        }

        let count: i64 = conn
            .query_row("SELECT count(*) FROM item_logs WHERE item_id = ?1", [item.id], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 500);

        // The newest 500 lines survive, in chronological order.
        let lines = get_item_log(&conn, item.id, None).unwrap();
        assert_eq!(lines.len(), 500);
        assert_eq!(lines.first().unwrap().line, "line 1500");
        assert_eq!(lines.last().unwrap().line, "line 1999");
    }

    #[test]
    fn get_item_log_tail_limits_to_last_n_in_chronological_order() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let item = insert_item(&conn, new_test_item("https://a", "downloading")).unwrap();
        for i in 0..10 {
            insert_log(&conn, item.id, "stderr", &format!("line {i}")).unwrap();
        }

        let tailed = get_item_log(&conn, item.id, Some(3)).unwrap();
        assert_eq!(
            tailed.iter().map(|l| l.line.as_str()).collect::<Vec<_>>(),
            vec!["line 7", "line 8", "line 9"]
        );
    }

    #[test]
    fn find_active_item_by_url_ignores_completed_and_cancelled() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let item = insert_item(&conn, new_test_item("https://dup", "queued")).unwrap();

        assert_eq!(
            find_active_item_by_url(&conn, "https://dup").unwrap().map(|i| i.id),
            Some(item.id)
        );
        assert!(find_active_item_by_url(&conn, "https://not-there").unwrap().is_none());

        finish_item(&conn, item.id, "completed", Some("/tmp/x.mp4"), None).unwrap();
        assert!(find_active_item_by_url(&conn, "https://dup").unwrap().is_none());

        let item2 = insert_item(&conn, new_test_item("https://dup2", "downloading")).unwrap();
        finish_item(&conn, item2.id, "cancelled", None, None).unwrap();
        assert!(find_active_item_by_url(&conn, "https://dup2").unwrap().is_none());
    }

    #[test]
    fn finish_item_sets_error_stage_and_message() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_for_test(&conn);
        let item = insert_item(&conn, new_test_item("https://a", "downloading")).unwrap();

        finish_item(&conn, item.id, "error", None, Some("Requested format is not available")).unwrap();

        let fetched = get_item(&conn, item.id).unwrap();
        assert_eq!(fetched.stage, "error");
        assert_eq!(
            fetched.error_message.as_deref(),
            Some("Requested format is not available")
        );
    }
}
