# PLAN — BegireX (001-core)

> Implementation plan in ordered chunks. Each chunk: files touched, requirements, falsifiable acceptance criteria (AC), what NOT to do. Max ~300 lines of new hand-written code per chunk (generated scaffold/lockfiles exempt). Requirement ids (K1–K5, V1–V5, NFR-n, AC-…) refer to `PRD.md`; screen ids (S1–S7) to `UX.md`; commands/events to `ARCHITECTURE.md §7`; tokens to `DESIGN_SYSTEM.md` via `DESIGN.md`.
>
> **Milestone 1 is the WALKING SKELETON**: the thinnest slice that makes the kernel journey (SPEC §1, Flow A) pass in the real app — real yt-dlp child process, real SQLite persistence, real resume. Ugly is fine; fake is not. Later milestones deepen, never re-architect.

---

## Milestone 1 — WALKING SKELETON

### Chunk 1 — Scaffold + database

**Files:** `src-tauri/` (generated Tauri 2 scaffold: `Cargo.toml`, `tauri.conf.json`, `src/main.rs`, `src/lib.rs`), `src/` (generated Svelte 5 + Vite scaffold), `tailwind.config` + `src/app.css` (theme vars per DESIGN.md §2), `src-tauri/src/persistence.rs`, `src-tauri/migrations/001_init.sql`, `components.json`.

**Requirements:**
- `npm create tauri-app` (Svelte 5 + TS + Vite), add Tailwind v4 + shadcn-svelte init; map DESIGN_SYSTEM.md tokens into shadcn CSS variables exactly as DESIGN.md §2 specifies. Add Instrument Sans + JetBrains Mono as packaged fonts (no CDN).
- `persistence.rs`: open `begirex.db` in Tauri app-data dir, `PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;`, run `001_init.sql` — the exact DDL from ARCHITECTURE.md §3 (items, presets, settings, item_logs, all indexes).
- First-run seed (ARCHITECTURE §9): "Default" preset (`is_default=1`, expression `bv*+ba/b`, template `%(title)s.%(ext)s`), settings `default_concurrency=2`, `default_output_dir=<OS Downloads dir>`, `default_output_template=%(title)s.%(ext)s`, `build_flavor=light`.

**AC:**
1. `npm run tauri dev` opens a window with the dark `surface` background and Instrument Sans rendering (visible, not default white/serif).
2. After first launch, `begirex.db` exists in the app-data dir; `sqlite3 … "select name from presets"` returns `Default`; journal mode is `wal`.
3. Second launch does not re-seed (still exactly one preset).

**NOT:** no views, no IPC commands beyond scaffold default, no bundled-flavor logic, no light-mode tokens.

### Chunk 2 — Engine spawn + progress pipeline (backend only)

**Files:** `src-tauri/src/engine_supervisor.rs`, `src-tauri/src/progress_parser.rs`, `src-tauri/src/binary_manager.rs` (detect-only), `src-tauri/src/error.rs` (`AppError` per ARCHITECTURE §7.1), `src-tauri/src/ipc.rs` (first commands).

**Requirements:**
- `binary_manager`: `detect_binaries` — check configured path from settings, then PATH (`which`/`where`), validate by running `--version`; persist resolved path + version to settings. `set_binary_path` re-validates.
- `engine_supervisor`: spawn `yt-dlp` via `tokio::process` with `--newline`, `--progress-template` (machine-parseable line), `-c`, `--ffmpeg-location <ffmpeg_path>`, format expr, `--proxy` when set, `-o <dir>/<template>`, extra args split verbatim. Stream stdout line-by-line; `progress_parser` extracts `{percent, downloaded_bytes, total_bytes, speed_bps, eta_seconds, stage}`; detect `merging` from the ffmpeg merge phase; exit 0 → `completed` + resolve `output_path`; non-zero → `error` + stderr summary into `items.error_message`, full stderr lines into `item_logs`.
- Emit `progress` (throttled ≤10/sec/item) and `stage_changed` (never throttled) events per ARCHITECTURE §7.3.
- Checkpoint `downloaded_bytes`/`percent`/`stage` to SQLite on ticks advancing ≥1% or ≥2s (ARCHITECTURE §8).
- Commands live: `detect_binaries`, `set_binary_path`, `add_download` (single URL only — inserts row, spawns immediately if <2 active), `list_items`, `get_settings`, `update_settings`.

