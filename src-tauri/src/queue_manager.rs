//! The authoritative in-memory-view-over-`items` scheduler (ARCHITECTURE §2,
//! §4). Owns the write path to `items`: decides the stage a new download
//! starts in ("spawn if fewer than N active"), picks the next `queued` item
//! when a slot frees, reconciles crash-dirty items on launch, and (T6) the
//! full lifecycle — pause/resume/cancel/remove/retry/reorder/set_concurrency/
//! bulk. Must NOT spawn processes directly (asks `engine_supervisor`) or
//! parse yt-dlp output (that's `progress_parser`/`engine_supervisor`'s job).

use crate::engine_supervisor::{self, ActiveRegistry, Emitter, SpawnParams};
use crate::error::AppError;
use crate::persistence::{self, Item, NewItem};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

type Db = Arc<Mutex<Connection>>;

/// One of the four `bulk_action` verbs (ARCHITECTURE §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkVerb {
    Pause,
    Resume,
    Cancel,
    Remove,
}

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
#[derive(Clone)]
pub struct AddDownloadParams {
    pub url: String,
    pub format_expr: String,
    pub output_dir: String,
    pub output_template: String,
    pub proxy: Option<String>,
    pub extra_args: Option<String>,
    pub preset_id: Option<i64>,
    // T19: set by `add_download_expanding` for playlist-derived rows; both
    // `None` for a lone video, same as every pre-T19 caller of
    // `add_and_schedule` directly.
    pub playlist_id: Option<String>,
    pub title: Option<String>,
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
    registry: ActiveRegistry,
) {
    let params = spawn_params_for(&item, &binaries);
    let item_id = item.id;
    tauri::async_runtime::spawn(async move {
        engine_supervisor::run_download(
            Arc::clone(&db),
            item_id,
            params,
            Arc::clone(&emitter),
            Arc::clone(&registry),
        )
        .await;
        // The item that just ran left its active slot one way or another
        // (completed/error — a pause/cancel already removed it from the
        // active set itself). Try to fill the slot it freed (ARCHITECTURE §4).
        try_fill_slot(db, binaries, emitter, n_slots, registry);
    });
}

/// Slot-refill (ARCHITECTURE §4): while fewer than `n_slots` items are
/// active, spawn the lowest-`queue_position` `queued` item. Called after
/// `add`, after any item leaves the active set, on manual resume, and on a
/// `set_concurrency` increase (where more than one slot can open up at
/// once — hence the loop, not a single pick). No-op once no slot is free or
/// no item is queued.
pub fn try_fill_slot(db: Db, binaries: BinaryPaths, emitter: Arc<dyn Emitter>, n_slots: i64, registry: ActiveRegistry) {
    loop {
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

        spawn_and_refill(
            Arc::clone(&db),
            started,
            binaries.clone(),
            Arc::clone(&emitter),
            n_slots,
            Arc::clone(&registry),
        );
    }
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
    registry: ActiveRegistry,
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
                playlist_id: params.playlist_id,
                title: params.title,
            },
        )?
    };

    emitter.emit_item_added(&item);
    emitter.emit_stage_changed(item.id, &item.stage, None);

    if item.stage == "downloading" {
        spawn_and_refill(db, item.clone(), binaries, emitter, n_slots, registry);
    }

    Ok(item)
}

/// Expands `params.url` via `engine_supervisor::expand_playlist` and adds one
/// row per resulting entry (T19, K2-AC3/V2-AC2): a lone video still comes
/// back as a single `PlaylistEntry` with no `playlist_id`, so this is the one
/// path `add_download` needs regardless of whether the submitted URL is a
/// playlist. Each entry is scheduled independently through
/// `add_and_schedule` — same "spawn if a slot's free, else queue" rule a
/// plain single add gets — so cancelling/erroring one never touches the
/// others (AC1/AC2/AC3).
pub async fn add_download_expanding(
    db: Db,
    emitter: Arc<dyn Emitter>,
    binaries: BinaryPaths,
    n_slots: i64,
    registry: ActiveRegistry,
    params: AddDownloadParams,
) -> Result<Vec<Item>, AppError> {
    let expansion =
        engine_supervisor::expand_playlist(&binaries.ytdlp_path, &params.url, params.proxy.as_deref()).await?;

    let mut items = Vec::with_capacity(expansion.entries.len());
    for entry in expansion.entries {
        let entry_params = AddDownloadParams {
            url: entry.url,
            title: entry.title,
            playlist_id: expansion.playlist_id.clone(),
            ..params.clone()
        };
        let item = add_and_schedule(
            Arc::clone(&db),
            Arc::clone(&emitter),
            binaries.clone(),
            n_slots,
            entry_params,
            Arc::clone(&registry),
        )?;
        items.push(item);
    }
    Ok(items)
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
    registry: ActiveRegistry,
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
            Arc::clone(&registry),
        );
    }
    Ok(())
}

