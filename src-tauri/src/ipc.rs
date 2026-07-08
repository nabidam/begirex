//! IPC command handlers (ARCHITECTURE §2, §7). Translates `invoke` calls into
//! module calls; validates inputs at the trust boundary (§8); holds no state
//! of its own — state lives in `AppState` (lib.rs) and is only borrowed here.

use crate::binary_manager::{self, BinaryStatus, Which};
use crate::error::AppError;
use crate::settings_service::{self, Settings, SettingsUpdate};
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

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
