//! IPC command handlers (ARCHITECTURE §2, §7). Translates `invoke` calls into
//! module calls; validates inputs at the trust boundary (§8); holds no state
//! of its own — state lives in `AppState` (lib.rs) and is only borrowed here.

use crate::binary_manager::{self, BinaryStatus, Which};
use crate::engine_supervisor::Emitter;
use crate::error::AppError;
use crate::persistence::{self, Item};
use crate::progress_parser::{self, ProgressTick};
use crate::queue_manager::{self, AddDownloadParams, BinaryPaths, BulkVerb};
use crate::settings_service::{self, Settings, SettingsUpdate};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter as _, Manager, State};

#[derive(Debug, Serialize)]
pub struct BinaryStatuses {
    pub ytdlp: BinaryStatus,
    pub ffmpeg: BinaryStatus,
}

#[tauri::command]
pub fn detect_binaries(state: State<AppState>) -> Result<BinaryStatuses, AppError> {
    let conn = state.db.lock().expect("db mutex poisoned");
    Ok(BinaryStatuses {
        ytdlp: binary_manager::detect(&conn, &Which::Ytdlp)?,
        ffmpeg: binary_manager::detect(&conn, &Which::Ffmpeg)?,
    })
}

#[tauri::command]
pub fn recheck_binaries(state: State<AppState>) -> Result<BinaryStatuses, AppError> {
    // ponytail: recheck is identical to detect for T1 (no cached in-memory
    // health state exists yet) — T16 adds mid-session health tracking that
    // will make this a distinct code path.
    detect_binaries(state)
}

#[derive(Debug, Deserialize)]
pub struct SetBinaryPathRequest {
    pub which: String,
    pub path: String,
}

#[tauri::command]
pub fn set_binary_path(
    state: State<AppState>,
    request: SetBinaryPathRequest,
) -> Result<BinaryStatus, AppError> {
    if request.path.trim().is_empty() {
        return Err(AppError::Validation {
            message: "path must not be empty".into(),
        });
    }
    let which = Which::parse(&request.which).ok_or_else(|| AppError::Validation {
        message: format!("unknown binary '{}', expected 'ytdlp' or 'ffmpeg'", request.which),
    })?;

    let conn = state.db.lock().expect("db mutex poisoned");
    binary_manager::set_path(&conn, &which, &request.path)
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<Settings, AppError> {
    let conn = state.db.lock().expect("db mutex poisoned");
    settings_service::get_settings(&conn)
}

#[tauri::command]
pub fn update_settings(
    state: State<AppState>,
    update: SettingsUpdate,
) -> Result<Settings, AppError> {
    if let Some(n) = update.default_concurrency {
        if n < 1 {
            return Err(AppError::Validation {
                message: "default_concurrency must be >= 1".into(),
            });
        }
    }
    if let Some(dir) = &update.default_output_dir {
        if dir.trim().is_empty() {
            return Err(AppError::Validation {
                message: "default_output_dir must not be empty".into(),
            });
        }
    }
    if let Some(template) = &update.default_output_template {
        if template.trim().is_empty() {
            return Err(AppError::Validation {
                message: "default_output_template must not be empty".into(),
            });
        }
    }

    let conn = state.db.lock().expect("db mutex poisoned");
    settings_service::update_settings(&conn, update)
}

// --- T2: engine spawn + progress pipeline + add_download -------------------

/// Emits via a real `tauri::AppHandle` (ARCHITECTURE §7.3 event shapes).
/// Production-only wiring around `engine_supervisor::Emitter` — the trait
/// itself stays decoupled from Tauri so engine_supervisor is unit-testable
/// without a running app (see engine_supervisor.rs's doc comment).
pub(crate) struct TauriEmitter {
    app: AppHandle,
}

impl TauriEmitter {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

#[derive(Serialize, Clone)]
struct ProgressPayload<'a> {
    id: i64,
    percent: f64,
    downloaded_bytes: Option<i64>,
    total_bytes: Option<i64>,
    speed_bps: Option<i64>,
    eta_seconds: Option<i64>,
    stage: &'a str,
}

#[derive(Serialize, Clone)]
struct StageChangedPayload<'a> {
    id: i64,
    stage: &'a str,
    error_message: Option<&'a str>,
}

impl Emitter for TauriEmitter {
    fn emit_progress(&self, item_id: i64, tick: &ProgressTick) {
        let stage = match tick.stage {
            progress_parser::Stage::Downloading => "downloading",
            progress_parser::Stage::Merging => "merging",
        };
        let _ = self.app.emit(
            "progress",
            ProgressPayload {
                id: item_id,
                percent: tick.percent,
                downloaded_bytes: tick.downloaded_bytes,
                total_bytes: tick.total_bytes,
                speed_bps: tick.speed_bps,
                eta_seconds: tick.eta_seconds,
                stage,
            },
        );
    }