// --- T6: full lifecycle -----------------------------------------------------

/// Pauses `item_id` (K2-AC6): if it's currently `downloading`/`merging`,
/// kills its child (partial bytes + `resume_capable` stay on disk per
/// ARCHITECTURE §4) before flipping the DB row; otherwise just flips the
/// stage (e.g. pausing a `queued` item keeps it out of the scheduler's pick).
pub async fn pause_item(db: Db, emitter: Arc<dyn Emitter>, registry: ActiveRegistry, item_id: i64) -> Result<Item, AppError> {
    let item = {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::get_item(&conn, item_id)?
    };
    if matches!(item.stage.as_str(), "downloading" | "merging") {
        engine_supervisor::pause(&registry, item_id).await?;
    }
    {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::set_stage(&conn, item_id, "paused")?;
    }
    emitter.emit_stage_changed(item_id, "paused", None);
    let conn = db.lock().expect("db mutex poisoned");
    persistence::get_item(&conn, item_id)
}

/// Resumes a `paused` item (K2-AC6): re-spawns with `-c` (always present in
/// `SpawnParams`) immediately if a slot is free, else leaves it `queued` for
/// the scheduler to pick up like any other queued item.
pub fn resume_item(
    db: Db,
    emitter: Arc<dyn Emitter>,
    binaries: BinaryPaths,
    n_slots: i64,
    registry: ActiveRegistry,
    item_id: i64,
) -> Result<Item, AppError> {
    let item = {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::get_item(&conn, item_id)?
    };
    let active = {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::count_active_items(&conn)?
    };
    let next_stage = if active < n_slots { "downloading" } else { "queued" };
    {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::set_stage(&conn, item_id, next_stage)?;
    }
    emitter.emit_stage_changed(item_id, next_stage, None);
    if next_stage == "downloading" {
        let mut resumed = item;
        resumed.stage = "downloading".to_string();
        spawn_and_refill(Arc::clone(&db), resumed, binaries, emitter, n_slots, registry);
    }
    let conn = db.lock().expect("db mutex poisoned");
    persistence::get_item(&conn, item_id)
}

/// Cancels `item_id` (K2-AC7): kills its child if active (deleting whatever
/// partial file it was writing), marks the row `cancelled`, then tries to
/// fill the slot it just freed.
///
/// ponytail: partial-file cleanup only knows about paths captured from a
/// *running* child's "Destination:" lines (engine_supervisor's
/// `partial_paths`) — cancelling an already-`paused` item (no running
/// child) can't locate its file this way. Upgrade path: persist the last
/// known destination path on the `items` row itself if that case matters.
pub async fn cancel_item(
    db: Db,
    emitter: Arc<dyn Emitter>,
    binaries: BinaryPaths,
    n_slots: i64,
    registry: ActiveRegistry,
    item_id: i64,
) -> Result<Item, AppError> {
    let item = {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::get_item(&conn, item_id)?
    };
    if matches!(item.stage.as_str(), "downloading" | "merging") {
        engine_supervisor::cancel(&registry, item_id).await?;
    }
    {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::finish_item(&conn, item_id, "cancelled", None, None)?;
    }
    emitter.emit_stage_changed(item_id, "cancelled", None);
    try_fill_slot(Arc::clone(&db), binaries, emitter, n_slots, registry);
    let conn = db.lock().expect("db mutex poisoned");
    persistence::get_item(&conn, item_id)
}

/// Removes `item_id` entirely (K2-AC8): stops it first if active (same
/// cleanup as cancel), deletes the row, emits `item_removed`, then tries to
/// fill the slot it freed. Returns the item's last known state (the row is
/// gone by the time this returns, so there's nothing left to re-fetch).
pub async fn remove_item(
    db: Db,
    emitter: Arc<dyn Emitter>,
    binaries: BinaryPaths,
    n_slots: i64,
    registry: ActiveRegistry,
    item_id: i64,
) -> Result<Item, AppError> {
    let item = {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::get_item(&conn, item_id)?
    };
    if matches!(item.stage.as_str(), "downloading" | "merging") {
        engine_supervisor::cancel(&registry, item_id).await?;
    }
    {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::delete_item(&conn, item_id)?;
    }
    emitter.emit_item_removed(item_id);
    try_fill_slot(db, binaries, emitter, n_slots, registry);
    Ok(item)
}