**AC:**
1. Integration test (or `cargo run` + CLI harness): `add_download` on a real small media URL produces monotonically increasing `percent` in emitted events and exit → stage `completed` with the file on disk at `<output_dir>/<template>`-resolved path.
2. An invalid format expression yields stage `error` and `error_message` containing yt-dlp's stderr text (not a generic message).
3. `detect_binaries` on a PATH with yt-dlp reports `{found:true, path, version}`; with a bogus configured path reports `found:false`.

**NOT:** no pause/cancel/reorder, no playlist, no probe, no in-app binary download, no scheduler beyond "spawn if <2 active".

### Chunk 3 — Skeleton UI + restart resume

**Files:** `src/lib/ipc.ts` (typed invoke/listen wrappers — the only `@tauri-apps/api` import), `src/lib/stores/queue.svelte.ts`, `src/lib/stores/settings.svelte.ts`, `src/routes/+page.svelte` (skeleton shell), `src/lib/views/Onboarding.svelte` (minimal S1), `src-tauri/src/queue_manager.rs` (launch reconcile + slot refill).

**Requirements:**
- Minimal S1: on launch call `detect_binaries`; if either missing, block with a plain panel — per-binary "Set path…" (native file dialog → `set_binary_path`) + proxy text field → Continue (calls `update_settings`). Bundled/`I'll set it later`/in-app download NOT yet.
- Skeleton queue page (unstyled beyond theme defaults): URL input, format-expression input (prefilled from the Default preset), Add button → `add_download`; list of items showing title/url, stage, percent, speed, ETA — hydrated from `list_items`, live-updated from `progress`/`stage_changed` events.
- Launch reconcile (ARCHITECTURE §8): items left `downloading`/`merging` → `paused`-equivalent, then auto re-spawn with `-c` up to N=2; UI shows last-checkpointed progress immediately.
- Enforce N=2: third `add_download` while two active inserts as `queued`; when a slot frees, `queue_manager` starts the lowest `queue_position` queued item.

**AC:**
1. **Kernel journey passes end-to-end** (script below, Demo Gate 1).
2. With two items downloading, adding a third shows it `queued`; it flips to `downloading` when one finishes (K2-AC2).
3. `kill -9` the app mid-download, relaunch: both rows render with pre-kill progress, resume, and reported downloaded-bytes on resume ≥ pre-kill value (K2-AC5, NFR-1).

**NOT:** no styling effort beyond mapped theme, no sidebar, no drawer, no presets UI, no virtualization.

### 🚦 DEMO GATE 1 — walking skeleton (kernel journey, minimal)

Walk exactly: launch light build on a machine where ffmpeg is not configured → S1-minimal shows it missing → **Set path…** to a real ffmpeg + enter a proxy → Continue → paste video URL, keep or edit the prefilled expression (`bv*[height<=1080]+ba/b` typed manually counts) → Add → paste second URL → Add → **observe both rows' percent advancing simultaneously** → quit mid-download → relaunch → **observe both resume from prior progress, not 0%** → wait → both `completed`, files on disk at the templated path. Any step failing blocks Milestone 2.

