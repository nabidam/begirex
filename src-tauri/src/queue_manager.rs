//! The authoritative in-memory-view-over-`items` scheduler (ARCHITECTURE §2,
//! §4). Owns the write path to `items`: decides the stage a new download
//! starts in ("spawn if fewer than N active"), picks the next `queued` item
//! when a slot frees, and reconciles crash-dirty items on launch. Must NOT
//! spawn processes directly (asks `engine_supervisor`) or parse yt-dlp
//! output (that's `progress_parser`/`engine_supervisor`'s job).
//!
//! NOT in scope here (T6): pause/cancel/remove/reorder, live N-resize.

use crate::engine_supervisor::{self, Emitter, SpawnParams};
use crate::error::AppError;
use crate::persistence::{self, Item, NewItem};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

type Db = Arc<Mutex<Connection>>;

/// Resolved binary paths needed to (re)spawn any item this module decides to
/// run — same values `ipc::add_download` already resolves from settings/
/// `binary_manager` (T1/T2); reconcile-on-launch resolves them the same way.
#[derive(Clone)]
pub struct BinaryPaths {
    pub ytdlp_path: String,
    pub ffmpeg_path: String,
}

/// Fields needed to add a new download; unlike `persistence::NewItem` this
/// carries no `stage` — that decision now belongs to queue_manager, not the
/// caller (ARCHITECTURE §7.2: route add_download's scheduling decision
/// through queue_manager instead of ipc.rs deciding inline).
pub struct AddDownloadParams {
    pub url: String,
    pub format_expr: String,
    pub output_dir: String,
    pub output_template: String,
    pub proxy: Option<String>,
    pub extra_args: Option<String>,
    pub preset_id: Option<i64>,
}

fn spawn_params_for(item: &Item, binaries: &BinaryPaths) -> SpawnParams {
    SpawnParams {
        ytdlp_path: binaries.ytdlp_path.clone(),
        ffmpeg_path: binaries.ffmpeg_path.clone(),
        url: item.url.clone(),
        format_expr: item.format_expr.clone(),
        output_dir: item.output_dir.clone(),
        output_template: item.output_template.clone(),
        proxy: item.proxy.clone(),
        extra_args: item.extra_args.clone(),
    }
}

/// Lowest-`queue_position` item currently `queued` (K2-AC2 / T3-AC3). Pure
/// read — `persistence::list_items` already orders by `queue_position`, so
/// the first row is the pick.
pub fn pick_next_queued(conn: &Connection) -> Result<Option<Item>, AppError> {
    Ok(persistence::list_items(conn, Some("queued"))?
        .into_iter()
        .next())
}

/// Spawns `item` (already flipped to `downloading` in the DB and emitted by
/// the caller) via `engine_supervisor::run_download`, awaits it to
/// completion within the same task, then tries to refill the slot it just
/// freed. Fire-and-forget, same contract as T2's `add_download` spawn: the
/// DB row + emitted events are the only observable signal, never a returned
/// `Err` from this fn.
fn spawn_and_refill(
    db: Db,
    item: Item,
    binaries: BinaryPaths,
    emitter: Arc<dyn Emitter>,
    n_slots: i64,
) {
    let params = spawn_params_for(&item, &binaries);
    let item_id = item.id;
    tauri::async_runtime::spawn(async move {
        engine_supervisor::run_download(Arc::clone(&db), item_id, params, Arc::clone(&emitter))
            .await;
        // The item that just ran left its active slot one way or another
        // (completed/error — cancelled/paused aren't reachable yet, T6).
        // Try to fill the slot it freed (ARCHITECTURE §4).
        try_fill_slot(db, binaries, emitter, n_slots);
    });
}

/// Slot-refill (ARCHITECTURE §4): if fewer than `n_slots` items are active,
/// spawn the lowest-`queue_position` `queued` item. Called after `add` and
/// after any item leaves the active set. No-op if no slot is free or no
/// item is queued.
pub fn try_fill_slot(db: Db, binaries: BinaryPaths, emitter: Arc<dyn Emitter>, n_slots: i64) {
    let next = {
        let conn = match db.lock() {
            Ok(c) => c,
            Err(_) => return, // ponytail: poisoned mutex means a prior panic already broke the app; nothing to schedule.
        };
        let active = match persistence::count_active_items(&conn) {
            Ok(n) => n,
            Err(_) => return,
        };
        if active >= n_slots {
            return;
        }
        match pick_next_queued(&conn) {
            Ok(Some(item)) => item,
            _ => return,
        }
    };

    let mut started = next;
    {
        let conn = match db.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        if persistence::set_stage(&conn, started.id, "downloading").is_err() {
            return;
        }
    }
    started.stage = "downloading".to_string();
    emitter.emit_stage_changed(started.id, "downloading", None);

    spawn_and_refill(db, started, binaries, emitter, n_slots);
}

