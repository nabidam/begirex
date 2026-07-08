//! IPC command handlers (ARCHITECTURE §2, §7). Translates `invoke` calls into
//! module calls; validates inputs at the trust boundary (§8); holds no state
//! of its own — state lives in `AppState` (lib.rs) and is only borrowed here.

use crate::binary_manager::{self, BinaryStatus, Which};
use crate::engine_supervisor::{self, Emitter, SpawnParams};
use crate::error::AppError;
use crate::persistence::{self, Item};
use crate::progress_parser::{self, ProgressTick};
use crate::settings_service::{self, Settings, SettingsUpdate};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter as _, State};

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
struct TauriEmitter {
    app: AppHandle,
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
}

#[derive(Debug, Serialize)]
pub struct AddDownloadResponse {
    pub items: Vec<Item>,
}

/// Adds one download and, if fewer than 2 items are currently
/// downloading/merging, spawns yt-dlp for it immediately (ARCHITECTURE §7.2,
/// §5). T2 only ever produces exactly 1 item — playlist expansion (N items)
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

    // "spawn if <2 active" — count computed before this row is inserted.
    let active = persistence::count_active_items(&conn)?;
    let stage = if active < 2 { "downloading" } else { "queued" };

    let item = persistence::insert_item(
        &conn,
        persistence::NewItem {
            url: request.url,
            format_expr: request.format_expr,
            output_dir,
            output_template,
            proxy,
            extra_args: request.extra_args,
            preset_id: request.preset_id,
            stage: stage.to_string(),
        },
    )?;

    let db = Arc::clone(&state.db);
    drop(conn);

    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter { app: app.clone() });
    emitter.emit_stage_changed(item.id, &item.stage, None);

    if stage == "downloading" {
        let params = SpawnParams {
            ytdlp_path,
            ffmpeg_path,
            url: item.url.clone(),
            format_expr: item.format_expr.clone(),
            output_dir: item.output_dir.clone(),
            output_template: item.output_template.clone(),
            proxy: item.proxy.clone(),
            extra_args: item.extra_args.clone(),
        };
        let item_id = item.id;
        tauri::async_runtime::spawn(async move {
            engine_supervisor::run_download(db, item_id, params, emitter).await;
        });
    }

    Ok(AddDownloadResponse { items: vec![item] })
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
