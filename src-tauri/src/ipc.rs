//! IPC command handlers (ARCHITECTURE §2, §7). Translates `invoke` calls into
//! module calls; validates inputs at the trust boundary (§8); holds no state
//! of its own — state lives in `AppState` (lib.rs) and is only borrowed here.

use crate::binary_manager::{self, BinaryStatus, Which};
use crate::engine_supervisor::{Emitter, ProbeFormat};
use crate::error::AppError;
use crate::persistence::{self, Item, Preset};
use crate::preset_service::{self, CreatePresetRequest, UpdatePresetRequest};
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
    // ponytail: recheck is identical to detect — both run binary_manager's
    // full `--version` probe. T16's mid-session health tracking is a
    // separate, automatic code path (engine_supervisor's pre-spawn existence
    // check, ARCHITECTURE §8) that this command never touches; recheck stays
    // the user-triggered deep check either way.
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
        message: format!(
            "unknown binary '{}', expected 'ytdlp' or 'ffmpeg'",
            request.which
        ),
    })?;

    let conn = state.db.lock().expect("db mutex poisoned");
    binary_manager::set_path(&conn, &which, &request.path)
}

#[derive(Debug, Deserialize)]
pub struct DownloadBinaryRequest {
    pub which: String,
}

#[derive(Serialize, Clone)]
struct BinaryDownloadPayload<'a> {
    which: &'a str,
    percent: f64,
}

/// T16: fetches a missing binary in-app (ARCHITECTURE §7.2 `download_binary`).
/// Downloads to disk with no DB lock held across the network await (T16's
/// `binary_manager::download_to_disk` is deliberately DB-free for that
/// reason), then reuses `set_path` to validate + persist the result exactly
/// like a user manually browsing to it in S1.
#[tauri::command]
pub async fn download_binary(
    app: AppHandle,
    state: State<'_, AppState>,
    request: DownloadBinaryRequest,
) -> Result<BinaryStatus, AppError> {
    let which = Which::parse(&request.which).ok_or_else(|| AppError::Validation {
        message: format!(
            "unknown binary '{}', expected 'ytdlp' or 'ffmpeg'",
            request.which
        ),
    })?;
    let app_data_dir = app.path().app_data_dir().map_err(|e| AppError::IoError {
        message: format!("could not resolve app data dir: {e}"),
    })?;

    let progress_app = app.clone();
    let which_label = request.which.clone();
    let downloaded_path = binary_manager::download_to_disk(&app_data_dir, which, move |percent| {
        let _ = progress_app.emit(
            "binary_download",
            BinaryDownloadPayload {
                which: &which_label,
                percent,
            },
        );
    })
    .await?;

    let conn = state.db.lock().expect("db mutex poisoned");
    binary_manager::set_path(&conn, &which, &downloaded_path)
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
        let _ = self
            .app
            .emit("item_removed", ItemRemovedPayload { id: item_id });
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

    fn emit_binary_health(&self, which: &str, found: bool, path: Option<&str>) {
        let _ = self
            .app
            .emit("binary_health", BinaryHealthPayload { which, found, path });
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

#[derive(Serialize, Clone)]
struct BinaryHealthPayload<'a> {
    which: &'a str,
    found: bool,
    path: Option<&'a str>,
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

/// Adds a download and routes the scheduling decision ("spawn if fewer than
/// N active, else queue") through `queue_manager` (ARCHITECTURE §7.2, §4/T3)
/// — ipc.rs only resolves inputs (settings/binary paths) and stays thin. T19:
/// every submitted URL goes through `queue_manager::add_download_expanding`,
/// which expands a real playlist into N independently-scheduled rows sharing
/// a `playlist_id` and passes a lone video through as the one row it always
/// was (ARCHITECTURE §7.2 "N rows for a playlist").
#[tauri::command]
pub async fn add_download(
    app: AppHandle,
    state: State<'_, AppState>,
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

    let (ytdlp_path, ffmpeg_path, output_dir, output_template, proxy, n_slots) = {
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
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| settings.default_output_dir.clone());
        let output_template = request
            .output_template
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| settings.default_output_template.clone());
        let proxy = request
            .proxy
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| settings.global_proxy.clone());
        let n_slots = settings.default_concurrency;

        (
            ytdlp_path,
            ffmpeg_path,
            output_dir,
            output_template,
            proxy,
            n_slots,
        )
    };

    let db = Arc::clone(&state.db);
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app.clone()));
    let binaries = BinaryPaths {
        ytdlp_path,
        ffmpeg_path,
    };

    let items = queue_manager::add_download_expanding(
        db,
        emitter,
        binaries,
        n_slots,
        Arc::clone(&state.registry),
        AddDownloadParams {
            url: request.url,
            format_expr: request.format_expr,
            output_dir,
            output_template,
            proxy,
            extra_args: request.extra_args,
            preset_id: request.preset_id,
            playlist_id: None,
            title: None,
        },
    )
    .await?;

    Ok(AddDownloadResponse { items })
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
pub async fn pause_item(
    app: AppHandle,
    state: State<'_, AppState>,
    request: GetItemRequest,
) -> Result<Item, AppError> {
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app));
    queue_manager::pause_item(
        Arc::clone(&state.db),
        emitter,
        Arc::clone(&state.registry),
        request.id,
    )
    .await
}