    fn emit_stage_changed(&self, item_id: i64, stage: &str, error_message: Option<&str>) {
        let _ = self.app.emit(
            "stage_changed",
            StageChangedPayload {
                id: item_id,
                stage,
                error_message,
            },
        );
    }

    fn emit_item_added(&self, item: &Item) {
        let _ = self.app.emit("item_added", item);
    }

    fn emit_item_removed(&self, item_id: i64) {
        let _ = self.app.emit("item_removed", ItemRemovedPayload { id: item_id });
    }

    // Only tails while a detail drawer is open for this item (ARCHITECTURE
    // §7.3) — `watch_log` toggles membership in `AppState.log_watchers`;
    // gating lives here (not engine_supervisor) so that module stays a plain
    // unconditional emitter, same as its other `emit_*` methods.
    fn emit_log_line(&self, item_id: i64, stream: &str, line: &str) {
        let watched = self
            .app
            .state::<AppState>()
            .log_watchers
            .lock()
            .unwrap()
            .contains(&item_id);
        if watched {
            let _ = self.app.emit(
                "log_line",
                LogLinePayload {
                    id: item_id,
                    stream,
                    line,
                },
            );
        }
    }
}

#[derive(Serialize, Clone)]
struct ItemRemovedPayload {
    id: i64,
}

#[derive(Serialize, Clone)]
struct LogLinePayload<'a> {
    id: i64,
    stream: &'a str,
    line: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct AddDownloadRequest {
    pub url: String,
    pub format_expr: String,
    pub output_dir: Option<String>,
    pub output_template: Option<String>,
    pub proxy: Option<String>,
    pub extra_args: Option<String>,
    pub preset_id: Option<i64>,
    pub force: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AddDownloadResponse {
    pub items: Vec<Item>,
}

/// T7 duplicate guard (ARCHITECTURE §7.2): a URL already sitting in a
/// non-`completed`/non-`cancelled` stage is rejected unless `force` is set.
/// A plain fn (not a `#[tauri::command]`) so it's unit-testable against an
/// in-memory `Connection` without needing a running Tauri app.
fn check_duplicate(conn: &rusqlite::Connection, url: &str, force: bool) -> Result<(), AppError> {
    if force {
        return Ok(());
    }
    if persistence::find_active_item_by_url(conn, url)?.is_some() {
        return Err(AppError::DuplicateUrl {
            message: format!("'{url}' is already in the queue"),
        });
    }
    Ok(())
}

/// Adds one download and routes the scheduling decision ("spawn if fewer
/// than N active, else queue") through `queue_manager` (ARCHITECTURE §7.2,
/// §4/T3) — ipc.rs only resolves inputs (settings/binary paths) and stays
/// thin. T2 only ever produces exactly 1 item — playlist expansion (N items)
/// is T19's job.
#[tauri::command]
pub fn add_download(
    app: AppHandle,
    state: State<AppState>,
    request: AddDownloadRequest,
) -> Result<AddDownloadResponse, AppError> {
    if request.url.trim().is_empty() {
        return Err(AppError::Validation {
            message: "url must not be empty".into(),
        });
    }
    if request.format_expr.trim().is_empty() {
        return Err(AppError::Validation {
            message: "format_expr must not be empty".into(),
        });
    }

    let conn = state.db.lock().expect("db mutex poisoned");
    check_duplicate(&conn, &request.url, request.force.unwrap_or(false))?;
    let settings = settings_service::get_settings(&conn)?;

    let ytdlp_path = binary_manager::detect(&conn, &Which::Ytdlp)?
        .path
        .ok_or_else(|| AppError::BinaryNotFound {
            message: "yt-dlp not found; set its path in Settings".into(),
        })?;
    let ffmpeg_path = binary_manager::detect(&conn, &Which::Ffmpeg)?
        .path
        .ok_or_else(|| AppError::BinaryNotFound {
            message: "ffmpeg not found; set its path in Settings".into(),
        })?;

    let output_dir = request
        .output_dir
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| settings.default_output_dir.clone());
    let output_template = request
        .output_template
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| settings.default_output_template.clone());
    let proxy = request
        .proxy
        .filter(|s| !s.trim().is_empty())
        .or_else(|| settings.global_proxy.clone());
    let n_slots = settings.default_concurrency;

    drop(conn);

    let db = Arc::clone(&state.db);
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app.clone()));
    let binaries = BinaryPaths {
        ytdlp_path,
        ffmpeg_path,
    };

    let item = queue_manager::add_and_schedule(
        db,
        emitter,
        binaries,
        n_slots,
        AddDownloadParams {
            url: request.url,
            format_expr: request.format_expr,
            output_dir,
            output_template,
            proxy,
            extra_args: request.extra_args,
            preset_id: request.preset_id,
        },
        Arc::clone(&state.registry),
    )?;

    Ok(AddDownloadResponse { items: vec![item] })
}

