# Bundled binaries

The `bundled` flavor (ARCHITECTURE §9, T20) ships yt-dlp/ffmpeg alongside the
app so a machine with neither installed can still download on first run
(K1-AC6, V5-AC2).

This folder is where those two binaries go before running a bundled build:

```
binaries/bin/yt-dlp        (or yt-dlp.exe on Windows)
binaries/bin/ffmpeg        (or ffmpeg.exe on Windows)
```

`tauri.bundled.conf.json` maps `binaries/bin/*` → the packaged app's resource
dir (`bin/`); `binary_manager::bundled_binary_path` + `lib.rs`'s
`#[cfg(feature = "bundled")]` setup block resolve and seed those paths into
`settings` on first run.

ponytail: binaries aren't checked into this repo — they're large,
per-platform, and already fetchable at the exact URLs
`binary_manager::download_url` uses for T16's in-app download. The release
pipeline populates this folder (same fetch, run once at build time instead of
at runtime) before invoking `scripts/build-flavors.sh bundled <os>`. Upgrade
path if that pipeline doesn't exist yet: run the light build's own "Download
for me" once, then copy its `<app-data>/bin/{yt-dlp,ffmpeg}` output here.
