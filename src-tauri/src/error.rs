//! Typed error surface shared across all IPC commands (ARCHITECTURE §7.1).
//! Every command returns `Result<T, AppError>`; Tauri serializes the `Err`
//! side to JSON via `serde::Serialize`, so the frontend always gets
//! `{code, message, stderr?}`.

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppError {
    #[error("{message}")]
    BinaryNotFound { message: String },

    #[error("{message}")]
    BinaryDownloadFailed { message: String },

    #[error("{message}")]
    ProbeFailed { message: String, stderr: Option<String> },

    #[error("{message}")]
    InvalidFormatExpr { message: String, stderr: Option<String> },

    #[error("{message}")]
    DuplicateUrl { message: String },

    #[error("{message}")]
    PresetNameTaken { message: String },

    #[error("{message}")]
    LastPreset { message: String },

    #[error("{message}")]
    DbError { message: String },

    #[error("{message}")]
    ProcessError { message: String, stderr: Option<String> },

    #[error("{message}")]
    Validation { message: String },

    #[error("{message}")]
    IoError { message: String },
}

// ponytail: rusqlite errors always carry enough context in their Display
// impl for the message field; a richer mapping (e.g. distinguishing
// constraint violations) can be added if a later task needs it.
impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::DbError {
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_not_found_serializes_with_wire_code() {
        let err = AppError::BinaryNotFound {
            message: "not found".into(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "BINARY_NOT_FOUND");
        assert_eq!(json["message"], "not found");
    }

    #[test]
    fn validation_serializes_with_wire_code() {
        let err = AppError::Validation {
            message: "bad input".into(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "VALIDATION");
    }
}