/// Resolves the binary paths + current concurrency the T6 lifecycle
/// commands need to (re)spawn a download — the same lookup `add_download`
/// already does inline, factored out since five more commands need it.
fn resolve_runtime(conn: &rusqlite::Connection) -> Result<(BinaryPaths, i64), AppError> {
    let settings = settings_service::get_settings(conn)?;
    let ytdlp_path = binary_manager::detect(conn, &Which::Ytdlp)?
        .path
        .ok_or_else(|| AppError::BinaryNotFound {
            message: "yt-dlp not found; set its path in Settings".into(),
        })?;
    let ffmpeg_path = binary_manager::detect(conn, &Which::Ffmpeg)?
        .path
        .ok_or_else(|| AppError::BinaryNotFound {
            message: "ffmpeg not found; set its path in Settings".into(),
        })?;
    Ok((
        BinaryPaths {
            ytdlp_path,
            ffmpeg_path,
        },
        settings.default_concurrency,
    ))
}

#[derive(Debug, Serialize)]
pub struct OkResponse {
    pub ok: bool,
}

#[tauri::command]
pub async fn pause_item(app: AppHandle, state: State<'_, AppState>, request: GetItemRequest) -> Result<Item, AppError> {
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app));
    queue_manager::pause_item(Arc::clone(&state.db), emitter, Arc::clone(&state.registry), request.id).await
}

#[tauri::command]
pub fn resume_item(app: AppHandle, state: State<AppState>, request: GetItemRequest) -> Result<Item, AppError> {
    let (binaries, n_slots) = {
        let conn = state.db.lock().expect("db mutex poisoned");
        resolve_runtime(&conn)?
    };
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app));
    queue_manager::resume_item(
        Arc::clone(&state.db),
        emitter,
        binaries,
        n_slots,
        Arc::clone(&state.registry),
        request.id,
    )
}

#[tauri::command]
pub async fn cancel_item(app: AppHandle, state: State<'_, AppState>, request: GetItemRequest) -> Result<Item, AppError> {
    let (binaries, n_slots) = {
        let conn = state.db.lock().expect("db mutex poisoned");
        resolve_runtime(&conn)?
    };
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app));
    queue_manager::cancel_item(
        Arc::clone(&state.db),
        emitter,
        binaries,
        n_slots,
        Arc::clone(&state.registry),
        request.id,
    )
    .await
}

#[tauri::command]
pub async fn remove_item(app: AppHandle, state: State<'_, AppState>, request: GetItemRequest) -> Result<OkResponse, AppError> {
    let (binaries, n_slots) = {
        let conn = state.db.lock().expect("db mutex poisoned");
        resolve_runtime(&conn)?
    };
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app));
    queue_manager::remove_item(
        Arc::clone(&state.db),
        emitter,
        binaries,
        n_slots,
        Arc::clone(&state.registry),
        request.id,
    )
    .await?;
    Ok(OkResponse { ok: true })
}

#[tauri::command]
pub fn retry_item(app: AppHandle, state: State<AppState>, request: GetItemRequest) -> Result<Item, AppError> {
    let (binaries, n_slots) = {
        let conn = state.db.lock().expect("db mutex poisoned");
        resolve_runtime(&conn)?
    };
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app));
    queue_manager::retry_item(
        Arc::clone(&state.db),
        emitter,
        binaries,
        n_slots,
        Arc::clone(&state.registry),
        request.id,
    )
}

#[derive(Debug, Deserialize)]
pub struct ReorderItemRequest {
    pub id: i64,
    pub new_position: i64,
}

#[tauri::command]
pub fn reorder_item(state: State<AppState>, request: ReorderItemRequest) -> Result<OkResponse, AppError> {
    queue_manager::reorder_item(Arc::clone(&state.db), request.id, request.new_position)?;
    Ok(OkResponse { ok: true })
}

#[derive(Debug, Deserialize)]
pub struct BulkActionRequest {
    pub ids: Vec<i64>,
    pub action: String,
}

#[derive(Debug, Serialize)]
pub struct BulkActionResponse {
    pub updated: Vec<Item>,
}

