//! Integration tests for T1 acceptance criteria 1 & 2 (binary_manager),
//! against a real SQLite file (not in-memory) and a real spawned process —
//! either the sandbox's actual yt-dlp/ffmpeg if present, or a fabricated
//! fake "binary" script when neither is available (no network/real yt-dlp
//! required, so these are NOT `#[ignore]`).

#![cfg(unix)]

use begirex_lib::binary_manager::{detect, set_path, Which};
use begirex_lib::persistence;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn temp_db_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "begirex_it_{name}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir.join("begirex.db")
}

/// Writes a tiny shell script at `path` that responds to `--version` with
/// `fake-binary 1.0` and exits 0 — a real spawnable executable, not a mock.
fn write_fake_binary(path: &std::path::Path) {
    fs::write(path, "#!/bin/sh\necho \"fake-binary 1.0\"\nexit 0\n").unwrap();
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();
}

#[test]
fn set_binary_path_rejects_bogus_path_and_leaves_settings_unchanged() {
    let db_path = temp_db_path("bogus");
    let conn = persistence::open_and_init(&db_path, "/tmp").unwrap();

    let before = persistence::get_setting(&conn, "ytdlp_path").unwrap();
    assert_eq!(before, None);

    let err = set_path(&conn, &Which::Ytdlp, "/no/such/binary/anywhere").unwrap_err();
    assert!(matches!(
        err,
        begirex_lib::error::AppError::BinaryNotFound { .. }
    ));

    let after = persistence::get_setting(&conn, "ytdlp_path").unwrap();
    assert_eq!(
        after, None,
        "settings must be unchanged after a rejected path"
    );
}

#[test]
fn set_binary_path_persists_real_binary_then_detect_reports_it_found() {
    let dir = std::env::temp_dir().join(format!(
        "begirex_fake_bin_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let fake_bin = dir.join("fake-ytdlp");
    write_fake_binary(&fake_bin);

    let db_path = temp_db_path("persist");
    let conn = persistence::open_and_init(&db_path, "/tmp").unwrap();

    let status = set_path(&conn, &Which::Ytdlp, fake_bin.to_str().unwrap()).unwrap();
    assert!(status.found);
    assert_eq!(status.version.as_deref(), Some("fake-binary 1.0"));

    // Persisted path survives — a following detect (simulating a fresh
    // command call / process restart) reports it found with its version.
    let redetected = detect(&conn, &Which::Ytdlp).unwrap();
    assert!(redetected.found);
    assert_eq!(redetected.path.as_deref(), Some(fake_bin.to_str().unwrap()));
    assert_eq!(redetected.version.as_deref(), Some("fake-binary 1.0"));

    fs::remove_dir_all(&dir).ok();
}

/// Exercises the real installed yt-dlp/ffmpeg in this sandbox's PATH when
/// present, proving PATH-based `detect_binaries` semantics (criterion 1)
/// end to end. Not `#[ignore]`: doesn't need network, just an installed CLI
/// that may or may not exist on the runner — skips itself if absent rather
/// than failing the suite.
#[test]
fn detect_finds_real_ytdlp_or_ffmpeg_on_path_if_installed() {
    let db_path = temp_db_path("real_path");
    let conn = persistence::open_and_init(&db_path, "/tmp").unwrap();

    let ytdlp = detect(&conn, &Which::Ytdlp).unwrap();
    let ffmpeg = detect(&conn, &Which::Ffmpeg).unwrap();

    if which::which("yt-dlp").is_err() && which::which("ffmpeg").is_err() {
        eprintln!("neither yt-dlp nor ffmpeg on PATH in this environment — skipping assertions");
        return;
    }
    if which::which("yt-dlp").is_ok() {
        assert!(ytdlp.found);
        assert!(ytdlp.version.is_some());
    }
    if which::which("ffmpeg").is_ok() {
        assert!(ffmpeg.found);
        assert!(ffmpeg.version.is_some());
    }
}

/// ponytail: a `which` crate dependency would be one more line than shelling
/// out ourselves, so this tiny local helper avoids adding a dev-dependency
/// just for a test-only PATH probe.
mod which {
    use std::process::Command;

    pub fn which(cmd: &str) -> Result<(), ()> {
        Command::new("which")
            .arg(cmd)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|_| ())
            .ok_or(())
    }
}
