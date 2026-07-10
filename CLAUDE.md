# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

BegireX: single-process desktop yt-dlp GUI. Tauri 2 shell, Svelte 5 (runes) frontend, Rust backend, SQLite persistence. Linux-primary dev target; Windows/macOS supported but verified by inspection, not CI, per CONVENTIONS.md.

## Commands

Frontend (from repo root):
- `npm run dev` — Vite dev server (used by `tauri dev`, not usually run standalone)
- `npm run build` — Vite build to `dist/`
- `npx tauri dev` — run the full app (frontend + Rust backend) in dev mode
- `npx tauri build` — production build, `light` flavor (default)

Backend (from `src-tauri/`):
- `cargo test` — unit + integration tests. Network-dependent tests (real yt-dlp spawn) are `#[ignore]`; run explicitly with `cargo test -- --ignored` at demo gates.
- `cargo test <test_name>` — single test
- `scripts/build-flavors.sh [light|bundled|all]` — packaging entry point (see build flavors below)

There is no lint/format command wired up beyond `cargo fmt`/`cargo clippy` defaults — no CI config in this repo yet.

## Architecture

Full contract lives in `ARCHITECTURE.md` (living doc — patch it in the same commit as code that changes its truth). Read it before touching backend module boundaries. Key points:

**Boundary law**: frontend holds no durable state. It is a projection of backend state via `invoke()` calls and `listen()` events. Progress, stage, and scheduling decisions are always backend-emitted — the frontend never computes durable truth.

**Backend modules** (`src-tauri/src/`, flat, one file per concern):
- `binary_manager` — detect/download/health-check yt-dlp & ffmpeg
- `engine_supervisor` — spawns yt-dlp child processes, streams stdout/stderr, enforces concurrency, pause/cancel/resume (pause = kill + resume with `-c`, no SIGSTOP)
- `queue_manager` — authoritative in-memory queue, scheduling (next `queued` item on free slot), playlist expansion, sole writer to `items` table
- `persistence` — all SQLite reads/writes, migrations, crash-safe checkpoint writes
- `preset_service` — preset CRUD, single-default invariant, format-expression dry-parse
- `settings_service` — global settings (proxy, concurrency N, output dir/template, build flavor)
- `ipc` — command handlers + event emitters; the only layer that validates input (trust boundary)
- `error.rs` — single `AppError` enum (`thiserror`), matches ARCHITECTURE §7.1 codes exactly; every command returns `Result<T, AppError>`; engine failures are data (stage=`error`+`error_message`), never `panic!`/`unwrap` on child-process or DB paths

Module boundaries are enforced by convention, not code: e.g. binary_manager must not touch queue/DB tables other than `settings`; engine_supervisor decides nothing about *which* item runs next (queue_manager's job); ipc holds no state itself.

**Frontend** (`src/`):
- `lib/ipc.ts` — the **only** file that imports `@tauri-apps/api`; typed invoke/listen wrappers
- `lib/types.ts` — wire types mirroring ARCHITECTURE §7 shapes verbatim (snake_case fields, no renaming layer)
- `lib/stores/*.svelte.ts` — one rune store per ARCHITECTURE §2 store name (`queue`, `filters`, `presets`, `settings`, `binaryHealth`), hydrated from backend and updated by events
- `lib/views/*.svelte` — one file per screen/overlay, referred to by UX id S1–S7 (Onboarding, Shell, Queue, AddDownload, FormatPicker, DetailDrawer, Presets, Settings)
- `lib/components/` — shared pieces used by ≥2 views; `components/ui/` is shadcn-svelte generated code (don't hand-edit beyond theming)

**Data model**: single SQLite file (`begirex.db`, WAL mode) with `items`, `presets`, `settings`, `item_logs` tables — full DDL in ARCHITECTURE §3. Schema changes are new numbered migrations in `src-tauri/migrations/`; never edit an applied migration.

**Build flavors** (`ARCHITECTURE.md` §9, `src-tauri/Cargo.toml` `bundled` feature): compile-time constant, not a runtime switch.
- `light` (default): detects/downloads yt-dlp & ffmpeg like a normal install
- `bundled`: seeds `ytdlp_path`/`ffmpeg_path` to binaries shipped in `src-tauri/binaries/bin/` (via `tauri.bundled.conf.json`'s `bundle.resources`), skips S1 detection
- Each flavor is a separate `tauri build` invocation (`scripts/build-flavors.sh`); four release artifacts (linux/windows × bundled/light) come from running the script once per OS.

## Conventions (full detail in CONVENTIONS.md)

- Rust modules/files match ARCHITECTURE module names exactly; IPC command fn names = wire names.
- Styling: tokens only via `DESIGN_SYSTEM.md` → shadcn CSS-variable mapping in `src/app.css` (the one file allowed raw values). No raw hex/px in components. Logical properties only (`margin-inline`, not `left/right`).
- No file > ~400 lines without a split; no grab-bag `utils/`; colocate helpers with their one caller until a second exists.
- Conventional commits, scope = module or screen id (`feat(queue_manager): …`, `feat(s3): …`). Living docs (ARCHITECTURE/UX/DESIGN/FILE_STRUCTURE/CONVENTIONS) patched in the same commit as the code that makes them true.
- Deliberate shortcuts get a `ponytail:` comment naming the ceiling and upgrade path (see `Cargo.toml` for an example).

## Other living docs

- `UX.md` — screens/flows by id (S1–S7)
- `DESIGN.md` / `DESIGN_SYSTEM.md` — design-system adoption map and authoritative tokens (purple Material design — do not confuse with any earlier green scheme)
- `FILE_STRUCTURE.md` — full expected file tree, chunk-numbered against `specs/001-core/PLAN.md`
- `specs/001-core/` — SPEC/PRD/PLAN/TASKS for the 001-core feature set