/// Adds a new download and schedules it: inserts as `downloading` if fewer
/// than `n_slots` items are currently active, else `queued` (K2-AC2).
/// Replaces ipc.rs's former inline "spawn if <2 active" check — queue_manager
/// now owns this decision + the write path to `items` (ARCHITECTURE §2).
pub fn add_and_schedule(
    db: Db,
    emitter: Arc<dyn Emitter>,
    binaries: BinaryPaths,
    n_slots: i64,
    params: AddDownloadParams,
) -> Result<Item, AppError> {
    let item = {
        let conn = db.lock().expect("db mutex poisoned");
        let active = persistence::count_active_items(&conn)?;
        let stage = if active < n_slots { "downloading" } else { "queued" };
        persistence::insert_item(
            &conn,
            NewItem {
                url: params.url,
                format_expr: params.format_expr,
                output_dir: params.output_dir,
                output_template: params.output_template,
                proxy: params.proxy,
                extra_args: params.extra_args,
                preset_id: params.preset_id,
                stage: stage.to_string(),
            },
        )?
    };

    emitter.emit_stage_changed(item.id, &item.stage, None);

    if item.stage == "downloading" {
        spawn_and_refill(db, item.clone(), binaries, emitter, n_slots);
    }

    Ok(item)
}

/// Launch-time reconcile (ARCHITECTURE §8, read literally — see module doc
/// at top of file). Any item left `downloading`/`merging` when the app last
/// ran is a crash artifact (the process died with the app): first
/// bulk-transitioned to `paused` so `list_items` shows correct
/// last-checkpointed bytes immediately (§8: "the UI shows real progress
/// immediately"), *then* the scheduler resumes it by spawning yt-dlp with
/// `-c` (SpawnParams already always passes `-c`, see engine_supervisor.rs) —
/// flipping it back to `downloading` directly, bypassing the normal
/// "must be `queued`" precondition, since this is the one place ARCHITECTURE
/// explicitly calls for auto-resuming a non-user-paused item.
///
/// ponytail: reconcile is inherently launch-time-bounded and T6 (pause/
/// resume) doesn't exist yet, so this resumes up to `n_slots` dirty items in
/// `queue_position` order and leaves any excess `paused` — upgrade path is
/// T6's manual-resume command picking up the rest once it exists.
pub fn reconcile_and_resume(
    db: Db,
    emitter: Arc<dyn Emitter>,
    binaries: BinaryPaths,
    n_slots: i64,
) -> Result<(), AppError> {
    let dirty = {
        let conn = db.lock().expect("db mutex poisoned");
        let dirty = persistence::find_dirty_items(&conn)?;
        persistence::pause_dirty_items(&conn)?;
        dirty
    };

    for item in dirty.into_iter().take(n_slots.max(0) as usize) {
        {
            let conn = db.lock().expect("db mutex poisoned");
            persistence::set_stage(&conn, item.id, "downloading")?;
        }
        emitter.emit_stage_changed(item.id, "downloading", None);
        let mut resumed = item;
        resumed.stage = "downloading".to_string();
        spawn_and_refill(
            Arc::clone(&db),
            resumed,
            binaries.clone(),
            Arc::clone(&emitter),
            n_slots,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_test_item(conn: &Connection, url: &str, stage: &str) -> Item {
        persistence::insert_item(
            conn,
            NewItem {
                url: url.into(),
                format_expr: "bv*+ba/b".into(),
                output_dir: "/tmp/out".into(),
                output_template: "%(title)s.%(ext)s".into(),
                proxy: None,
                extra_args: None,
                preset_id: None,
                stage: stage.into(),
            },
        )
        .unwrap()
    }

    #[test]
    fn pick_next_queued_selects_lowest_queue_position_among_queued() {
        // T3 acceptance criterion 3.
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);

        new_test_item(&conn, "https://a", "downloading");
        let b = new_test_item(&conn, "https://b", "queued");
        let _c = new_test_item(&conn, "https://c", "downloading");
        let d = new_test_item(&conn, "https://d", "queued");

        // b (position 1) and d (position 3) are queued; b is lower.
        let picked = pick_next_queued(&conn).unwrap().expect("expected a pick");
        assert_eq!(picked.id, b.id);
        assert_eq!(picked.queue_position, b.queue_position);
        assert!(b.queue_position < d.queue_position);
    }

    #[test]
    fn pick_next_queued_returns_none_when_nothing_queued() {
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        new_test_item(&conn, "https://a", "downloading");

        assert!(pick_next_queued(&conn).unwrap().is_none());
    }
}
