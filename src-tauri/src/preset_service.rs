//! Preset CRUD + single-default/last-preset invariants (ARCHITECTURE §2, §3).
//! `format_expr` dry-parsing (via `engine_supervisor`, needs `.await`) is the
//! caller's (ipc.rs's) job, done *before* the DB connection is locked —
//! `rusqlite::Connection` isn't `Send`, so it can never be held across an
//! `.await` inside a `#[tauri::command]` future. This module's fns are
//! therefore plain sync DB writes; ipc.rs sequences dry-parse-then-write.
//! Does NOT apply presets to a download — the frontend composes the
//! effective config from a fetched `Preset` (ARCHITECTURE §2 "Apply presets
//! to a download" is explicitly out of scope here).

use crate::error::AppError;
use crate::persistence::{self, NewPreset, Preset, PresetUpdate};
use rusqlite::Connection;
use serde::Deserialize;

pub fn list_presets(conn: &Connection) -> Result<Vec<Preset>, AppError> {
    persistence::list_presets(conn)
}

/// Request payload for `create_preset` — `is_default` lets S6's "+ New
/// preset" flow star a preset immediately on creation (K4's default-star
/// behavior), though the common case leaves it `false`/absent.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CreatePresetRequest {
    pub name: String,
    pub format_expr: String,
    pub output_template: String,
    pub proxy: Option<String>,
    pub extra_args: Option<String>,
    pub is_default: Option<bool>,
}