#[tauri::command]
pub fn resume_item(
    app: AppHandle,
    state: State<AppState>,
    request: GetItemRequest,
) -> Result<Item, AppError> {
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
pub async fn cancel_item(
    app: AppHandle,
    state: State<'_, AppState>,
    request: GetItemRequest,
) -> Result<Item, AppError> {
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
pub async fn remove_item(
    app: AppHandle,
    state: State<'_, AppState>,
    request: GetItemRequest,
) -> Result<OkResponse, AppError> {
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
pub fn retry_item(
    app: AppHandle,
    state: State<AppState>,
    request: GetItemRequest,
) -> Result<Item, AppError> {
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
pub fn reorder_item(
    state: State<AppState>,
    request: ReorderItemRequest,
) -> Result<OkResponse, AppError> {
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
pub async fn bulk_action(
    app: AppHandle,
    state: State<'_, AppState>,
    request: BulkActionRequest,
) -> Result<BulkActionResponse, AppError> {
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
pub fn set_concurrency(
    app: AppHandle,
    state: State<AppState>,
    request: SetConcurrencyRequest,
) -> Result<SetConcurrencyResponse, AppError> {
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
pub fn list_items(
    state: State<AppState>,
    request: ListItemsRequest,
) -> Result<Vec<Item>, AppError> {
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
    let mut watchers = state
        .log_watchers
        .lock()
        .expect("log watchers mutex poisoned");
    if request.on {
        watchers.insert(request.id);
    } else {
        watchers.remove(&request.id);
    }
    Ok(OkResponse { ok: true })
}

// --- T9: probe (S3/S4) ------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ProbeFormatsRequest {
    pub url: String,
    pub proxy: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProbeFormatsResponse {
    pub title: String,
    pub formats: Vec<ProbeFormat>,
}

/// Resolves yt-dlp's path + the effective proxy (per-request override falls
/// back to the global setting, same precedence `add_download` uses) then
/// delegates to `engine_supervisor::probe` (ARCHITECTURE §7.2).
#[tauri::command]
pub async fn probe_formats(
    state: State<'_, AppState>,
    request: ProbeFormatsRequest,
) -> Result<ProbeFormatsResponse, AppError> {
    if request.url.trim().is_empty() {
        return Err(AppError::Validation {
            message: "url must not be empty".into(),
        });
    }

    let (ytdlp_path, proxy) = {
        let conn = state.db.lock().expect("db mutex poisoned");
        let ytdlp_path = binary_manager::detect(&conn, &Which::Ytdlp)?
            .path
            .ok_or_else(|| AppError::BinaryNotFound {
                message: "yt-dlp not found; set its path in Settings".into(),
            })?;
        let settings = settings_service::get_settings(&conn)?;
        let proxy = request
            .proxy
            .filter(|p| !p.trim().is_empty())
            .or(settings.global_proxy);
        (ytdlp_path, proxy)
    };

    let result =
        crate::engine_supervisor::probe(&ytdlp_path, &request.url, proxy.as_deref()).await?;
    Ok(ProbeFormatsResponse {
        title: result.title,
        formats: result.formats,
    })
}

// --- T11: presets (S6, S3) --------------------------------------------------

fn resolve_ytdlp_path(conn: &rusqlite::Connection) -> Result<String, AppError> {
    binary_manager::detect(conn, &Which::Ytdlp)?
        .path
        .ok_or_else(|| AppError::BinaryNotFound {
            message: "yt-dlp not found; set its path in Settings".into(),
        })
}

#[tauri::command]
pub fn list_presets(state: State<AppState>) -> Result<Vec<Preset>, AppError> {
    let conn = state.db.lock().expect("db mutex poisoned");
    preset_service::list_presets(&conn)
}

/// Trust-boundary validation (§8): non-empty name/format_expr/output_template
/// before any module call. Dry-parse + the DB's `PRESET_NAME_TAKEN` uniqueness
/// check happen inside `preset_service::create_preset`.
#[tauri::command]
pub async fn create_preset(
    state: State<'_, AppState>,
    request: CreatePresetRequest,
) -> Result<Preset, AppError> {
    if request.name.trim().is_empty() {
        return Err(AppError::Validation {
            message: "name must not be empty".into(),
        });
    }
    if request.format_expr.trim().is_empty() {
        return Err(AppError::Validation {
            message: "format_expr must not be empty".into(),
        });
    }
    if request.output_template.trim().is_empty() {
        return Err(AppError::Validation {
            message: "output_template must not be empty".into(),
        });
    }

    let ytdlp_path = {
        let conn = state.db.lock().expect("db mutex poisoned");
        resolve_ytdlp_path(&conn)?
    };
    // Dry-parse before locking the connection: `rusqlite::Connection` isn't
    // `Send`, so it can never be held across this `.await` inside a
    // `#[tauri::command]` future (see preset_service.rs's module doc).
    crate::engine_supervisor::dry_parse_format(&ytdlp_path, &request.format_expr).await?;

    let conn = state.db.lock().expect("db mutex poisoned");
    preset_service::create_preset(&conn, request)
}

#[derive(Debug, Deserialize)]
pub struct UpdatePresetPathRequest {
    pub id: i64,
    #[serde(flatten)]
    pub update: UpdatePresetRequest,
}

#[tauri::command]
pub async fn update_preset(
    state: State<'_, AppState>,
    request: UpdatePresetPathRequest,
) -> Result<Preset, AppError> {
    if let Some(name) = &request.update.name {
        if name.trim().is_empty() {
            return Err(AppError::Validation {
                message: "name must not be empty".into(),
            });
        }
    }
    if let Some(expr) = &request.update.format_expr {
        if expr.trim().is_empty() {
            return Err(AppError::Validation {
                message: "format_expr must not be empty".into(),
            });
        }
    }

    let ytdlp_path = {
        let conn = state.db.lock().expect("db mutex poisoned");
        resolve_ytdlp_path(&conn)?
    };
    if let Some(expr) = &request.update.format_expr {
        crate::engine_supervisor::dry_parse_format(&ytdlp_path, expr).await?;
    }

    let conn = state.db.lock().expect("db mutex poisoned");
    preset_service::update_preset(&conn, request.id, request.update)
}

#[derive(Debug, Serialize)]
pub struct PresetListResponse {
    pub presets: Vec<Preset>,
}

#[tauri::command]
pub fn delete_preset(
    state: State<AppState>,
    request: GetItemRequest,
) -> Result<PresetListResponse, AppError> {
    let conn = state.db.lock().expect("db mutex poisoned");
    let presets = preset_service::delete_preset(&conn, request.id)?;
    Ok(PresetListResponse { presets })
}

#[tauri::command]
pub fn set_default_preset(
    state: State<AppState>,
    request: GetItemRequest,
) -> Result<PresetListResponse, AppError> {
    let conn = state.db.lock().expect("db mutex poisoned");
    let presets = preset_service::set_default_preset(&conn, request.id)?;
    Ok(PresetListResponse { presets })
}

// --- T15: S5 detail drawer — open_path (ARCHITECTURE §7.2) ------------------

#[derive(Debug, Deserialize)]
pub struct OpenPathRequest {
    pub path: String,
    pub reveal: Option<bool>,
}

/// `reveal:true` opens the path's parent directory (used for a completed
/// item's "Open folder"); otherwise opens the path itself ("Open file", or
/// a plain directory like `output_dir`). Pure so it's unit-testable without
/// shelling out.
fn resolve_open_target(path: &str, reveal: bool) -> std::path::PathBuf {
    if reveal {
        match std::path::Path::new(path).parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => std::path::PathBuf::from(path),
        }
    } else {
        std::path::PathBuf::from(path)
    }
}

/// ponytail: shells out to the OS's default opener rather than adding a
/// cross-platform "opener" crate — Linux (`xdg-open`) is this project's
/// primary target (no other runtime dep added anywhere else either); the
/// macOS/Windows branches are the standard `open`/`explorer` invocations,
/// untested on those OSes. Upgrade path: `tauri-plugin-opener` if a real
/// cross-platform gap shows up.
#[tauri::command]
pub fn open_path(request: OpenPathRequest) -> Result<OkResponse, AppError> {
    if request.path.trim().is_empty() {
        return Err(AppError::Validation {
            message: "path must not be empty".into(),
        });
    }
    let target = resolve_open_target(&request.path, request.reveal.unwrap_or(false));

    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(&target).status();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(&target).status();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer").arg(&target).status();

    match result {
        Ok(status) if status.success() => Ok(OkResponse { ok: true }),
        Ok(status) => Err(AppError::IoError {
            message: format!(
                "failed to open '{}': exited with {status}",
                target.display()
            ),
        }),
        Err(err) => Err(AppError::IoError {
            message: format!("failed to open '{}': {err}", target.display()),
        }),
    }
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
                playlist_id: None,
                title: None,
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

    #[test]
    fn resolve_open_target_without_reveal_returns_path_itself() {
        let target = resolve_open_target("/tmp/out/video.mp4", false);
        assert_eq!(target, std::path::PathBuf::from("/tmp/out/video.mp4"));
    }

    #[test]
    fn resolve_open_target_with_reveal_returns_parent_dir() {
        let target = resolve_open_target("/tmp/out/video.mp4", true);
        assert_eq!(target, std::path::PathBuf::from("/tmp/out"));
    }

    #[test]
    fn resolve_open_target_with_reveal_falls_back_when_no_parent() {
        let target = resolve_open_target("video.mp4", true);
        assert_eq!(target, std::path::PathBuf::from("video.mp4"));
    }
}
