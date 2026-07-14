# BegireX

> A local-first desktop download manager powered by yt-dlp.

BegireX is a Tauri desktop application for managing video and audio downloads
through a clear queue-based interface. It uses yt-dlp and FFmpeg locally; no
accounts, telemetry, or cloud service are required.

**Status:** v0.1.0 is an early release. Please report bugs and avoid relying
on it for irreplaceable download queues.

## Features

- Queue, pause, resume, retry, reorder, and cancel downloads
- Inspect available formats before adding a download
- Save reusable format and output presets
- Persist queues and settings locally in SQLite
- Find or download yt-dlp and FFmpeg during onboarding
- Choose a light build (default) or a build that bundles those executables

## Install

Download the installer for your platform from the repository's
[Releases](https://github.com/nabidam/begirex/releases) page. The first launch
guides you through locating or downloading yt-dlp and FFmpeg.

Use BegireX only to download content you are authorized to access, and comply
with the terms of the source service and applicable law.

## Stack

- Shell: Tauri 2
- Frontend: Svelte 5 (runes), TypeScript, Vite, shadcn-svelte + Tailwind
- Backend: Rust (tokio async runtime, `tokio::process` for yt-dlp/ffmpeg child processes)
- Persistence: SQLite (single file in app-data dir, WAL mode)

## Develop

```bash
npm install
npx tauri dev
```

## Build

```bash
npx tauri build                       # light flavor: detects/downloads yt-dlp+ffmpeg at runtime
src-tauri/scripts/build-flavors.sh    # light + bundled flavors (see below)
```

Two build flavors (compile-time, `ARCHITECTURE.md` §9):
- **light** (default) — detects yt-dlp/ffmpeg on PATH or downloads them in-app
- **bundled** — ships yt-dlp/ffmpeg binaries with the app (`src-tauri/binaries/bin/`), skips first-run detection

## Test

```bash
cd src-tauri
cargo test              # unit + integration tests
cargo test -- --ignored # includes tests that spawn real yt-dlp (network)
```

CI also enforces Rust formatting and verifies that the frontend production
bundle builds successfully.

## Security

Please report vulnerabilities privately as described in
[SECURITY.md](SECURITY.md). Do not open public issues for suspected security
problems.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) and adhere to
the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

BegireX is released under the [MIT License](LICENSE). yt-dlp and FFmpeg are
separate projects with their own licenses; BegireX does not grant rights to
them.

## Docs

- `CLAUDE.md` — guidance for Claude Code working in this repo
- `ARCHITECTURE.md` — system contract: modules, data model, IPC contract
- `UX.md` — screens and flows (S1–S7)
- `DESIGN.md` / `DESIGN_SYSTEM.md` — design tokens and adoption map
- `CONVENTIONS.md` — naming, error handling, folder rules, commit style
- `FILE_STRUCTURE.md` — full expected file tree
- `specs/001-core/` — spec, PRD, plan, tasks for the current feature set