*(In-app ffmpeg download and the preset picker join the journey at Demo Gate 4; the SPEC journey's "Download for me" and "Archive preset" steps are satisfied there.)*

---

## Milestone 2 — Queue depth

### Chunk 4 — Full queue lifecycle

**Files:** `src-tauri/src/queue_manager.rs`, `src-tauri/src/engine_supervisor.rs`, `src-tauri/src/ipc.rs`, `src/lib/stores/queue.svelte.ts`, `src/routes/+page.svelte` (row action buttons, still plain).

**Requirements:**
- Commands: `pause_item` (kill child, keep partial + `resume_capable`), `resume_item` (re-spawn `-c`), `cancel_item` (kill child, stage `cancelled`, delete partial file), `remove_item` (stop if active, delete row + partial), `retry_item` (stage → `queued`), `reorder_item`, `set_concurrency`, `get_item`, `bulk_action` (per-id result list).
- Scheduler per ARCHITECTURE §4: semaphore of size N; on any item leaving active, start lowest-`queue_position` `queued` item. N change resizes; decrease never kills in-flight items.
- Events `item_added`/`item_removed` wired; frontend store applies them without refetch.

**AC:**
1. Pause freezes percent; resume continues from the paused offset (K2-AC6).
2. Cancel frees a slot and a queued item starts (K2-AC7). Cancelled item's partial file is gone from disk.
3. Remove deletes the row; it does not reappear after restart (K2-AC8).
4. Reordering a queued item above another changes which starts next (K2-AC9).
5. `set_concurrency{n:0}` returns `VALIDATION` and N is unchanged.

**NOT:** no drag-and-drop UI (buttons suffice until Chunk 9), no undo toast yet, no playlist.

### Chunk 5 — Logs + retry semantics + duplicate guard

**Files:** `src-tauri/src/persistence.rs` (log ring buffer + trim), `src-tauri/src/ipc.rs` (`get_item_log`, `log_line` event, `DUPLICATE_URL`), `src-tauri/src/engine_supervisor.rs` (stderr → item_logs).

**Requirements:**
- Every child stdout/stderr line appended to `item_logs`; trimmed to last 500 lines/item on insert. `get_item_log{id, tail}` returns the tail. `log_line` event emitted only while a detail drawer is open for that id (frontend subscribes/unsubscribes via a `watch_log{id, on}` command — add it to ipc and ARCHITECTURE on implementation).
- Retry of an errored item with partial bytes resumes ≥ pre-failure bytes (V3-AC3).
- `add_download` with a URL already in the queue (any non-`completed`/`cancelled` stage) returns `DUPLICATE_URL`; a `force:true` flag re-submits (PRD §7).

**AC:**
1. A failed item's full yt-dlp stderr is retrievable via `get_item_log` (K3-AC6 backend half).
2. Retry on a partially-downloaded errored item reports downloaded-bytes ≥ pre-failure (V3-AC3).
3. Adding a duplicate URL errors with `DUPLICATE_URL`; adding with `force` succeeds.
4. An item with 2000 log lines stores ≤500.

### 🚦 DEMO GATE 2 — queue control

Walk: 3 URLs, N=2 → third queues → pause item 1 (percent freezes) → resume (continues) → cancel item 2 (slot frees, item 3 starts, partial file deleted) → set N=1 mid-flight (nothing killed; new starts respect 1) → `kill -9` → relaunch → queue intact, active items resume. Observe every stage transition in the UI without refresh.

---

## Milestone 3 — Formats & presets (the differentiator)

### Chunk 6 — Probe + Add overlay (S3)

**Files:** `src-tauri/src/ipc.rs` (`probe_formats`), `src-tauri/src/engine_supervisor.rs` (probe = `yt-dlp -F`/`-J` run), `src/lib/views/AddDownload.svelte` (S3 as shadcn Dialog), `src/lib/components/FormatQuickPicks.svelte`.

**Requirements:**
- `probe_formats{url, proxy}` implements PRD's `-F` semantics via `yt-dlp -J` (structured JSON dump — same probe, parseable output), maps to `{title, formats: Format[]}`; failure returns `PROBE_FAILED` with stderr verbatim.
- S3 per UX: centered sheet (shadcn Dialog) over S2; URL field auto-focused; Probe button; on success the format region unfolds — quick picks derived from probed formats + the always-visible raw expression field as one selectable group (pick fills expression; editing expression deselects picks). Preset dropdown (reads `list_presets`, defaults to global default) + "Manage presets…" link. Advanced disclosure: output template, proxy override, extra args. Add enabled once URL non-empty (blind add allowed).
- Probe failure: format region replaced by inline error with stderr verbatim + Retry; Advanced + blind Add still work.
- Probing state: skeleton rows, Add disabled until probe resolves or user dismisses.

**AC:**
1. Probe on a valid URL shows ≥1 quick pick with resolution/size within the format region (K3-AC1) and the expression field visible without further disclosure (K3-AC2).
2. Typing an expression and adding records exactly that string on the item (K3-AC3 — verify via `get_item`).
3. Probe on a non-media URL shows yt-dlp stderr text, not a generic error (K3-AC5).
4. `Esc` closes S3; `Ctrl/Cmd+N` opens it (NFR-5 part).

**NOT:** no full format table (Chunk 7), no playlist expansion (Chunk 11).

### Chunk 7 — Format Picker (S4)

**Files:** `src/lib/views/FormatPicker.svelte`, `src/lib/components/VirtualList.svelte` (shared virtualizer — also used by S2 in Chunk 9).

**Requirements:**
- S4 as a modal Dialog opened from S3's "Format Picker" button: sortable table (id, res, ext, fps, size, codec, note), filter chips (video-only / audio-only / free-merge) + text filter, virtualized rows, best row pre-highlighted.
- Selecting a video row + an audio row composes `<vid>+<aid>` into the expression field live; "Use format" writes back to S3 and closes; expression edits in S4 flow back to S3.
- Empty state: "No formats returned — the URL may require auth or is not a media page."

**AC:**
1. Selecting video 137 + audio 140 composes `137+140` (K3-AC4).
2. A probe result with 100+ formats scrolls smoothly; DOM rows ≈ visible + buffer (virtualization).
3. "Use format" puts the composed expression into S3 and deselects quick picks (Flow B step 3).

### Chunk 8 — Presets (S6 + service)

**Files:** `src-tauri/src/preset_service.rs`, `src-tauri/src/ipc.rs` (preset commands), `src/lib/views/Presets.svelte`, `src/lib/stores/presets.svelte.ts`.

**Requirements:**
- Commands per ARCHITECTURE §7: `list_presets`, `create_preset`/`update_preset` (dry-parse `format_expr` via a no-download yt-dlp invocation; invalid → `INVALID_FORMAT_EXPR` + stderr), `delete_preset` (`LAST_PRESET` guard; deleting the default promotes the next), `set_default_preset` (single-default invariant + partial unique index).
- S6 per UX: list with default starred + sorted first, row overflow (duplicate / set-default / delete), inline editor (name, default toggle, expression, template, proxy, extra args), Save/Delete. Delete confirms.
- S3 preset dropdown applies the chosen preset's fields (all overridable; override wins on Add).

**AC:**
1. Create "4K" with `bv*[height<=2160]+ba/b`, relaunch, reopen S6 → present with that expression (K4-AC1, K4-AC5).
2. Marking a preset default un-stars the previous (K4-AC2); DB rejects two `is_default=1` rows.
3. Applying a preset in S3 fills the expression (K4-AC3); editing after apply records the edit, not the preset value (K4-AC4).
4. Delete on the only preset is blocked with an explanation (K4-AC6).
5. Saving a preset with garbage expression blocks with stderr inline (PRD §6).

### 🚦 DEMO GATE 3 — Flows B and C

Walk Flow B verbatim (probe → picker → filter video-only → select 248+140 → Use format → Add → item records `248+140`) and Flow C verbatim (create 4K preset with extra args → apply in S3 → download runs under it — verify via S5-precursor `get_item` that args/expression match). Also: save-blocked-on-invalid-expression observed.

---

## Milestone 4 — Real shell & detail (styling per DESIGN.md)

### Chunk 9 — Shell: sidebar + queue view (S2 proper)

**Files:** `src/lib/views/Shell.svelte`, `src/lib/components/Sidebar.svelte`, `src/lib/components/QueueToolbar.svelte`, `src/lib/components/QueueRow.svelte`, `src/lib/components/StageToken.svelte`, `src/lib/components/SelectionBar.svelte`, `src/lib/stores/filters.svelte.ts`, `src/routes/+page.svelte` (rewire).

**Requirements:**
- Sidebar per UX/DESIGN: fixed 240px, collapses to ~56px icon rail (width threshold + user toggle; labels → tooltips, counts → badges); **+ Add** CTA top; status filter tree (All/Downloading/Queued/Paused/Completed/Failed) with live count badges driven by `stage_changed`; Presets/Settings pinned bottom. Active filter marked by weight + indicator, not color alone. `cancelled` items appear under **All** only.
- Toolbar: title search, inline N control, Start all / Pause all.
- Queue rows per UX S2: checkbox, truncating title + tooltip, size, inline progress (pill bar + % + speed + stage token), ETA, row-level retry on error. Stage token = `label-mono` text + icon, never color alone; UI labels equal stage names: `downloading / merging / queued / paused / completed / error / cancelled`. Uses the Chunk-7 VirtualList above 50 rows.
- Selection bar on ≥1 selected: Start/Pause/Cancel/Remove/Move ▲▼ → `bulk_action`. Drag-reorder with movement threshold (plain click opens detail, V4-AC3).
- Empty states per PRD §8 (no downloads / empty filter with "Show all").
- Destructive actions (cancel/remove) confirm + undo toast (soft-delete for toast lifetime per ARCHITECTURE §8).
- Keyboard: arrow keys move row focus, Enter opens detail (NFR-5).

**AC:**
1. 60-item queue scrolls with DOM rows ≈ visible + buffer, no render stall (K2-AC10, NFR-3).
2. Bulk Pause on two selected rows flips both to `paused` (V4-AC1); bulk Remove survives restart (V4-AC2).
3. Filter counts update live as stages change; clicking a filter narrows the list in place.
4. Every text/bg pair in the new components meets WCAG AA; focus ring visible on every control (NFR-4) — spot-check with a contrast tool.
5. While 2 items download, search + sidebar + Add stay lag-free (NFR-2).

### Chunk 10 — Detail drawer (S5)

**Files:** `src/lib/views/DetailDrawer.svelte`, `src/lib/components/FactsGrid.svelte`, `src/lib/components/LogDisclosure.svelte`, `src-tauri/src/ipc.rs` (`open_path`).

**Requirements:**
- Right-side drawer (shadcn Sheet) over S2 opened by row click/Enter: header (title + live pill bar), facts grid (address+copy, output path+open dir, status, size/downloaded, speed/ETA, resume capability, format · preset) in `label-mono` where DESIGN.md says, collapsed Log disclosure tailing `get_item_log` + live `log_line`, contextual actions (Pause/Resume toggle, Retry only when errored/cancelled, Cancel/Remove confirmed + undo toast; completed → Open file / Open folder / Remove via `open_path`).
- Error state: reason above the log, log auto-expands to failing tail, Retry emphasized (Flow D).
- `Esc` closes topmost overlay only (S5 before S3).

**AC:**
1. Errored item's drawer shows stderr in the log (K3-AC6, K5-AC4).
2. Proxy set globally → item's S5 reflects it; per-item override → that item shows the override, others the global (K5-AC1, K5-AC2).
3. Completed item offers Open file / Open folder and they open the real path.
4. Flow D passes end-to-end (error → read log → fix proxy in S7 → Retry → resumes with partial bytes).

*(Chunk 10 AC 2/4 depend on the S7 proxy field — land Chunk 11 before walking Demo Gate 4.)*

### Chunk 11 — Onboarding complete (S1) + Settings (S7) + binary health

**Files:** `src-tauri/src/binary_manager.rs` (in-app download via reqwest, `binary_download` event, health re-check before spawn, `binary_health` event), `src/lib/views/Onboarding.svelte` (full S1), `src/lib/views/Settings.svelte` (S7), `src/lib/components/GlobalBanner.svelte`.

**Requirements:**
- S1 full per UX: per-binary rows with live tokens (found/not found/downloading/failed), "Download for me" (fetch official yt-dlp release binary / static ffmpeg build for the current OS into app-data `bin/`, determinate progress via `binary_download`, cancel, inline failure + Retry + stderr disclosure) or "Set path…"; proxy field; Continue gated on all binaries resolved; **I'll set it later** → S2 degraded read-only (Add disabled with explanation).
- S7 per UX: Engine & health (paths, versions, Re-check, Re-run onboarding → reopens S1 with current state), Downloads (N, default dir + native picker, template, default preset), Network (global proxy), About (build flavor + app/yt-dlp versions). `Ctrl/Cmd+,` opens it.
- Mid-session health: spawn-time check per ARCHITECTURE §8 — missing binary emits `binary_health` → GlobalBanner ("yt-dlp is no longer at its path — downloads are paused") + Fix (reopens S1); active items paused; no download starts (K1-AC7).

**AC:**
1. K1-AC1…AC5 and AC7 pass verbatim on the light build.
2. In-app download failure shows inline error + Retry on that row while the other binary remains resolvable (PRD §7).
3. Playlist NOT here — S7 shows persisted proxy round-trip (K1-AC4).

### 🚦 DEMO GATE 4 — full kernel journey + Flow D

Walk SPEC's kernel journey **verbatim**, now with the real UI: light build, no ffmpeg → "Download for me" completes in-app → proxy → probe → pick 1080p via picker *or* type expression → apply preset → parallel 2 → quit mid-download → relaunch → resume → complete at templated path. Then Flow D verbatim. Observe stage tokens, counts, undo toasts, and keyboard-only operation of the whole journey (NFR-5 script).

---

## Milestone 5 — v1 completion

### Chunk 12 — Playlist expansion + remaining edges

**Files:** `src-tauri/src/queue_manager.rs` (playlist expand via `yt-dlp --flat-playlist -J`), `src-tauri/src/ipc.rs`, `src/lib/views/AddDownload.svelte` (playlist hint).

**Requirements:**
- `add_download` with a playlist URL expands to one row per entry (shared `playlist_id`), each independently controllable (V2). Mixed availability: unavailable entries become individual `error` rows with their own stderr (PRD §7).
- Remaining PRD §7/§8 rows verified and patched where missing: disk full → item `error` others unaffected; offline at launch → app opens, queue browsable, network errors on attempt.

**AC:**
1. Playlist of M entries yields M rows (K2-AC3); cancelling one leaves the others running (V2-AC2).
2. A playlist with one dead entry: live entries complete, the dead one is `error` with its own stderr.

### Chunk 13 — Build flavors + packaging

**Files:** `src-tauri/tauri.conf.json` (+ per-flavor config), `src-tauri/build.rs` or cargo feature `bundled`, `src-tauri/src/binary_manager.rs` (bundled seeding), CI/packaging config (`.github/workflows/build.yml` if CI wanted — else local `tauri build` targets AppImage/deb/msi).

**Requirements:**
- `bundled` compile-time flavor: ships yt-dlp + ffmpeg as Tauri sidecars, seeds `ytdlp_path`/`ffmpeg_path` on first run, skips S1 detection (single confirming line per UX S1 density note); `light` = current behavior. Flavor mirrored to `settings.build_flavor`, shown in S7 About (V5).
- Cross-platform paths verified on Linux + Windows (NFR-6): binary discovery, app-data DB, OS Downloads default, templated output.
- Builds: Linux AppImage + deb, Windows installer, for both flavors.

**AC:**
1. Bundled build on a machine with no system yt-dlp/ffmpeg completes a download (V5-AC2) and never shows the S1 wizard (K1-AC6).
2. S7 About matches the installed flavor (V5-AC1).
3. All four artifacts (linux/windows × bundled/light) build; light on Windows resolves `where yt-dlp` correctly.

### 🚦 DEMO GATE 5 — v1 exit

Both flavors, both OSes where hardware allows: kernel journey + one playlist + duplicate-URL warn + delete-default-preset promotion + `kill -9` durability + keyboard-only pass + WCAG AA spot-check. This gate is the 001-core exit bar; anything failing is fixed before the cycle closes.

---

## Cross-cutting rules (every chunk)

- **Boundary law** (ARCHITECTURE §2): frontend never computes durable truth; `src/lib/ipc.ts` is the only `@tauri-apps/api` import.
- **Styling**: tokens via DESIGN.md mapping only — no raw hex/px in components.
- **Tests**: per CONVENTIONS.md — Rust unit tests for parser/scheduler/invariants; one integration test per chunk touching the engine (may be `#[ignore]`-gated on network).
- **Docs are living**: any deviation discovered mid-chunk patches ARCHITECTURE.md/UX.md first, then code (e.g. the `watch_log` command in Chunk 5).
