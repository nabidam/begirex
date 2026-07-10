# BegireX

Desktop GUI for yt-dlp. Tauri 2 + Svelte 5 frontend, Rust backend, SQLite persistence.

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

## Docs

- `CLAUDE.md` — guidance for Claude Code working in this repo
- `ARCHITECTURE.md` — system contract: modules, data model, IPC contract
- `UX.md` — screens and flows (S1–S7)
- `DESIGN.md` / `DESIGN_SYSTEM.md` — design tokens and adoption map
- `CONVENTIONS.md` — naming, error handling, folder rules, commit style
- `FILE_STRUCTURE.md` — full expected file tree
- `specs/001-core/` — spec, PRD, plan, tasks for the current feature set