/// Retries an `error`ed item (T7 builds resume-correct semantics on top of
/// this — T6 just wires the transition + reschedule): flips it back to
/// `queued` and lets the scheduler pick it up like any other queued item.
pub fn retry_item(
    db: Db,
    emitter: Arc<dyn Emitter>,
    binaries: BinaryPaths,
    n_slots: i64,
    registry: ActiveRegistry,
    item_id: i64,
) -> Result<Item, AppError> {
    {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::set_stage(&conn, item_id, "queued")?;
    }
    emitter.emit_stage_changed(item_id, "queued", None);
    try_fill_slot(Arc::clone(&db), binaries, emitter, n_slots, registry);
    let conn = db.lock().expect("db mutex poisoned");
    persistence::get_item(&conn, item_id)
}

/// Moves `item_id` to `new_position` among all items (K2-AC9), renumbering
/// everyone else to stay contiguous. `new_position` is clamped into range.
pub fn reorder_item(db: Db, item_id: i64, new_position: i64) -> Result<(), AppError> {
    let conn = db.lock().expect("db mutex poisoned");
    let mut ordered = persistence::list_items(&conn, None)?;
    let current_index = ordered
        .iter()
        .position(|i| i.id == item_id)
        .ok_or_else(|| AppError::DbError {
            message: format!("item {item_id} not found"),
        })?;
    let item = ordered.remove(current_index);
    let target = (new_position.max(0) as usize).min(ordered.len());
    ordered.insert(target, item);
    let ids: Vec<i64> = ordered.iter().map(|i| i.id).collect();
    persistence::reorder_items(&conn, &ids)
}

/// Sets the concurrency semaphore size (K2-AC-adjacent — ARCHITECTURE §4 "N
/// change"): an increase immediately tries to fill any newly-available
/// slots; a decrease never kills an in-flight item, it just lowers the
/// ceiling new starts respect. `n >= 1` is validated at the ipc trust
/// boundary, not here.
pub fn set_concurrency(
    db: Db,
    emitter: Arc<dyn Emitter>,
    binaries: BinaryPaths,
    registry: ActiveRegistry,
    n: i64,
) -> Result<i64, AppError> {
    {
        let conn = db.lock().expect("db mutex poisoned");
        persistence::set_setting(&conn, "default_concurrency", &n.to_string())?;
    }
    try_fill_slot(db, binaries, emitter, n, registry);
    Ok(n)
}