/// Inserts a preset (caller has already dry-parsed `format_expr` — see
/// module doc). If `is_default` is requested, clears the previous default
/// first so the DB's partial unique index (ARCHITECTURE §3) never sees two
/// rows with `is_default=1` at once.
pub fn create_preset(conn: &Connection, request: CreatePresetRequest) -> Result<Preset, AppError> {
    let make_default = request.is_default.unwrap_or(false);
    if make_default {
        persistence::clear_default_preset(conn)?;
    }
    persistence::insert_preset(
        conn,
        NewPreset {
            name: request.name,
            format_expr: request.format_expr,
            output_template: request.output_template,
            proxy: request.proxy,
            extra_args: request.extra_args,
            is_default: make_default,
        },
    )
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdatePresetRequest {
    pub name: Option<String>,
    pub format_expr: Option<String>,
    pub output_template: Option<String>,
    pub proxy: Option<Option<String>>,
    pub extra_args: Option<Option<String>>,
}

/// Updates a preset (caller has already dry-parsed `format_expr`, if
/// present — see module doc). Does not touch `is_default` — that's
/// `set_default_preset`'s job, kept separate so "star this preset" can't
/// accidentally get bundled with an in-flight edit of unrelated fields.
pub fn update_preset(conn: &Connection, id: i64, request: UpdatePresetRequest) -> Result<Preset, AppError> {
    persistence::update_preset(
        conn,
        id,
        PresetUpdate {
            name: request.name,
            format_expr: request.format_expr,
            output_template: request.output_template,
            proxy: request.proxy,
            extra_args: request.extra_args,
        },
    )
}

/// Deletes a preset, refusing the last one (K4-AC6 `LAST_PRESET`) and
/// promoting the next-lowest-id preset to default when the deleted one was
/// the default (ARCHITECTURE §3 "exactly one default must exist whenever
/// ≥1 preset exists").
pub fn delete_preset(conn: &Connection, id: i64) -> Result<Vec<Preset>, AppError> {
    if persistence::count_presets(conn)? <= 1 {
        return Err(AppError::LastPreset {
            message: "cannot delete the last preset".into(),
        });
    }

    let deleted = persistence::get_preset(conn, id)?;
    persistence::delete_preset(conn, id)?;

    if deleted.is_default {
        if let Some(promote_id) = persistence::first_other_preset_id(conn, id)? {
            persistence::set_default_preset(conn, promote_id)?;
        }
    }

    persistence::list_presets(conn)
}

/// Stars `id` as the default, un-starring whatever was default before
/// (K4-AC2) — clear-then-set so the DB's partial unique index never briefly
/// sees two `is_default=1` rows.
pub fn set_default_preset(conn: &Connection, id: i64) -> Result<Vec<Preset>, AppError> {
    persistence::get_preset(conn, id)?; // 404s as DbError if id doesn't exist
    persistence::clear_default_preset(conn)?;
    persistence::set_default_preset(conn, id)?;
    persistence::list_presets(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        conn
    }

    // Dry-parse itself (engine_supervisor::dry_parse_format spawning a real
    // yt-dlp) is exercised in engine_supervisor.rs's own tests and end-to-end
    // via ipc.rs's create_preset/update_preset commands, which run dry-parse
    // *before* calling these fns (see module doc — Connection isn't Send
    // across an .await, so this module never awaits anything itself).

    #[test]
    fn create_preset_persists_valid_expression() {
        let conn = seeded_conn();
        let preset = create_preset(
            &conn,
            CreatePresetRequest {
                name: "4K".into(),
                format_expr: "bv*[height<=2160]+ba/b".into(),
                output_template: "%(title)s.%(ext)s".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(preset.format_expr, "bv*[height<=2160]+ba/b");
        assert!(!preset.is_default);
    }

    #[test]
    fn create_preset_as_default_unstars_previous() {
        let conn = seeded_conn();
        let archive = create_preset(
            &conn,
            CreatePresetRequest {
                name: "Archive".into(),
                format_expr: "bv*+ba/b".into(),
                output_template: "%(title)s.%(ext)s".into(),
                is_default: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(archive.is_default);

        let fourk = create_preset(
            &conn,
            CreatePresetRequest {
                name: "4K".into(),
                format_expr: "bv*[height<=2160]+ba/b".into(),
                output_template: "%(title)s.%(ext)s".into(),
                is_default: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(fourk.is_default);
        assert!(!persistence::get_preset(&conn, archive.id).unwrap().is_default);
    }

    #[test]
    fn update_preset_applies_new_expression() {
        let conn = seeded_conn();
        let preset = create_preset(
            &conn,
            CreatePresetRequest {
                name: "4K".into(),
                format_expr: "bv*[height<=2160]+ba/b".into(),
                output_template: "%(title)s.%(ext)s".into(),
                ..Default::default()
            },
        )
        .unwrap();

        let updated = update_preset(
            &conn,
            preset.id,
            UpdatePresetRequest {
                format_expr: Some("bv*[height<=1080]+ba/b".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.format_expr, "bv*[height<=1080]+ba/b");
    }

    #[test]
    fn delete_preset_refuses_the_last_one() {
        let conn = seeded_conn();
        let only = persistence::insert_preset(
            &conn,
            NewPreset {
                name: "Only".into(),
                format_expr: "bv*+ba/b".into(),
                output_template: "%(title)s.%(ext)s".into(),
                proxy: None,
                extra_args: None,
                is_default: true,
            },
        )
        .unwrap();

        let err = delete_preset(&conn, only.id).unwrap_err();
        assert!(matches!(err, AppError::LastPreset { .. }));
    }

    #[test]
    fn delete_preset_promotes_next_when_default_is_deleted() {
        let conn = seeded_conn();
        let archive = persistence::insert_preset(
            &conn,
            NewPreset {
                name: "Archive".into(),
                format_expr: "bv*+ba/b".into(),
                output_template: "%(title)s.%(ext)s".into(),
                proxy: None,
                extra_args: None,
                is_default: true,
            },
        )
        .unwrap();
        let fourk = persistence::insert_preset(
            &conn,
            NewPreset {
                name: "4K".into(),
                format_expr: "bv*[height<=2160]+ba/b".into(),
                output_template: "%(title)s.%(ext)s".into(),
                proxy: None,
                extra_args: None,
                is_default: false,
            },
        )
        .unwrap();

        let remaining = delete_preset(&conn, archive.id).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, fourk.id);
        assert!(remaining[0].is_default);
    }

    #[test]
    fn set_default_preset_moves_the_star() {
        let conn = seeded_conn();
        let archive = persistence::insert_preset(
            &conn,
            NewPreset {
                name: "Archive".into(),
                format_expr: "bv*+ba/b".into(),
                output_template: "%(title)s.%(ext)s".into(),
                proxy: None,
                extra_args: None,
                is_default: true,
            },
        )
        .unwrap();
        let fourk = persistence::insert_preset(
            &conn,
            NewPreset {
                name: "4K".into(),
                format_expr: "bv*[height<=2160]+ba/b".into(),
                output_template: "%(title)s.%(ext)s".into(),
                proxy: None,
                extra_args: None,
                is_default: false,
            },
        )
        .unwrap();

        let presets = set_default_preset(&conn, fourk.id).unwrap();
        let fourk_row = presets.iter().find(|p| p.id == fourk.id).unwrap();
        let archive_row = presets.iter().find(|p| p.id == archive.id).unwrap();
        assert!(fourk_row.is_default);
        assert!(!archive_row.is_default);
    }
}
