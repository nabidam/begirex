//! Global proxy, default N, default output dir/template, default preset id,
//! build flavor (ARCHITECTURE §2). Read/write goes through `persistence`'s
//! settings key/value helpers; this module owns the `Settings` shape and
//! what counts as a valid update.

use crate::error::AppError;
use crate::persistence;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Settings {
    pub global_proxy: Option<String>,
    pub default_concurrency: i64,
    pub default_output_dir: String,
    pub default_output_template: String,
    pub default_preset_id: Option<i64>,
    pub build_flavor: String,
    pub ytdlp_version: Option<String>,
    pub ffmpeg_version: Option<String>,
}

/// Partial update payload for `update_settings`. Only present (`Some`) fields
/// are applied; `None` leaves that setting untouched.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SettingsUpdate {
    pub global_proxy: Option<String>,
    pub default_concurrency: Option<i64>,
    pub default_output_dir: Option<String>,
    pub default_output_template: Option<String>,
    pub default_preset_id: Option<i64>,
}

pub fn get_settings(conn: &Connection) -> Result<Settings, AppError> {
    Ok(Settings {
        global_proxy: persistence::get_setting(conn, "global_proxy")?,
        default_concurrency: persistence::get_setting(conn, "default_concurrency")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(2),
        default_output_dir: persistence::get_setting(conn, "default_output_dir")?
            .unwrap_or_default(),
        default_output_template: persistence::get_setting(conn, "default_output_template")?
            .unwrap_or_default(),
        default_preset_id: persistence::get_setting(conn, "default_preset_id")?
            .and_then(|v| v.parse().ok()),
        build_flavor: persistence::get_setting(conn, "build_flavor")?.unwrap_or_default(),
        ytdlp_version: persistence::get_setting(conn, "ytdlp_version")?,
        ffmpeg_version: persistence::get_setting(conn, "ffmpeg_version")?,
    })
}

/// Applies `update` to the `settings` table and returns the resulting
/// `Settings`. Validation (e.g. `default_concurrency >= 1`) is the ipc
/// layer's job per CONVENTIONS "validation happens only at the trust
/// boundary" — this fn trusts its input is already validated.
pub fn update_settings(conn: &Connection, update: SettingsUpdate) -> Result<Settings, AppError> {
    if let Some(proxy) = &update.global_proxy {
        persistence::set_setting(conn, "global_proxy", proxy)?;
    }
    if let Some(n) = update.default_concurrency {
        persistence::set_setting(conn, "default_concurrency", &n.to_string())?;
    }
    if let Some(dir) = &update.default_output_dir {
        persistence::set_setting(conn, "default_output_dir", dir)?;
    }
    if let Some(template) = &update.default_output_template {
        persistence::set_setting(conn, "default_output_template", template)?;
    }
    if let Some(preset_id) = update.default_preset_id {
        persistence::set_setting(conn, "default_preset_id", &preset_id.to_string())?;
    }
    get_settings(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_settings_round_trips_global_proxy() {
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        persistence::set_setting(&conn, "default_concurrency", "2").unwrap();
        persistence::set_setting(&conn, "default_output_dir", "/tmp/out").unwrap();
        persistence::set_setting(&conn, "default_output_template", "%(title)s.%(ext)s").unwrap();
        persistence::set_setting(&conn, "build_flavor", "light").unwrap();

        let updated = update_settings(
            &conn,
            SettingsUpdate {
                global_proxy: Some("socks5://127.0.0.1:9050".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            updated.global_proxy.as_deref(),
            Some("socks5://127.0.0.1:9050")
        );

        let fetched = get_settings(&conn).unwrap();
        assert_eq!(
            fetched.global_proxy.as_deref(),
            Some("socks5://127.0.0.1:9050")
        );
    }

    #[test]
    fn update_settings_leaves_unset_fields_untouched() {
        let conn = Connection::open_in_memory().unwrap();
        persistence::migrate_for_test(&conn);
        persistence::set_setting(&conn, "default_output_dir", "/tmp/out").unwrap();

        update_settings(
            &conn,
            SettingsUpdate {
                global_proxy: Some("http://proxy".into()),
                ..Default::default()
            },
        )
        .unwrap();

        let fetched = get_settings(&conn).unwrap();
        assert_eq!(fetched.default_output_dir, "/tmp/out");
    }
}