#[tauri::command]
pub async fn bulk_action(app: AppHandle, state: State<'_, AppState>, request: BulkActionRequest) -> Result<BulkActionResponse, AppError> {
    let verb = match request.action.as_str() {
        "pause" => BulkVerb::Pause,
        "resume" => BulkVerb::Resume,
        "cancel" => BulkVerb::Cancel,
        "remove" => BulkVerb::Remove,
        other => {
            return Err(AppError::Validation {
                message: format!("unknown bulk action '{other}'"),
            })
        }
    };
    let (binaries, n_slots) = {
        let conn = state.db.lock().expect("db mutex poisoned");
        resolve_runtime(&conn)?
    };
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app));
    let updated = queue_manager::bulk_action(
        Arc::clone(&state.db),
        emitter,
        binaries,
        n_slots,
        Arc::clone(&state.registry),
        request.ids,
        verb,
    )
    .await;
    Ok(BulkActionResponse { updated })
}

#[derive(Debug, Deserialize)]
pub struct SetConcurrencyRequest {
    pub n: i64,
}

#[derive(Debug, Serialize)]
pub struct SetConcurrencyResponse {
    pub n: i64,
}

#[tauri::command]
pub fn set_concurrency(app: AppHandle, state: State<AppState>, request: SetConcurrencyRequest) -> Result<SetConcurrencyResponse, AppError> {
    if request.n < 1 {
        return Err(AppError::Validation {
            message: "n must be >= 1".into(),
        });
    }
    let binaries = {
        let conn = state.db.lock().expect("db mutex poisoned");
        resolve_runtime(&conn)?.0
    };
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app));
    let n = queue_manager::set_concurrency(
        Arc::clone(&state.db),
        emitter,
        binaries,
        Arc::clone(&state.registry),
        request.n,
    )?;
    Ok(SetConcurrencyResponse { n })
}

#[derive(Debug, Deserialize, Default)]
pub struct ListItemsRequest {
    pub filter: Option<String>,
}

#[tauri::command]
pub fn list_items(state: State<AppState>, request: ListItemsRequest) -> Result<Vec<Item>, AppError> {
    let conn = state.db.lock().expect("db mutex poisoned");
    persistence::list_items(&conn, request.filter.as_deref())
}

#[derive(Debug, Deserialize)]
pub struct GetItemRequest {
    pub id: i64,
}

#[tauri::command]
pub fn get_item(state: State<AppState>, request: GetItemRequest) -> Result<Item, AppError> {
    let conn = state.db.lock().expect("db mutex poisoned");
    persistence::get_item(&conn, request.id)
}

#[derive(Debug, Deserialize)]
pub struct GetItemLogRequest {
    pub id: i64,
    pub tail: Option<i64>,
}

#[tauri::command]
pub fn get_item_log(
    state: State<AppState>,
    request: GetItemLogRequest,
) -> Result<Vec<persistence::LogLine>, AppError> {
    let conn = state.db.lock().expect("db mutex poisoned");
    persistence::get_item_log(&conn, request.id, request.tail)
}

#[derive(Debug, Deserialize)]
pub struct WatchLogRequest {
    pub id: i64,
    pub on: bool,
}

/// Toggles whether `log_line` events are emitted for `id` (ARCHITECTURE
/// §7.3: only while a detail drawer is open for that item) — S5 calls this
/// on open/close of its log disclosure.
#[tauri::command]
pub fn watch_log(state: State<AppState>, request: WatchLogRequest) -> Result<OkResponse, AppError> {
    let mut watchers = state.log_watchers.lock().expect("log watchers mutex poisoned");
    if request.on {
        watchers.insert(request.id);
    } else {
        watchers.remove(&request.id);
    }
    Ok(OkResponse { ok: true })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{self, NewItem};
    use rusqlite::Connection;

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
    fn check_duplicate_rejects_active_url_unless_forced() {
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        new_test_item(&conn, "https://dup", "queued");

        let err = check_duplicate(&conn, "https://dup", false).unwrap_err();
        assert!(matches!(err, AppError::DuplicateUrl { .. }));

        // force:true bypasses it.
        check_duplicate(&conn, "https://dup", true).unwrap();
    }

    #[test]
    fn check_duplicate_allows_url_once_completed_or_cancelled() {
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        let item = new_test_item(&conn, "https://done", "downloading");
        persistence::finish_item(&conn, item.id, "completed", Some("/tmp/x.mp4"), None).unwrap();

        check_duplicate(&conn, "https://done", false).unwrap();
    }

    #[test]
    fn check_duplicate_allows_unrelated_url() {
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        new_test_item(&conn, "https://dup", "queued");

        check_duplicate(&conn, "https://different", false).unwrap();
    }
}