/// Applies one bulk verb to each id in `ids`, best-effort (ARCHITECTURE
/// §7.2: "partial: per-id result list") — an id that errors (e.g. already
/// removed) is skipped rather than aborting the whole batch.
pub async fn bulk_action(
    db: Db,
    emitter: Arc<dyn Emitter>,
    binaries: BinaryPaths,
    n_slots: i64,
    registry: ActiveRegistry,
    ids: Vec<i64>,
    verb: BulkVerb,
) -> Vec<Item> {
    let mut updated = Vec::new();
    for id in ids {
        let result = match verb {
            BulkVerb::Pause => pause_item(Arc::clone(&db), Arc::clone(&emitter), Arc::clone(&registry), id).await,
            BulkVerb::Resume => resume_item(
                Arc::clone(&db),
                Arc::clone(&emitter),
                binaries.clone(),
                n_slots,
                Arc::clone(&registry),
                id,
            ),
            BulkVerb::Cancel => {
                cancel_item(
                    Arc::clone(&db),
                    Arc::clone(&emitter),
                    binaries.clone(),
                    n_slots,
                    Arc::clone(&registry),
                    id,
                )
                .await
            }
            BulkVerb::Remove => {
                remove_item(
                    Arc::clone(&db),
                    Arc::clone(&emitter),
                    binaries.clone(),
                    n_slots,
                    Arc::clone(&registry),
                    id,
                )
                .await
            }
        };
        if let Ok(item) = result {
            updated.push(item);
        }
    }
    updated
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
                playlist_id: None,
                title: None,
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

    // --- T19: playlist expansion ---------------------------------------------

    #[derive(Default)]
    struct NoopEmitter;
    impl Emitter for NoopEmitter {
        fn emit_progress(&self, _item_id: i64, _tick: &crate::progress_parser::ProgressTick) {}
        fn emit_stage_changed(&self, _item_id: i64, _stage: &str, _error_message: Option<&str>) {}
        fn emit_item_added(&self, _item: &Item) {}
        fn emit_item_removed(&self, _item_id: i64) {}
        fn emit_log_line(&self, _item_id: i64, _stream: &str, _line: &str) {}
        fn emit_binary_health(&self, _which: &str, _found: bool, _path: Option<&str>) {}
    }

    /// A fake `yt-dlp` that answers `-J --flat-playlist` with a canned
    /// two-entry playlist, standing in for the real binary the same way
    /// `engine_supervisor`'s own tests fake it out for `run_download`.
    fn write_fake_flat_playlist_ytdlp() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("begirex-fake-ytdlp-{}", std::process::id()));
        std::fs::write(
            &path,
            "#!/bin/sh\necho '{\"id\":\"PLTEST\",\"entries\":[\
             {\"webpage_url\":\"https://example.invalid/watch?v=a\",\"title\":\"A\"},\
             {\"webpage_url\":\"https://example.invalid/watch?v=b\",\"title\":\"B\"}]}'\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[tokio::test]
    async fn add_download_expanding_creates_one_independent_row_per_playlist_entry() {
        // T19-AC1: a playlist of M entries yields M rows sharing a
        // `playlist_id`, each its own row in the queue.
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        let db: Db = Arc::new(Mutex::new(conn));

        let script = write_fake_flat_playlist_ytdlp();
        let emitter: Arc<dyn Emitter> = Arc::new(NoopEmitter::default());
        let registry: ActiveRegistry = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let binaries = BinaryPaths {
            ytdlp_path: script.to_string_lossy().to_string(),
            ffmpeg_path: "ffmpeg".into(),
        };

        let items = add_download_expanding(
            Arc::clone(&db),
            emitter,
            binaries,
            // n_slots: 0 keeps every entry `queued` (never `downloading`), so
            // this test only exercises expansion + row creation, not a real
            // download spawn against the fake binary.
            0,
            registry,
            AddDownloadParams {
                url: "https://example.invalid/playlist?list=PLTEST".into(),
                format_expr: "bv*+ba/b".into(),
                output_dir: "/tmp/out".into(),
                output_template: "%(title)s.%(ext)s".into(),
                proxy: None,
                extra_args: None,
                preset_id: None,
                playlist_id: None,
                title: None,
            },
        )
        .await
        .unwrap();

        std::fs::remove_file(&script).ok();

        assert_eq!(items.len(), 2);
        assert!(items[0].playlist_id.is_some());
        assert_eq!(items[0].playlist_id, items[1].playlist_id);
        assert_ne!(items[0].id, items[1].id);
        assert_ne!(items[0].url, items[1].url);
        assert_eq!(items[0].title, Some("A".to_string()));
        assert_eq!(items[1].title, Some("B".to_string()));
        assert_eq!(items[0].stage, "queued");
        assert_eq!(items[1].stage, "queued");
    }

    #[tokio::test]
    async fn add_download_expanding_leaves_a_lone_video_without_a_playlist_id() {
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        let db: Db = Arc::new(Mutex::new(conn));

        // `--flat-playlist` on a lone video returns no `entries` key at all —
        // reuse the fake binary path but with a title-only script.
        let path = std::env::temp_dir().join(format!("begirex-fake-ytdlp-solo-{}", std::process::id()));
        std::fs::write(&path, "#!/bin/sh\necho '{\"title\":\"Solo video\"}'\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let emitter: Arc<dyn Emitter> = Arc::new(NoopEmitter::default());
        let registry: ActiveRegistry = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let binaries = BinaryPaths {
            ytdlp_path: path.to_string_lossy().to_string(),
            ffmpeg_path: "ffmpeg".into(),
        };

        let items = add_download_expanding(
            db,
            emitter,
            binaries,
            0,
            registry,
            AddDownloadParams {
                url: "https://example.invalid/watch?v=solo".into(),
                format_expr: "bv*+ba/b".into(),
                output_dir: "/tmp/out".into(),
                output_template: "%(title)s.%(ext)s".into(),
                proxy: None,
                extra_args: None,
                preset_id: None,
                playlist_id: None,
                title: None,
            },
        )
        .await
        .unwrap();

        std::fs::remove_file(&path).ok();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].playlist_id, None);
        assert_eq!(items[0].url, "https://example.invalid/watch?v=solo");
    }
}
