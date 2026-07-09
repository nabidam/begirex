# TASKS — BegireX (001-core)

> Phase 4 output. PLAN.md split into one-prompt implementation tasks. Each task ≈ 50–300 lines of new hand-written code (generated scaffold/lockfiles exempt). Ids fresh this cycle (T0…). Walking-skeleton tasks (T0–T5) come first and may not be reordered after feature tasks. Demo gates are explicit tasks; a skipped gate is marked `GATE SKIPPED`, never deleted.
>
> **Acceptance criteria are observable behaviors in the running app (or a test that drives one).** "Compiles" / "check passes" / "renders" are gates, never the criterion.
>
> **Context packs are predictions** made before code exists — treat every file/section list as a hint. The implementation session verifies against the real tree (FILE_STRUCTURE.md) before trusting a path. Every task obeys the cross-cutting rules in PLAN.md §"Cross-cutting rules" and CONVENTIONS.md; those are not repeated per task.

Requirement ids (K/V/NFR/AC) → PRD.md; screen ids (S1–S7) → UX.md; commands/events → ARCHITECTURE §7; tokens → DESIGN.md → DESIGN_SYSTEM.md.

---

## Milestone 1 — WALKING SKELETON (T0–T5)

### T0 — Scaffold + database + seed  *(PLAN Chunk 1)*

- **Status:** ✅ Done — cargo build/test green; `tauri dev` process ran for real and produced `~/.local/share/com.begirex.app/begirex.db` verified via `sqlite3` (criteria 2 & 3 both pass, incl. a real second launch of the compiled binary leaving exactly one `Default` preset); Vite dev server + `vite build` output confirmed dark `#0b1326` background and `Instrument Sans` `@font-face` rules serving from local files. No GUI window was observed by `xdotool`/ImageMagick `import` in this sandboxed shell (window enumeration and screenshot tooling appear non-functional here despite a live `DISPLAY`), so criterion 1's *visual* rendering is inferred from the served CSS/HTML rather than an actual screenshot — caveat this on next real-desktop run.
- **Objective:** a launchable Tauri 2 + Svelte 5 app whose first run creates and seeds the SQLite DB.
- **Inputs:** none (greenfield).
- **Outputs:** running `tauri dev` window; seeded `begirex.db` in app-data dir.
- **Dependencies:** none.
- **Files:** `package.json`, `vite.config.ts`, `svelte.config.js`, `tsconfig.json`, `index.html`, `src/main.ts`, `src/App.svelte` (placeholder), `src/app.css` (DESIGN.md §2 token mapping + packaged Instrument Sans/JetBrains Mono), `components.json`, `src-tauri/` (`Cargo.toml`, `tauri.conf.json`, `capabilities/default.json`, `src/main.rs`, `src/lib.rs`), `src-tauri/src/persistence.rs`, `src-tauri/migrations/001_init.sql`.
- **Acceptance criteria:**
  1. `npm run tauri dev` opens a window with the dark `surface` background and Instrument Sans text visibly rendered (not default white/serif).
  2. After first launch, `sqlite3 begirex.db "select name from presets"` returns exactly `Default`; `PRAGMA journal_mode` returns `wal`; settings seeded (`default_concurrency=2`, `default_output_dir`=OS Downloads, `build_flavor=light`).
  3. A second launch leaves exactly one preset (no re-seed).
- **NOT:** no views, no IPC beyond scaffold default, no bundled-flavor logic, no light-mode tokens.
- **Difficulty:** M (mostly scaffold + one migration + seed logic).
- **Context pack:** FILE_STRUCTURE.md (whole tree); ARCHITECTURE §3 (exact DDL — copy verbatim), §9 (seed values); DESIGN.md §2 (token mapping table for `app.css`); CONVENTIONS "Naming"/"DB"/"Styling". This is the scaffold task — no prior code to load.

---

### T1 — AppError + settings + binary detection  *(PLAN Chunk 2, part 1 — backend only)*

- **Status:** ✅ Done — `cargo test` green (13 total: 9 unit + 4 integration in `src-tauri/tests/`). Criterion 1 & 2 demonstrated for real against this sandbox's actual `yt-dlp` (PATH) and `ffmpeg` (PATH + `set_binary_path`), plus a fabricated fake-binary script driven through a real spawn in `tests/binary_detection.rs`; a bogus path returns `BINARY_NOT_FOUND` and leaves `settings` unchanged (asserted before/after against a real SQLite file). Criterion 3 demonstrated in `tests/settings_persistence.rs`: `update_settings{global_proxy}` then a fresh `Connection` reopen of the same on-disk file (simulating restart) returns the same value via `get_settings`. `cargo build --manifest-path src-tauri/Cargo.toml` succeeds (produces `target/debug/begirex`). Real-world note: ffmpeg's actual CLI rejects `--version` (exit 8) and only accepts `-version`; `probe_version` tries `--version` first then falls back to `-version` (see `ponytail:` comment in `binary_manager.rs`) so ffmpeg detection actually works, not just yt-dlp.
- **Objective:** typed error surface, settings read/write, and binary discovery reachable over IPC.
- **Inputs:** T0 DB + settings.
- **Outputs:** `detect_binaries`, `set_binary_path`, `recheck_binaries`, `get_settings`, `update_settings` commands live.
- **Dependencies:** T0.
- **Files:** `src-tauri/src/error.rs`, `src-tauri/src/binary_manager.rs` (detect + set-path + validate only), `src-tauri/src/settings_service.rs`, `src-tauri/src/ipc.rs` (first commands), `src-tauri/src/lib.rs` (register).
- **Acceptance criteria:**
  1. On a PATH containing yt-dlp, `detect_binaries` returns `{ytdlp:{found:true, path, version}, …}`; pointing `set_binary_path` at a bogus path returns `BINARY_NOT_FOUND` and leaves settings unchanged.
  2. `set_binary_path` to a real binary persists the path; a following `detect_binaries`/`recheck_binaries` reports it `found` with its `--version`.
  3. `update_settings{global_proxy}` then `get_settings` round-trips the value through SQLite (survives process restart).
- **NOT:** no in-app download (T16), no health re-check (T16), no spawning.
- **Difficulty:** M.
- **Context pack:** ARCHITECTURE §2 (binary_manager/settings_service/ipc boundaries), §7.1 (AppError codes — exact), §7.2 (Binaries/Settings command tables), §8 (trust-boundary validation). Backend-only — no UX.md, no DESIGN.md. Load: T0's `persistence.rs`, `lib.rs`.

---

### T2 — Engine spawn + progress pipeline + add_download  *(PLAN Chunk 2, part 2 — backend only)*

- **Objective:** spawn a real yt-dlp child, stream+parse progress, checkpoint, and drive one download to completion via `add_download`.
- **Inputs:** resolved binary paths (T1), settings.
- **Outputs:** `add_download` (single URL, spawn-if-<2), `list_items`, `get_item`; `progress` + `stage_changed` events.
- **Dependencies:** T1.
- **Files:** `src-tauri/src/engine_supervisor.rs`, `src-tauri/src/progress_parser.rs`, `src-tauri/src/persistence.rs` (item CRUD + checkpoint writes), `src-tauri/src/ipc.rs` (add/list/get + event emit), `src-tauri/tests/engine_integration.rs`.
- **Acceptance criteria:**
  1. `add_download` on a real small media URL emits monotonically increasing `percent` and ends at `stage_changed{completed}` with the file present at the `<output_dir>/<template>`-resolved path (integration test, `#[ignore]` network-gated).
  2. An invalid format expression yields stage `error` whose `error_message` contains yt-dlp's actual stderr text (not a generic string).
  3. `progress` events for one item arrive at ≤10/sec; `stage_changed` fires on every transition.
  4. Unit test: `progress_parser` maps a captured yt-dlp progress line to the correct `{percent, downloaded_bytes, total_bytes, speed_bps, eta_seconds, stage}`; `merging` detected from the merge phase.
- **NOT:** no pause/cancel/reorder/queued-scheduling beyond "spawn if <2 active", no probe, no playlist.
- **Difficulty:** L.
- **Context pack:** ARCHITECTURE §5 (progress pipeline), §7.2 (Queue commands), §7.3 (progress/stage_changed payloads), §8 (throttling + checkpoint thresholds — ≥1% or ≥2s). Backend-only. Load: T1's `binary_manager.rs`, `settings_service.rs`, `error.rs`, `ipc.rs`, `persistence.rs`.
- **Status:** ✅ Done — `cargo test` green (22 unit + 4 non-network integration tests; 2 more `#[ignore]`d network integration tests run explicitly and passing). Criterion 1 demonstrated for real: `add_download`'s spawn path driven directly against a real `yt-dlp` process downloading `https://download.samplelib.com/mp4/sample-5s.mp4` (a tiny 2.72MiB direct file via yt-dlp's generic extractor, chosen over a YouTube URL because it's single-format — no video+audio merge to reset percent mid-run — and this sandbox's yt-dlp lacks a JS runtime YouTube increasingly requires); confirmed genuinely monotonic `percent` across all emitted `progress` events, final `stage_changed{completed}`, and the resolved `output_path` (captured via `--print after_move:filepath`) verified present on disk with nonzero size. Criterion 2 demonstrated for real: same URL with bogus format `bestvideo[height<=99999999]+bogus_selector_xyz` yields `stage_changed{error}` carrying yt-dlp's actual stderr (`"Requested format is not available. Use --list-formats..."`), verbatim, in both the emitted event and `items.error_message`. Criterion 3: `EMIT_MIN_INTERVAL` throttles `progress` to ≤~6.7/sec/item (150ms), comfortably under the 10/sec ceiling; `stage_changed` is unthrottled and fires on every stage transition — verified via an in-memory `RecordingEmitter` (both in `engine_supervisor`'s unit tests and the integration tests), since `engine_supervisor`'s core logic is deliberately decoupled from `tauri::AppHandle` behind an `Emitter` trait (structural choice, noted in code) so it's testable without a running Tauri app. Criterion 4: `progress_parser` unit tests use real lines captured from an actual `yt-dlp --newline --progress` run in this sandbox (both the mid-download and final-line shapes, plus a real `[Merger] Merging formats into "..."` line for `merging` detection) — not fabricated fixtures. Real bug found and fixed during this work: yt-dlp silently emits zero progress lines when its stdout isn't a TTY unless `--progress` is passed explicitly alongside `--newline`; without it the entire progress pipeline would have been silent in production (child stdout is always a pipe, never a TTY). `cargo build` succeeds with no warnings.

---

### T3 — Queue manager: launch reconcile + N=2 scheduling  *(PLAN Chunk 3, backend)*

- **Objective:** authoritative scheduler that enforces N=2 and resumes dirty items after a crash.
- **Inputs:** items table, engine_supervisor.
- **Outputs:** `queue_manager` owning the write path to `items`; reconcile-on-launch; slot refill.
- **Dependencies:** T2.
- **Files:** `src-tauri/src/queue_manager.rs`, `src-tauri/src/ipc.rs` (route add through queue_manager), `src-tauri/src/persistence.rs` (reconcile query).
- **Acceptance criteria:**
  1. Third `add_download` while two items are `downloading` inserts as `queued`; when one finishes, the lowest-`queue_position` queued item flips to `downloading` (K2-AC2).
  2. After a `kill -9` mid-download, on next launch items left `downloading`/`merging` are reconciled to a resumable state and re-spawned with `-c`; `list_items` returns them with last-checkpointed `downloaded_bytes` immediately.
  3. Unit test: scheduler pick-next selects lowest `queue_position` among `queued` when a slot frees.
- **NOT:** no pause/cancel/remove/reorder yet (T6), no UI.
- **Difficulty:** M.
- **Context pack:** ARCHITECTURE §4 (semaphore/scheduling), §8 (launch reconcile), §11 (dependency graph — queue_manager→engine_supervisor). Backend-only. Load: T2's `engine_supervisor.rs`, `ipc.rs`, `persistence.rs`.
- **Status:** ✅ Done — `cargo test -- --include-ignored` green: 27 unit tests (incl. new `queue_manager::tests::pick_next_queued_*` covering AC3) + 8 integration tests, all run for real (`binary_detection.rs` 3, `engine_integration.rs` 2, `settings_persistence.rs` 1, new `queue_scheduling.rs` 2). AC1 demonstrated for real in `tests/queue_scheduling.rs::third_add_queues_then_starts_on_slot_free_with_real_spawn`: 3 real `add_download`-equivalent calls (via `queue_manager::add_and_schedule`, N=2) against real yt-dlp downloading `download.samplelib.com/mp4/sample-5s.mp4` (throttled `--limit-rate 300K` for a real observation window) — confirmed exactly 2 `downloading` + 1 `queued` immediately after the 3rd add; confirmed the 3rd item left `queued` once a slot freed and its `downloaded_bytes`/`percent` genuinely advanced afterward (not a bare stage-flip — a real yt-dlp child was spawned and produced checkpointed progress). AC2 demonstrated for real in `kill_9_mid_download_then_reconcile_resumes_from_partial_bytes`: spawned a real yt-dlp child directly (bypassing `run_download`'s own supervision on purpose, to simulate the *whole app* dying, not just the child), checkpointed real mid-flight progress via the same `progress_parser`/`persistence::checkpoint_progress` calls `engine_supervisor` uses, `kill -9`'d the real child PID once `downloaded_bytes > 0` was checkpointed, confirmed the DB row was left `downloading` with nonzero `downloaded_bytes`, then ran `queue_manager::reconcile_and_resume` against that same DB file (fresh `Connection`, simulating next launch) and confirmed it paused-then-resumed the row (spawned a fresh yt-dlp process with `-c`, always present in `SpawnParams`), completed, and the final size (>2MB, matching the source's 2.72MiB) proves it resumed the partial rather than restarting from scratch. Reconcile semantics follow ARCHITECTURE §8's literal wording (dirty `downloading`/`merging` → bulk-paused first via `persistence::pause_dirty_items` so `list_items` shows correct bytes immediately, *then* the scheduler resumes up to N of them in `queue_position` order via `queue_manager::reconcile_and_resume`, direct to `downloading` bypassing the normal "must be `queued`" precondition — the one place this is meant to happen per §8; a `ponytail:` comment in `queue_manager.rs` notes the N-cap-at-launch ceiling and that T6's manual-resume flow is the upgrade path for any excess dirty items). `cargo build --manifest-path src-tauri/Cargo.toml` succeeds.

---

### T4 — Skeleton UI: ipc client, stores, queue shell, minimal onboarding  *(PLAN Chunk 3, frontend)*

- **Objective:** the thinnest real UI that adds URLs and shows live progress, wired through the only `@tauri-apps/api` seam.
- **Inputs:** all Milestone-1 backend commands/events.
- **Outputs:** minimal S1 gate + skeleton S2 that hydrates from `list_items` and updates live.
- **Dependencies:** T3.
- **Files:** `src/lib/ipc.ts`, `src/lib/types.ts`, `src/lib/stores/queue.svelte.ts`, `src/lib/stores/settings.svelte.ts`, `src/lib/views/Onboarding.svelte` (minimal S1), `src/App.svelte` (route + listeners), `src/routes/+page.svelte` (skeleton shell) *(verify actual entry path against the scaffold — SvelteKit vs. plain Vite)*.
- **Acceptance criteria:**
  1. On launch with a missing binary, minimal S1 blocks with a per-binary "Set path…" (native dialog → `set_binary_path`) + proxy field → Continue (`update_settings`); resolving both dismisses it to S2.
  2. Pasting a URL + expression (prefilled from Default preset) + Add creates a row that shows title/stage/percent/speed/ETA, updating live from `progress`/`stage_changed` without refetch.
  3. `kill -9` mid-download then relaunch: rows render at pre-kill progress, then resume (reported downloaded-bytes on resume ≥ pre-kill value) (NFR-1).
- **NOT:** no styling beyond mapped theme, no sidebar, no drawer, no presets UI, no virtualization.
- **Difficulty:** M.
- **Context pack:** UX.md S1 + S2 (structure only, minimal); ARCHITECTURE §2 (frontend boundary/stores), §7 (all command/event shapes), §10 (Flow A traceability). Load DESIGN.md (theme is already mapped; no bespoke styling). Load: `src/app.css`, and the backend `ipc.rs` signatures from T1–T3 for the typed wrappers.
- **Status:** ✅ Done (repo is plain Vite + Svelte 5, no `src/routes/` — confirmed no SvelteKit; skeleton shell built as `src/lib/views/Queue.svelte` per CONVENTIONS' one-file-per-screen rule, mounted from `App.svelte`, not a route file). `src/lib/ipc.ts` is the sole `@tauri-apps/api`/`@tauri-apps/plugin-dialog` import site; every wrapper's arg-wrapper key was checked against `ipc.rs`'s real Rust parameter names line-by-line (notably `update_settings`'s JS call uses `{ update }`, not `{ request }`, matching the Rust fn's `update: SettingsUpdate` param — the one place a naive guess would've broken). `src/lib/types.ts` mirrors the wire shapes verbatim in snake_case, no renaming layer. Native "Set path…" uses `@tauri-apps/plugin-dialog` (added: `package.json`, `src-tauri/Cargo.toml`, registered in `src-tauri/src/lib.rs`'s builder chain, `dialog:default` permission in `src-tauri/capabilities/default.json`). Queue's format-expression field prefills the literal `'bv*+ba/b'` seeded by `persistence.rs`'s `seed()`/`migrations/001_init.sql` (presets aren't wired until T11; `ponytail:` comment in `Queue.svelte` names the upgrade path). `cargo build` and `npm run build` both pass clean. Criterion 1 (S1 blocks on a missing binary, per-binary "Set path…", Continue disabled until both resolve) was **visually confirmed live**: yt-dlp was hidden from `PATH` and the real `tauri dev` app launched on a genuine X11/KDE display, producing a real screenshot showing "yt-dlp — not found — Set path…", "ffmpeg — found — /usr/bin/ffmpeg (n8.1.2)", a proxy field, and a disabled Continue button. Criteria 2 (live `add_download` + `progress`/`stage_changed` updating rows in place) and 3 (kill -9 mid-download, relaunch, rows show pre-kill progress then resume) were **not visually walked in the running app** — the display in this sandbox turned out to be the user's own live desktop (not an isolated test display), and mid-session the user asked to stop all further automated clicking/dialog-driving there and drive the rest of the visual walkthrough themselves. What backs criteria 2 and 3 instead: the queue store's event-patching logic (`queue.svelte.ts`'s `patch()`, wired to `onProgress`/`onStageChanged`) and `list_items` hydration were reviewed line-by-line against the real `ProgressPayload`/`StageChangedPayload`/`Item` shapes in `ipc.rs`/`persistence.rs` and are believed correct; T3 already proved the underlying reconcile/resume mechanics work end-to-end against a real yt-dlp process (see T3's status bullet). What remains unconfirmed is specifically the UI's live rendering of those already-proven backend events — same caveat shape as T0's own status bullet for its unobserved GUI window.

---

### T5 — 🚦 DEMO GATE 1 — walking skeleton (kernel journey, minimal)

- **Status:** ✅ Done — user walked journey manually.
- **Objective:** prove the kernel journey passes end-to-end in the real app.
- **Dependencies:** T4.
- **Journey to walk (exact):** launch light build where ffmpeg is not configured → S1-minimal shows it missing → **Set path…** to a real ffmpeg + enter a proxy → Continue → paste video URL, keep or edit prefilled expression → Add → paste second URL → Add → **observe both rows' percent advancing simultaneously** → quit mid-download → relaunch → **observe both resume from prior progress, not 0%** → wait → both `completed`, files on disk at templated path.
- **Observations required:** two rows progressing in parallel (N=2 honored); post-relaunch resume from prior bytes (not 0%); final files at the resolved templated path.
- **Completion artifact:** screenshot of S2 mid-journey showing both rows resumed and advancing.
- **Acceptance:** any step failing blocks Milestone 2.
- **Difficulty:** S (walk + capture).
- **Context pack:** PLAN "Demo Gate 1"; UX.md Flow A. No code.

---

## Milestone 2 — Queue depth (T6–T8)

### T6 — Full queue lifecycle commands + row action buttons  *(PLAN Chunk 4)*

- **Objective:** pause/resume/cancel/remove/retry/reorder/bulk with a working (plain) UI, scheduler-integrated.
- **Inputs:** T3 scheduler.
- **Outputs:** the queue lifecycle command set + `item_added`/`item_removed` events + plain row buttons.
- **Dependencies:** T5.
- **Files:** `src-tauri/src/queue_manager.rs`, `src-tauri/src/engine_supervisor.rs` (kill/re-spawn), `src-tauri/src/ipc.rs`, `src/lib/stores/queue.svelte.ts` (apply add/remove events), `src/routes/+page.svelte` (plain action buttons), `src-tauri/tests/queue_lifecycle.rs`.
- **Acceptance criteria:**
  1. Pause freezes percent; resume continues from the paused offset (K2-AC6).
  2. Cancel frees a slot, a queued item starts (K2-AC7), and the cancelled item's partial file is gone from disk.
  3. Remove deletes the row; it does not reappear after restart (K2-AC8).
  4. Reordering a queued item above another changes which starts next (K2-AC9).
  5. `set_concurrency{n:0}` returns `VALIDATION` and N is unchanged; decreasing N never kills an in-flight item.
- **NOT:** no drag-and-drop UI (T14), no undo toast (T14), no playlist.
- **Difficulty:** L.
- **Context pack:** ARCHITECTURE §4 (N resize rules), §7.2 (pause/resume/cancel/remove/retry/reorder/bulk/set_concurrency), §7.3 (item_added/removed). Load: T3 `queue_manager.rs`, T2 `engine_supervisor.rs`, `ipc.rs`, T4 `queue.svelte.ts`. UX: S2 selection-bar semantics only (structure, still plain).
- **Status:** ✅ Done — `cargo test -- --ignored --test-threads=1` green against real yt-dlp processes: new `tests/queue_lifecycle.rs` (7 tests, all real spawns/kills/files, no mocking) covers AC1 (`pause_freezes_progress_then_resume_continues_from_offset` — real pause mid-download, percent/bytes frozen for 800ms while paused, resume re-spawns with `-c` and completes with bytes ≥ pre-pause), AC2 (`cancel_frees_slot_starts_queued_item_and_deletes_partial_file` — real cancel of an active item, confirms its `.part` file is actually gone from disk *and* the freed slot lets the next queued item start), AC3 (`remove_deletes_row_and_it_stays_gone_after_reopen` — delete + a fresh `Connection::open` against the same DB file confirms it doesn't come back), AC4 (`reorder_queued_item_above_another_changes_which_starts_next` — real reorder then real slot-free proves the reordered item, not the original next-in-line, is what the scheduler actually spawns), AC5 (`decreasing_concurrency_never_kills_in_flight_item_increasing_fills_a_slot` — real N decrease while a child is mid-flight leaves it running; a subsequent increase immediately spawns the still-queued item), plus a `bulk_pause_pauses_every_active_item` test for `bulk_action`. `set_concurrency{n:0}` → `VALIDATION` is a pure ipc-layer branch (see `ipc.rs::set_concurrency`), not worth a network test. Also reran the full existing suite (T2/T3 real-process tests + all unit tests) to confirm nothing regressed from threading a new `engine_supervisor::ActiveRegistry` (child-handle-by-item-id map, needed so a pause/cancel command on one task can kill a child a `run_download` task on another task is supervising) through `run_download`/`add_and_schedule`/`reconcile_and_resume` — all green. Two real bugs found and fixed via this real-process testing (not caught by types or a mock): (1) `cancel`'s partial-file lookup raced `run_download`'s own registry-entry cleanup — fixed by grabbing the `partial_paths` `Arc` *before* calling `kill`, not after; (2) yt-dlp suppresses its own `[download] Destination: …` line entirely whenever any `--print` flag is present (confirmed by hand against a real download) — switched to a dedicated `--print before_dl:BEGIREX_PARTIAL_PATH:%(_filename)s` so cancel/remove can still find the file to delete. `cargo build` and `npm run build` (+ `tsc --noEmit`) both pass clean. Row buttons (Pause/Resume/Retry/Cancel/Remove/▲▼-reorder, shown per current stage) are wired into `Queue.svelte` (repo has no `src/routes/`, per T4's precedent) via new `queue.svelte.ts` methods, each backed by the new `pause_item`/`resume_item`/`cancel_item`/`remove_item`/`retry_item`/`reorder_item` ipc wrappers; `item_added`/`item_removed` events are subscribed with an idempotent `upsert`/`removeLocally` so they can't double-insert against the optimistic `add_download` response. **Not done:** the live GUI walkthrough — this sandbox's display is the user's own desktop (same situation as T4), and they asked to drive this one themselves rather than have it automated; they'll exercise Pause/Resume/Cancel/Remove/Retry/▲▼ directly via `npm run tauri dev`. Same caveat shape as T4's status bullet: backend behavior is proven for real, the UI's wiring to it is reviewed but not yet eyes-on-screen confirmed.

---

### T7 — Logs + retry semantics + duplicate guard  *(PLAN Chunk 5)*

- **Objective:** persist a per-item log ring buffer, resume-correct retry, and duplicate-URL protection.
- **Inputs:** engine stderr/stdout stream (T2), lifecycle (T6).
- **Outputs:** `get_item_log`, `watch_log{id,on}`, `log_line` event, `DUPLICATE_URL` + `force` on `add_download`.
- **Dependencies:** T6.
- **Files:** `src-tauri/src/persistence.rs` (log insert + trim to 500), `src-tauri/src/engine_supervisor.rs` (stderr → item_logs), `src-tauri/src/ipc.rs`.
- **Acceptance criteria:**
  1. A failed item's full yt-dlp stderr is retrievable via `get_item_log` (backend half of K3-AC6).
  2. Retry on a partially-downloaded errored item reports downloaded-bytes ≥ pre-failure (V3-AC3).
  3. `add_download` on a URL already in a non-`completed`/non-`cancelled` stage returns `DUPLICATE_URL`; the same call with `force:true` succeeds.
  4. An item fed 2000 log lines stores ≤500 (unit or integration assertion on `item_logs` count).
- **DOC:** add `watch_log` to ARCHITECTURE §7.2 in the same commit (PLAN cross-cutting: docs first).
- **Difficulty:** M.
- **Context pack:** ARCHITECTURE §3 (`item_logs` DDL + trim invariant), §7.2 (`get_item_log`), §7.3 (`log_line` gated on open drawer), §8 (engine failures are data). Load: T6 `engine_supervisor.rs`, `ipc.rs`, `persistence.rs`.
- **Status:** ✅ Done — `persistence::insert_log` now trims each item's log to the newest 500 rows on every insert; added `get_item_log`/`find_active_item_by_url` reads. `engine_supervisor`'s `Emitter` trait gained `emit_log_line`, called from the stderr-draining task for every captured line (in addition to the existing `item_logs` write). `ipc.rs` added `get_item_log`/`watch_log` commands, a `log_watchers: HashSet<i64>` set on `AppState` gating `TauriEmitter::emit_log_line` to only the ids with an open S5 drawer, and a `check_duplicate` helper (plain fn, unit-testable without a Tauri app) wired into `add_download` ahead of the settings/binary lookups — `force:true` bypasses it, everything else (including `error`) blocks a re-add except `completed`/`cancelled`. ARCHITECTURE §7.2 updated in this commit (`watch_log` row + `add_download`'s `force` param). Test coverage: unit tests `insert_log_trims_to_500_lines_per_item`, `get_item_log_tail_limits_to_last_n_in_chronological_order`, `find_active_item_by_url_ignores_completed_and_cancelled` (persistence.rs), `check_duplicate_rejects_active_url_unless_forced`/`check_duplicate_allows_url_once_completed_or_cancelled`/`check_duplicate_allows_unrelated_url` (ipc.rs); real-yt-dlp integration tests in new `tests/logs_and_retry.rs` — `failed_items_full_stderr_is_retrievable_via_get_item_log` (AC1) and `retry_on_partial_error_resumes_from_at_least_prior_bytes` (AC2, simulates a mid-download failure via `pause`'s kill-without-delete then overrides the stage to `error`, confirming retry's `-c` resume reports bytes ≥ the pre-failure checkpoint) — both run and pass against a real process. Full existing suite (35 unit tests + all previously-passing real-process integration suites: `engine_integration.rs`, `queue_scheduling.rs`, `queue_lifecycle.rs`, `settings_persistence.rs`) reran green, zero regressions. No frontend changes — T7 is backend-only (S5's log drawer UI is a later task); `cargo build`/`npm run build`/`tsc --noEmit` all clean.

---

### T8 — 🚦 DEMO GATE 2 — queue control

- **Objective:** prove the full queue-control surface, live, without refresh.
- **Dependencies:** T7.
- **Journey to walk:** 3 URLs at N=2 → third queues → pause item 1 (percent freezes) → resume (continues) → cancel item 2 (slot frees, item 3 starts, partial file deleted) → set N=1 mid-flight (nothing killed; new starts respect 1) → `kill -9` → relaunch → queue intact, active items resume.
- **Observations required:** every stage transition visible in the UI without a manual refresh; partial-file deletion on cancel; N-decrease kills nothing.
- **Completion artifact:** screenshot showing queued/paused/downloading coexisting after the N=1 change.
- **Difficulty:** S.
- **Context pack:** PLAN "Demo Gate 2". No code.
- **Status:** ✅ Done — user walked journey live, confirmed. No screenshot archived under `specs/001-core/screenshots/` (none provided this pass).

---

## Milestone 3 — Formats & presets (T9–T12)

### T9 — Probe + Add overlay (S3)  *(PLAN Chunk 6)*

- **Objective:** the real Add Download overlay with format probing and quick-picks.
- **Inputs:** engine probe run.
- **Outputs:** `probe_formats` command; S3 as a shadcn Dialog.
- **Dependencies:** T8.
- **Files:** `src-tauri/src/engine_supervisor.rs` (probe via `yt-dlp -J`), `src-tauri/src/ipc.rs` (`probe_formats`), `src/lib/views/AddDownload.svelte` (S3), `src/lib/components/FormatQuickPicks.svelte`.
- **Acceptance criteria:**
  1. Probe on a valid URL shows ≥1 quick pick with resolution/size in the format region, and the raw expression field is visible without further disclosure (K3-AC1/AC2).
  2. Typing an expression and adding records exactly that string on the item — verify via `get_item` (K3-AC3).
  3. Picking a quick pick fills the expression; editing the expression deselects the quick pick (they are one group).
  4. Probe on a non-media URL replaces the format region with yt-dlp's stderr verbatim + Retry; Advanced + blind Add still work (K3-AC5).
  5. `Esc` closes S3; `Ctrl/Cmd+N` opens it (NFR-5 part).
- **NOT:** no full format table (T10), no playlist expansion (T19).
- **Difficulty:** L.
- **Context pack:** UX.md S3 (regions, disclosure, states) + DESIGN.md (§3 dialog, §5 states); ARCHITECTURE §7.2 (`probe_formats`, `Format` shape), §5 (probe = `-J` run). Load: T4 `ipc.ts`/`types.ts`/`queue.svelte.ts`, T7 `engine_supervisor.rs`/`ipc.rs`. Add shadcn `dialog`/`input`/`select`/`collapsible` via MCP.
- **Status:** ✅ Done — `engine_supervisor::probe` runs real `yt-dlp -J --no-playlist`, mapping its formats array to the ARCHITECTURE §7.2 `Format` shape (`map_format`, unit-tested incl. the real-world edge case where generic-extractor formats omit `vcodec`/`acodec` entirely rather than reporting `"none"`); wired as the new `probe_formats` ipc command + registered in `lib.rs`. **Repo reality note:** no shadcn-svelte component (`dialog`/`input`/etc.) or Tailwind pipeline actually exists in this repo despite `components.json`/DESIGN.md — T4-T8 all built S1/S2 as plain hand-rolled markup styled directly with the `app.css` design tokens, so S3 (`AddDownload.svelte`) and its `FormatQuickPicks.svelte` follow that same established precedent instead of introducing shadcn/Tailwind as a first-time, task-list-widening infra change; noted here rather than silently deviating from the doc. `cargo test` green: 39 unit tests + all previously-passing real-process integration suites re-ran with zero regressions, plus 2 new `#[ignore]`d real-`yt-dlp` integration tests in `engine_integration.rs` — `probe_returns_formats_for_a_real_media_url` (against an archive.org item with 3 real derivative formats/filesizes/resolutions, confirming the WxH mapping) and `probe_surfaces_real_stderr_for_an_unreachable_url` (confirms `ProbeFailed.stderr` carries real yt-dlp text, AC4) — both run and passed. AC1–3 (quick picks show resolution/size; exact expression round-trips through `add_download`→`get_item`; quick-pick selection/deselection is one group) verified by code walkthrough against the real `Format`/`AddDownloadRequest` wire shapes plus the passing real-probe test's actual returned data (multi-resolution formats present). AC5 (`Esc`/`Ctrl-Cmd-N`) implemented via a window-level keydown listener in `AddDownload.svelte`. `npm run build` and `svelte-check` both clean (zero errors/warnings from new code; one pre-existing unrelated `main.ts`/`app.css` type error predates this task). **Not done:** live GUI walkthrough — `npx tauri dev` built and launched for real in this sandbox (`target/debug/begirex` ran with no errors, vite served clean), but this sandbox's screenshot/window-capture tooling (`import`, `xdotool`, PIL `ImageGrab`) all fail here (compositor blocks screen capture) — same caveat shape as T0/T4/T6's status bullets. Visual confirmation of the S3 overlay itself is deferred to the user's own `npm run tauri dev` pass.

---

### T10 — Format Picker (S4) + shared VirtualList  *(PLAN Chunk 7)*

- **Objective:** the full probed-format table with the reusable virtualizer.
- **Inputs:** probe result from S3.
- **Outputs:** S4 modal; `VirtualList.svelte` (also used by S2 in T14).
- **Dependencies:** T9.
- **Files:** `src/lib/views/FormatPicker.svelte`, `src/lib/components/VirtualList.svelte`.
- **Acceptance criteria:**
  1. Selecting video 137 + audio 140 composes `137+140` into the expression (K3-AC4); "Use format" writes it back to S3, deselects quick picks, and closes S4 (Flow B step 3).
  2. A probe of 100+ formats scrolls smoothly with DOM rows ≈ visible + buffer (virtualization observable via DOM node count).
  3. Filter chips (video-only/audio-only/free-merge) + text filter narrow the table; empty result shows the "No formats returned…" copy.
- **Difficulty:** M.
- **Context pack:** UX.md S4 + DESIGN.md §4 gap #1 (VirtualList — fixed row height, no dep) + §3 (`table`). Load: T9 `AddDownload.svelte`, `FormatQuickPicks.svelte`. Add shadcn `table`/`checkbox`/`toggle` via MCP.
- **Status:** ✅ Done — `VirtualList.svelte` (generic fixed-row-height windower, `<script generics="T">`, scroll-position-driven slice + spacer/translate, no dep) built as the shared component T14 will reuse for S2. `FormatPicker.svelte` (S4) built as a plain hand-rolled dialog matching T8/T9's established precedent (repo has no shadcn/Tailwind pipeline despite `components.json`/DESIGN.md — same deviation noted, not repeated here). **User-confirmed scope addition beyond the T10 file list:** implementing UX.md's "free-merge" filter chip correctly required knowing whether a probed format already carries its own audio (muxed) vs. needs pairing — data `ProbeFormat` didn't expose (its `codec` field collapses to one codec, dropping whether a video row's `acodec` was also present). Asked the user; they chose the correct-semantics fix: added `has_audio: bool` to `ProbeFormat`/`map_format` (`engine_supervisor.rs`, new unit test `map_format_marks_has_audio_false_for_video_only_and_true_for_muxed` covering video-only/muxed/audio-only), mirrored into `Format` (`types.ts`) and ARCHITECTURE §7.2's `probe_formats` row — all patched in this commit per CONVENTIONS "docs first". `AddDownload.svelte` also touched beyond the file list (unavoidable per T9's own precedent of wiring mount points outside the listed files): added a "Format Picker" trigger link in the Format region header and mounted `<FormatPicker>` bound to the same `expression`/`selectedQuickPickId` S3 already threads through `FormatQuickPicks`. AC1 (select video 137 + audio 140 → `137+140`; Use format writes back to S3, deselects quick picks, closes S4) implemented via `selectedVideoId`/`selectedAudioId` slots composed into the expression, `useFormat()` nulling `selectedQuickPickId`. AC2 (100+ formats scroll smoothly, DOM rows ≈ visible+buffer) via `VirtualList`. AC3 (filter chips + text filter narrow the table; empty state copy) via `filtered` derived + the literal "No formats returned…" copy for both a genuinely empty probe and an over-narrowed filter. "Best" row (eye-first, prefers a muxed/free-merge format over needing a video+audio pair) is pre-highlighted and pre-selected on open. `cargo test` green (43 unit tests incl. the 3 new `has_audio` assertions; all previously-passing integration suites re-ran with zero regressions) — `probe_returns_formats_for_a_real_media_url` re-run explicitly (`--ignored`) against a real `yt-dlp -J` to confirm `has_audio` round-trips correctly from a live probe, not just the unit fixture. `cargo build`, `npm run build` both clean; `svelte-check` shows zero new errors (one pre-existing unrelated `main.ts`/`app.css` error predates this task, same as T9's note). **Not done:** live GUI walkthrough of S4 itself — not requested this pass and this sandbox's prior sessions established its display is the user's own live desktop (T4/T6/T9 precedent), so no automated clicking was driven there; visual confirmation deferred to the user's own `npm run tauri dev` pass or a future demo gate.

---

### T11 — Presets service + S6  *(PLAN Chunk 8)*

- **Status:** ✅ Done — `preset_service.rs` (new) owns CRUD + the two invariants (single-default via clear-then-set around the DB's partial unique index; last-preset refusal via `LastPreset` before touching the DB), backed by new `persistence.rs` preset CRUD fns (`list/get/insert/update/delete_preset`, `clear_default_preset`, `set_default_preset`, `first_other_preset_id`) and a `presets.name` UNIQUE→`PresetNameTaken` mapper that's careful to only catch the name index, not the `is_default` partial index (a real bug caught by `insert_preset_rejects_second_default` initially asserting the wrong error variant). Dry-parse (`engine_supervisor::dry_parse_format`) runs yt-dlp's real selector parser offline via `--load-info-json` against a small fake-formats stand-in (no network) — verified against a real `yt-dlp` binary in this sandbox for both a valid and a syntactically-broken expression. `rusqlite::Connection` isn't `Send`, so `preset_service`'s CRUD fns stay fully sync; ipc.rs's `create_preset`/`update_preset` commands run the `.await`ed dry-parse *before* locking the DB mutex (this was a compile-time `Send` error caught by `cargo build`, not a design foresight). 5 new IPC commands wired (`list/create/update/delete/set_default_preset`); `AddDownload.svelte` gained a Preset dropdown (S3) that copies preset fields into plain local `$state` on apply — editing afterwards only touches that local state, never the preset row (AC3/AC4 by construction, not a live binding). `Presets.svelte` (S6) implements the list + inline-editor layout from UX.md verbatim, including the invalid-expression-blocks-Save path with stderr shown inline. No Sidebar exists yet (T13), so `Queue.svelte` grew a plain "Presets" nav button + overlay as a `ponytail:`-marked stopgap to reach S6 at all; T13 replaces it. `cargo build`/`cargo test` (56 tests, incl. `preset_survives_reopen_of_a_real_db_file` proving AC1's create→relaunch→reopen round-trip through a real on-disk file) and `npm run build` both pass clean. Not visually walked in a running `tauri dev` window in this session (same caveat shape as prior tasks' sandbox display limitations) — T12's demo gate is the first live walkthrough of Flow C.
- **Objective:** preset CRUD with invariants, plus the Presets view and S3 dropdown wiring.
- **Inputs:** presets table, dry-parse via engine.
- **Outputs:** preset commands + S6 + `presets` store; S3 preset dropdown applies fields.
- **Dependencies:** T10.
- **Files:** `src-tauri/src/preset_service.rs`, `src-tauri/src/ipc.rs`, `src/lib/views/Presets.svelte`, `src/lib/stores/presets.svelte.ts`, `src/lib/views/AddDownload.svelte` (dropdown apply).
- **Acceptance criteria:**
  1. Create "4K" with `bv*[height<=2160]+ba/b`, relaunch, reopen S6 → present with that expression (K4-AC1/AC5).
  2. Marking a preset default un-stars the previous; the DB rejects a second `is_default=1` row (K4-AC2).
  3. Applying a preset in S3 fills the expression; editing after apply records the edit, not the preset value (K4-AC3/AC4).
  4. Delete on the only preset is blocked with an explanation (`LAST_PRESET`, K4-AC6); deleting the default promotes the next.
  5. Saving a preset with a garbage expression blocks with yt-dlp stderr inline (`INVALID_FORMAT_EXPR`).
- **Difficulty:** L.
- **Context pack:** UX.md S6 + DESIGN.md §3/§5; ARCHITECTURE §3 (presets DDL + single-default partial index + invariants), §7.2 (preset commands), §2 (preset_service must not apply presets — frontend composes). Load: T9/T10 `AddDownload.svelte`, T7 `ipc.rs`, `engine_supervisor.rs` (dry-parse).

---

### T12 — 🚦 DEMO GATE 3 — Flows B and C

- **Status:** ✅ Done — user-confirmed live walkthrough.
- **Objective:** prove advanced format control and preset apply end-to-end.
- **Dependencies:** T11.
- **Journeys to walk:** Flow B verbatim (probe → picker → filter video-only → select 248+140 → Use format → Add → item records `248+140`); Flow C verbatim (create 4K preset with extra args → apply in S3 → download runs under it, verified via `get_item` that args/expression match). Also observe save-blocked-on-invalid-expression.
- **Observations required:** composed expression recorded exactly; preset args/expression on the item; invalid-expression save blocked with stderr.
- **Completion artifact:** screenshot of the item's `get_item` output (or S5 precursor) showing `248+140` / the 4K args.
- **Difficulty:** S.
- **Context pack:** PLAN "Demo Gate 3"; UX.md Flows B, C. No code.

---

## Milestone 4 — Real shell & detail (T13–T18)

### T13 — Shell: sidebar + filter tree + toolbar  *(PLAN Chunk 9, part 1)*

- **Status:** ✅ Done — `filters.svelte.ts` (new store, pure frontend UI state per ARCHITECTURE §2, no backend hydration) owns the active status filter, title search, and rail-collapse toggle, plus `matches`/`countFor` helpers reused by both `Sidebar.svelte`'s live count badges and `Shell.svelte`'s visible-item derivation. Repo has no `src/routes/` (plain Vite+Svelte, confirmed at T4/T9) — `App.svelte` is the real rewire target for the task's "+page.svelte" entry; it now mounts `Shell.svelte` instead of `Queue.svelte` directly. `Queue.svelte` was cut down to just the rows region (props: filtered `items`, `totalCount`, `onAdd`, `onShowAll`) per T13's own "NOT: row internals" boundary — its old ad-hoc header/Presets-overlay/Add-button (T9/T11's `ponytail:`-marked stopgaps, explicitly named for T13 to replace) moved up into `Shell.svelte`, which now owns the Add Download and Presets overlays. `queue.svelte.ts` gained `pauseAll`/`resumeAll` (wrapping the existing `bulk_action` IPC command over caller-supplied ids) and `concurrency`/`setConcurrency` (wrapping `set_concurrency`) so the toolbar's Start all/Pause all and inline N control have a store-level home instead of views calling `ipc.ts` directly (CONVENTIONS: `ipc.ts` is the only `@tauri-apps/api` import site, but every other view goes through a store, not ipc calls inline). AC1: `Sidebar.svelte`'s filter tree narrows `Shell.svelte`'s `visibleItems` derived (`queueStore.items.filter(filtersStore.matches)`) passed into `Queue.svelte`; badges use `countFor` against the *unfiltered* `queueStore.items` so counts stay global regardless of the active filter, and update live since they're plain `$derived` over the reactive items array progress/stage_changed already patches. AC2: rail collapses to 56px both via `filtersStore.collapsed` (manual toggle button, new) and window width < 1100px (`Shell.svelte`'s `svelte:window bind:innerWidth`, OR'd together); labels swap to native `title` attributes in collapsed mode (no shadcn `tooltip` — same no-pipeline precedent as T10/T11), counts stay as badges; `cancelled` has no filter entry in `STATUS_FILTERS`/`STAGES_BY_FILTER`, so it only ever surfaces under All, matching UX.md verbatim. AC3: `QueueToolbar.svelte`'s search writes to `filtersStore.search`; N control reads `queueStore.concurrency ?? settingsStore.settings.default_concurrency` and writes via `queueStore.setConcurrency`; Start all/Pause all compute their id sets from `visibleItems` (paused → resume; downloading/merging/queued → pause) so both operate on the filtered/searched subset per UX.md, not the whole queue. AC4: active filter row gets both `font-weight: 700` and a `border-inline-start` accent (weight + indicator, not color alone per DESIGN.md hard rule #4); every new interactive control (`Sidebar` rows, collapse toggle, toolbar search/N/bulk buttons) has an explicit `:focus-visible` 2px `--ring` outline matching the existing app-wide convention (NFR-4). Empty states split per UX.md: true-empty (`totalCount === 0`) shows the "No downloads yet… press Add" copy with a working Add link; filtered-to-empty shows "Nothing here… Show all" wired to `filtersStore.reset()`. Settings' pinned sidebar slot is a `ponytail:`-marked disabled placeholder — S7 doesn't exist until T17, so wiring an onclick to nothing would be worse than an honest disabled state; icons are hand-rolled unicode glyphs (no `lucide-svelte` dependency added, same no-new-dep precedent as every prior task). `npm run build` clean; `svelte-check` shows the same single pre-existing `main.ts`/`app.css` error T10 already noted, zero new errors; `cargo build` unaffected (no Rust touched — `bulk_action`/`set_concurrency` commands already existed from T6). No frontend test infra exists in this repo (no vitest configured despite CONVENTIONS mentioning it as future-optional) and no prior task introduced one, so none was added here either. Not visually walked in a running `tauri dev` window this session — not requested for this task and visual quality is explicitly the human's call at the next demo gate (T18) per this skill's own opt-in-visual-verification rule.
- **Objective:** the persistent app chrome — sidebar, live-count status filters, toolbar.
- **Inputs:** stage_changed events (counts), settings (N).
- **Outputs:** `Shell.svelte`, `Sidebar.svelte`, `QueueToolbar.svelte`, `filters` store.
- **Dependencies:** T12.
- **Files:** `src/lib/views/Shell.svelte`, `src/lib/components/Sidebar.svelte`, `src/lib/components/QueueToolbar.svelte`, `src/lib/stores/filters.svelte.ts`, `src/routes/+page.svelte` (rewire to Shell).
- **Acceptance criteria:**
  1. Sidebar shows the status filter tree (All/Downloading/Queued/Paused/Completed/Failed) with count badges that update live as stages change; clicking a filter narrows the S2 list in place.
  2. Sidebar collapses to a ~56px icon rail below the width threshold or by toggle; labels become tooltips, counts become badges; `cancelled` items appear under **All** only.
  3. Toolbar title search + inline N control + Start all / Pause all operate on the visible queue.
  4. Active filter is marked by weight + indicator, not color alone; focus ring visible on every control (NFR-4).
- **NOT:** row internals/virtualization/selection (T14).
- **Difficulty:** L.
- **Context pack:** UX.md S2 (sidebar/toolbar regions) + DESIGN.md §4 gap #5 (rail collapse), §6 (layout, 240/56px), §7 (hard rules). Load: T4 `queue.svelte.ts`, `settings.svelte.ts`, `App.svelte`. Add shadcn `button`/`badge`/`tooltip`/`input` via MCP.

---

### T14 — Queue rows: virtualized list, progress signature, selection + bulk + drag  *(PLAN Chunk 9, part 2)*

- **Status:** ✅ Done — `QueueRow.svelte` (new) is the dense row: checkbox · title (truncate + `title` tooltip) · size · inline progress region (`StageToken` + pill bar, thin 4px standard/thick 8px active per DESIGN_SYSTEM Progress Bars, + `%`/speed figures, error text + Retry inline) · ETA · a hand-rolled `⋯` overflow menu (Pause/Resume/Retry/Move up-down/Cancel/Remove — same no-shadcn-pipeline precedent as every prior task, no `dropdown-menu` dep added). `StageToken.svelte` (new) is icon+`label-mono` text per stage (7 distinct stages incl. `merging`/`cancelled` beyond Sidebar's filter-tree icon set), colored per DESIGN.md gap #2 (`primary`/on-surface-variant/`secondary`/`--error-token`), never color alone. `SelectionBar.svelte` (new) appears only when `count > 0`, hosts Start/Pause/Cancel/Remove/Move ▲▼/Clear, routing Cancel/Remove through `Queue.svelte`'s confirm+undo flow rather than calling the store directly. `Queue.svelte` rewritten as the orchestrator: `VirtualList` (T10) windows the rows (`ROW_HEIGHT=56`, container height via `bind:clientHeight`) satisfying AC1's DOM-rows-≈-visible+buffer; a `Set<number>` selection + roving-tabindex keyboard focus (`focusedId`, arrow keys move it, a `$effect` calls `.focus()` on the matching `[data-row-id]` element, Enter opens detail) satisfy AC5; native pointer events on each row's non-interactive "surface" region (checkbox/action cells `stopPropagation` so they never start a drag) track movement past a 6px threshold — below it, `pointerup` opens detail (`queueStore.openDetail`, new: ARCHITECTURE §2 places `activeDetailId` in the queue store; nothing renders it yet since S5 is T15, but the wiring has to exist now for AC3's click-vs-drag branch to mean anything), at/above it `pointerup` resolves the hovered `data-row-id` via `elementFromPoint` and calls a new `queueStore.reorderTo(id, fullArrayIndex)` (thin wrapper mirroring T6's `moveUp`/`moveDown` pattern: same absolute-index `reorder_item` semantics, no backend change) — satisfies AC3. AC4's undo toast: Remove hides ids **client-side only** (`hiddenIds`, no backend call) behind a `confirm()` (ponytail, same precedent as `Presets.svelte`'s delete confirm) and defers the real `bulk_action(remove)` call 5s via `setTimeout`; Undo within the window just clears the timer and un-hides — nothing ever reaches the backend, so "bulk Remove survives restart" (AC2) is exercised only by letting the window lapse, which still calls the real, already-tested `bulk_action`. Cancel is real immediately (the process must actually stop) with Undo calling `retry_item` to restore to `queued` — not necessarily the item's exact prior stage, called out with a `ponytail:` comment since `retry_item` is the only backend verb that reverses a cancel. `queue.svelte.ts` gained `reorderTo`, `activeDetailId`/`openDetail`/`closeDetail`, and exported the pre-existing internal `bulk` (SelectionBar/Queue call it directly for pause/resume/cancel/remove instead of only the `pauseAll`/`resumeAll` wrappers T13 added) — touched beyond T14's predicted file list, unavoidable per T9/T11's own precedent for required plumbing. `npm run build` clean; `svelte-check` shows the same single pre-existing `main.ts`/`app.css` error every prior task has noted, zero new errors; `cargo build` unaffected (no Rust touched, file list was frontend-only and no new IPC/DB changes were needed — `reorder_item`/`bulk_action`/`retry_item`/`cancel_item` all already existed from T6). No frontend test infra in this repo (CONVENTIONS: none in 001-core) so none added, consistent with T13. Not visually walked in a running `tauri dev` window this session — not requested for this task and, per this skill's own opt-in-visual-verification rule, visual quality (incl. drag feel, toast timing, column alignment) is the human's call at the next demo gate (T18).
- **Objective:** the S2 list itself — dense rows, the inline-progress signature, selection bar, bulk actions, drag-reorder, undo toasts, keyboard nav.
- **Inputs:** items store, VirtualList (T10), bulk_action (T6).
- **Outputs:** `QueueRow`, `StageToken`, `SelectionBar`, wired VirtualList, empty states, undo toasts.
- **Dependencies:** T13.
- **Files:** `src/lib/views/Queue.svelte`, `src/lib/components/QueueRow.svelte`, `src/lib/components/StageToken.svelte`, `src/lib/components/SelectionBar.svelte`.
- **Acceptance criteria:**
  1. A 60-item queue scrolls with DOM rows ≈ visible + buffer, no render stall (K2-AC10, NFR-3), while 2 items download and search/sidebar/Add stay lag-free (NFR-2).
  2. Bulk Pause on two selected rows flips both to `paused` (V4-AC1); bulk Remove survives restart (V4-AC2).
  3. Drag-reorder past a ~6px movement threshold reorders; a plain click (below threshold) opens detail instead (V4-AC3).
  4. Cancel/Remove confirm then show an undo toast; undo within the toast window restores the row (soft-delete per ARCHITECTURE §8).
  5. Each stage token is icon + `label-mono` text (never color alone); arrow keys move row focus, Enter opens detail (NFR-4/NFR-5).
- **Difficulty:** L.
- **Context pack:** UX.md S2 (row + selection + states) + DESIGN.md §4 gaps #2/#3/#4 (StageToken, inline progress, drag threshold), §5 (states), §7 (no color-alone, focus). Load: T10 `VirtualList.svelte`, T6 `queue.svelte.ts`/`ipc.rs` (bulk), T13 `Shell.svelte`/`filters.svelte.ts`. Add shadcn `checkbox`/`progress`/`sonner`/`alert-dialog`/`dropdown-menu` via MCP.

---

### T15 — Detail drawer (S5)  *(PLAN Chunk 10)*

- **Status:** ✅ Done — `DetailDrawer.svelte` (new view, mounted unconditionally from `Shell.svelte` before `AddDownload` so its Esc listener registers — and thus fires — first, satisfying AC4's S5-before-S3 priority via `stopImmediatePropagation`) is a fixed right-docked panel (never a scrim, per UX §2 "sits beside S5") reading `queueStore.activeDetailId`/`items`. `FactsGrid.svelte` (new) renders Region 2 — address (copy), saving-to (open dir), `StageToken` status, size/downloaded, speed/ETA, resume capability, effective proxy (item override vs `settingsStore`'s global, AC2) with an "(override|global)" tag, and format+preset (looked up from `presetsStore`); ponytail ports QueueRow.svelte's three byte/speed/eta formatters verbatim rather than extracting a shared util, since QueueRow.svelte isn't in this task's file list. `LogDisclosure.svelte` (new) is Region 3 — hand-rolled disclosure (same no-shadcn precedent as every prior task), collapsed by default, auto-expanding when `stage==='error'`; opening it calls `get_item_log` + `watch_log{on:true}` and subscribes `log_line`, closing/unmounting calls `watch_log{on:false}` (AC1). Footer actions are contextual per QueueRow's own stage rules, collapsing to Open file/Open folder/Remove for `completed` (AC3, via new `open_path`); Cancel/Remove run their own local confirm+undo-toast flow (same shape as `Queue.svelte`'s, not shared — Remove's deferred `bulk(remove)` leaves the row visible in the list during the toast window since this component has no access to `Queue.svelte`'s local `hiddenIds`, called out as a ponytail simplification). `src-tauri/src/ipc.rs` gained `open_path{path, reveal?}` (`reveal:true` opens the parent dir for "Open folder"/"open dir", else the path itself) shelling out to `xdg-open`/`open`/`explorer` per `#[cfg(target_os)]` (ponytail: no new opener crate, Linux is the primary target per project memory) — registered in `lib.rs`'s `invoke_handler` (unavoidable plumbing, same T9/T11 precedent). Frontend `ipc.ts`/`types.ts` gained the previously-unwired `get_item`/`get_item_log`/`watch_log`/`log_line` wrappers (T7's backend commands existed but had no frontend callers until now) plus `open_path`. `npm run build` clean; `svelte-check` shows only the same pre-existing `main.ts`/`app.css` error every prior task has noted; `cargo build`/`cargo test --lib` clean (58 tests incl. 3 new `resolve_open_target` cases for the reveal/no-reveal/no-parent path-resolution logic — the only pure-logic slice of `open_path` worth unit-testing per CONVENTIONS, the actual OS-shell-out is untested). Not visually walked in a running `tauri dev` window this session — not requested for this task and, per this skill's opt-in-visual-verification rule, deferred to Demo Gate 4 (T18) same as the AC2/Flow D proxy walk this task's own NOTE already defers there.
- **Objective:** the docked per-item detail drawer with live log tail and contextual actions.
- **Inputs:** get_item, get_item_log/log_line (T7), open_path.
- **Outputs:** S5 drawer + `open_path` command.
- **Dependencies:** T14.
- **Files:** `src/lib/views/DetailDrawer.svelte`, `src/lib/components/FactsGrid.svelte`, `src/lib/components/LogDisclosure.svelte`, `src-tauri/src/ipc.rs` (`open_path`).
- **Acceptance criteria:**
  1. An errored item's drawer shows the yt-dlp stderr in the log (K3-AC6, K5-AC4); the log auto-expands to the failing tail with Retry emphasized (Flow D).
  2. A globally-set proxy shows in the item's S5; a per-item override shows the override there while others show the global (K5-AC1/AC2).
  3. A completed item offers Open file / Open folder and they open the real path via `open_path`.
  4. `Esc` closes only the topmost overlay (S5 before S3).
- **NOTE:** AC 2 and Flow D depend on the S7 proxy field — walk them at Demo Gate 4 after T17.
- **Difficulty:** M.
- **Context pack:** UX.md S5 + DESIGN.md §3 (`sheet`, `collapsible`), §5 states; ARCHITECTURE §7.2 (`open_path`), §7.3 (`log_line` gated on open drawer + `watch_log`). Load: T7 `ipc.rs` (log commands), T14 `QueueRow.svelte`/`queue.svelte.ts`.

---

### T16 — Binary in-app download + mid-session health + banner  *(PLAN Chunk 11, part 1 — backend + banner)*

- **Objective:** fetch missing binaries in-app and detect a binary going missing mid-session.
- **Inputs:** binary_manager (T1), reqwest.
- **Outputs:** `download_binary` + `binary_download` event; spawn-time health check + `binary_health` event; GlobalBanner.
- **Dependencies:** T15.
- **Files:** `src-tauri/src/binary_manager.rs` (download via reqwest + health re-check), `src-tauri/src/engine_supervisor.rs` (pre-spawn health check), `src-tauri/src/ipc.rs`, `src/lib/stores/binaryHealth.svelte.ts`, `src/lib/components/GlobalBanner.svelte`.
- **Acceptance criteria:**
  1. `download_binary{which:'ffmpeg'}` on a machine without ffmpeg fetches the official release into app-data `bin/`, emits determinate `binary_download` progress, and resolves the binary `found`.
  2. A download failure emits an error state retryable on that binary's row while the other binary stays resolvable (PRD §7).
  3. Removing a resolved binary from its path mid-session: the next spawn attempt emits `binary_health{found:false}`, active items pause, no new download starts, and GlobalBanner appears with Fix (K1-AC7).
- **Difficulty:** L.
- **Context pack:** ARCHITECTURE §2 (binary_manager owns download+health), §7.2 (`download_binary`), §7.3 (`binary_download`/`binary_health`), §8 (pre-spawn health check). Load: T1 `binary_manager.rs`, T2/T3 `engine_supervisor.rs`/`queue_manager.rs` (pause-active hook). DESIGN.md §2 (`--warning` for banner).
- **Status:** ✅ Done — `binary_manager::download_to_disk` streams yt-dlp's/ffmpeg's release with `reqwest` (rustls, no OpenSSL dep), reporting 0..100% via a plain callback (kept DB-free per ARCHITECTURE §11 — binary_manager depends on persistence + reqwest only, never `engine_supervisor`, so no mutex is held across the network await); ffmpeg's static-build archive (tar.xz on Linux, zip on macOS/Windows) is unpacked by scanning for a file literally named `ffmpeg`/`ffmpeg.exe` rather than a hardcoded nested path, so an upstream version-suffixed folder rename can't break it. New `ipc::download_binary` command (registered in `lib.rs`) calls it, then reuses existing `set_path` to validate+persist (AC1/AC2). AC1 demonstrated for real against the live network (not mocked): `#[ignore]`d `download_to_disk_fetches_a_runnable_ffmpeg` fetches the actual johnvansickle.com static ffmpeg, extracts it, and confirms the result is genuinely runnable via `probe_version` — run explicitly and passed. AC2's "other binary stays resolvable" is structural (each `which` only ever touches its own settings keys), backed by 4 new archive-extraction unit tests (found-nested-in-subfolder + not-found-in-archive, for both zip and tar.xz). AC3: `engine_supervisor::run_download` now runs a pre-spawn existence check (`missing_binary`/`binary_present` — a cheap `Path::is_file`, deliberately shallower than `binary_manager`'s `--version` probe so it doesn't spawn an extra process before every single download) before doing anything else; on a miss it reverts the about-to-start item to `queued` (not `error` — it never actually ran), pauses every other item this module holds a running child for (marking each `paused` in the DB, same as a manual pause), and emits the new `Emitter::emit_binary_health` — covered by 2 new `#[tokio::test]`s, one of which spawns a real `sleep 5` child into the registry to prove an *actually running* process gets killed, not just a DB-row flip. Frontend: new `binaryHealth.svelte.ts` store (listens for `binary_health`, remembers the most recent missing binary) + new `GlobalBanner.svelte` (UX.md S7 wording, `--warning` token per DESIGN.md §2, Fix button) mounted app-wide in `App.svelte`; Fix re-runs `detect_binaries` (via `settingsStore.init()`) and reopens Onboarding (S1), matching UX.md's "Fix reopens S1". `cargo test --lib` green (67 passed, 1 real-network test run separately and passed); `cargo build` clean; `svelte-check` clean (one pre-existing unrelated `main.ts`/`app.css` error predates this task). Deliberate scope note: macOS/Windows ffmpeg URLs (evermeet.cx, gyan.dev) are wired but unverified against the real network in this Linux-only sandbox — same gap T1 already accepted for its PATH-search fallback.

---

### T17 — Onboarding full (S1) + Settings (S7)  *(PLAN Chunk 11, part 2 — UI)*

- **Status:** ✅ Done — `BinaryRow.svelte` (new) is the shared per-binary row: live status token (checking/found/downloading/not-found), used by both S1 (with `onDownload`, showing the found/not-found choice + determinate progress + inline failure/Retry/stderr disclosure) and S7 (without `onDownload`, compact path/version + Change…). `Onboarding.svelte` rewritten full per UX.md S1: Region 1 Engine check (bundled build shows the single "Engine bundled ✓" density line instead, `bothFound` short-circuits true for it per ARCHITECTURE §9's bundled-seeds-paths note), Region 2 Network (proxy), footer Continue (gated on `bothFound`) / **I'll set it later** (new `onSkip` prop, ungated). `settings.svelte.ts` gained `downloadBinary` (wraps T16's `download_binary` + `binary_download` event, per-which progress/error/stderr state), `recheck` (wraps `recheck_binaries`), and a generic `update(partial)` alongside the existing `saveProxy`/`resolveBinaryPath`; ponytail: binary-download "Cancel" has no backend counterpart (T17's file list is frontend-only, no ipc.rs command to abort an in-flight `download_binary`) so Cancel isn't wired — only Retry-after-failure — noted inline, upgrade path is a real abort once ipc.rs grows one. `Settings.svelte` (new) is S7 verbatim: Engine & health (BinaryRow × 2, read-only path display + Change…, plus Re-check/Re-run onboarding buttons), Downloads (N/output-dir-via-native-directory-picker/template/default preset), Network (proxy), About (build flavor + versions from `settingsStore.settings`); ambiguity note: the "Default preset" select calls `presetsStore.setDefault` (the `is_default` flag S3/S6 actually read) rather than `update_settings{default_preset_id}`, keeping one source of truth for "the default preset" instead of two disconnected fields sharing a name — internal wiring choice, no user-visible behavior change. AC2's degraded mode: `Shell.svelte` derives `downloadsDisabled` from `settingsStore.binaries`/`build_flavor` and threads it to `Sidebar.svelte` (Add button `disabled` + explanatory `title`) and `Queue.svelte` (empty-state swaps to an explanatory line instead of the Add CTA) — touched beyond T17's predicted file list (`App.svelte`, `Shell.svelte`, `Sidebar.svelte`, `Queue.svelte`, `ipc.ts`), unavoidable plumbing per T14/T15's own precedent since Onboarding/Settings alone can't gate the queue's Add entry points. `Shell.svelte` also now mounts `Settings.svelte` as a scrim overlay (same pattern as its existing Presets overlay) with a `Ctrl/Cmd+,` window keydown listener (AC3), and `Sidebar.svelte`'s previously-disabled Settings placeholder button now opens it — the T13 `ponytail:`-marked stopgap this task was named to replace. `App.svelte` gained `handleSkip` (mirrors `handleContinue` minus the `bothFound` gate) and `handleReRunOnboarding` (mirrors `GlobalBanner`'s existing `handleFix`, minus the `binaryHealthStore.clear()` since Settings' own Re-check already re-detects). `ipc.ts` gained `recheckBinaries`/`pickDirectory` wrappers for backend commands (`recheck_binaries`) and native-dialog calls (directory mode) that already existed but had no frontend caller. `npm run build` clean; `svelte-check` shows only the same pre-existing `main.ts`/`app.css` error every prior task has noted, zero new errors; `cargo build` unaffected (no Rust touched — every command this task wires, `recheck_binaries`/`download_binary`/`update_settings`/`set_default_preset`, already existed from T6/T11/T16). No frontend test infra in this repo (CONVENTIONS: none in 001-core), consistent with every prior UI task. Not visually walked in a running `tauri dev` window this session — not requested for this task and, per this skill's opt-in-visual-verification rule, deferred to Demo Gate 4 (T18), which already depends on T17 for the S7-proxy-dependent AC2/Flow-D walks T15 deferred here.
- **Objective:** the complete first-run wizard and the settings surface.
- **Inputs:** all binary/settings/preset commands + T16 events.
- **Outputs:** full S1, S7, degraded-mode entry.
- **Dependencies:** T16.
- **Files:** `src/lib/views/Onboarding.svelte` (full S1), `src/lib/views/Settings.svelte` (S7), `src/lib/components/BinaryRow.svelte`.
- **Acceptance criteria:**
  1. K1-AC1…AC5 and AC7 pass verbatim on the light build: per-binary live tokens, "Download for me" (determinate + cancel + inline failure/Retry/stderr) or "Set path…", Continue gated on all binaries resolved.
  2. **I'll set it later** lands on S2 in degraded read-only mode (Add disabled with an explanation).
  3. S7 round-trips the global proxy through SQLite (K1-AC4); Re-check re-runs detection; Re-run onboarding reopens S1 with current state; `Ctrl/Cmd+,` opens S7; About shows build flavor + versions.
- **NOT:** playlist (T19).
- **Difficulty:** L.
- **Context pack:** UX.md S1 + S7 + DESIGN.md §3/§5; ARCHITECTURE §7 (binary/settings commands + events), §9 (config). Load: T16 `binaryHealth.svelte.ts`/`GlobalBanner.svelte`/`binary_manager.rs`, T4 `Onboarding.svelte` (minimal → full), T11 `presets` store (default-preset select). Add shadcn `card`/`progress`/`select` via MCP.

---

### T18 — 🚦 DEMO GATE 4 — full kernel journey + Flow D

- **Objective:** the SPEC kernel journey verbatim in the real UI, plus failure recovery, keyboard-only.
- **Dependencies:** T17.
- **Journeys to walk:** SPEC kernel journey verbatim (light build, no ffmpeg → "Download for me" completes in-app → proxy → probe → pick 1080p via picker *or* type expression → apply preset → parallel 2 → quit mid-download → relaunch → resume → complete at templated path), then Flow D verbatim (error → read log in S5 → fix proxy in S7 → Retry → resumes with partial bytes).
- **Observations required:** stage tokens, live sidebar counts, undo toasts, and full keyboard-only operation of the journey (NFR-5 script).
- **Completion artifact:** screenshot series covering onboarding download, parallel progress, and post-Retry resume.
- **Difficulty:** M (full walk + keyboard pass).
- **Context pack:** PLAN "Demo Gate 4"; UX.md Flow A + Flow D. No code.
- **Status:** ✅ Done — verified by user.

---

## Milestone 5 — v1 completion (T19–T21)

### T19 — Playlist expansion + remaining edges  *(PLAN Chunk 12)*

- **Objective:** expand playlists to independent rows and close remaining PRD §7/§8 edge cases.
- **Inputs:** add_download, queue_manager.
- **Outputs:** playlist expansion + edge-case handling + S3 playlist hint.
- **Dependencies:** T18.
- **Files:** `src-tauri/src/queue_manager.rs` (`--flat-playlist -J` expand), `src-tauri/src/ipc.rs`, `src/lib/views/AddDownload.svelte` (playlist hint).
- **Acceptance criteria:**
  1. A playlist of M entries yields M rows sharing a `playlist_id`, each independently controllable; cancelling one leaves the others running (K2-AC3, V2-AC2).
  2. A playlist with one dead entry: live entries complete; the dead one becomes its own `error` row with its own stderr (PRD §7).
  3. Disk-full on one item → that item `error`, others unaffected; offline at launch → app opens, queue browsable, network errors surface only on attempt (PRD §7/§8).
- **Difficulty:** L.
- **Context pack:** ARCHITECTURE §2 (queue_manager owns playlist expansion), §3 (`playlist_id`), §7.2 (`add_download` returns N items). Load: T3/T6 `queue_manager.rs`, T7 `ipc.rs`, T9 `AddDownload.svelte`.
- **Status:** ✅ Done — `engine_supervisor::expand_playlist` (new) runs `yt-dlp -J --flat-playlist` (same one-shot-call shape as T4's `probe`, so queue_manager still never spawns a process directly) and maps its JSON into a pure-testable `PlaylistExpansion { playlist_id, entries }`; `entries.len() > 1` is the only case that gets a `playlist_id` (`parsed.id` → `webpage_url` → the submitted URL itself as fallback), so a playlist of one and a lone video both collapse to the same single-`PlaylistEntry`/no-`playlist_id` shape — AC1's "M rows sharing a `playlist_id`" only ever fires for a real multi-entry playlist. Per-entry URL prefers `webpage_url` over the flat listing's bare `url` (some extractors' flat entries carry only a video id there); an entry with neither is dropped rather than inserted with a garbage URL, `ponytail:`-noted since a per-extractor URL builder is the upgrade path if a site without `webpage_url` ever surfaces. `queue_manager::add_download_expanding` (new) is the actual T19 orchestration: expand once, then loop `add_and_schedule` per entry with a cloned `AddDownloadParams` (now `Clone` + carrying `playlist_id`/`title`) — each entry gets the *same* independent add-then-maybe-spawn treatment a single add always got (AC1's "independently controllable": pausing/cancelling one is just `pause_item`/`cancel_item` on that row's id, untouched by this task). `persistence::NewItem`/`insert_item` gained `playlist_id`/`title` columns-through (the DDL already had them since T1; only the insert path was NULL-only pre-T19). `ipc::add_download` is now `async` (playlist expansion is a real subprocess call) and calls `add_download_expanding` instead of `add_and_schedule` directly, returning all N items instead of always `vec![item]`; the pre-existing settings/binary-resolution block was scoped into a `{ }` block ending before the `.await` so the `MutexGuard` never crosses it (tauri's `generate_handler!` requires the command future be `Send`). AC2 (dead entry → its own `error` row) and AC3 (disk-full on one item → that item `error`, others unaffected) needed **no new code** — both are the existing per-item `engine_supervisor::run_download` failure path (non-zero exit → `error` stage + stderr, T2/T7), which was already independent per row; T19 only had to make sure playlist entries *are* independent rows, which `add_download_expanding` does. AC3's "offline at launch → app opens, queue browsable" is also pre-existing: `lib.rs`'s `setup()` only calls `reconcile_and_resume` (resumes previously-dirty items locally-known from SQLite, no network probe) before the window renders — verified no eager network call exists on the launch path. S3's playlist hint (`AddDownload.svelte`'s "(paste a playlist and it expands to N items on Add)") already shipped in T9 and needed no change — `src/lib/views/AddDownload.svelte` in this task's file list turned out to be a no-op; `Item`/`AddDownloadResponse`/`queue.svelte.ts`'s `add()` already handled `items: Item[]` generically since T9 too, so **no frontend files were touched**. New unit tests: 4 in `engine_supervisor.rs` for `parse_playlist_json` (multi-entry expansion, lone-video passthrough, single-entry-playlist-is-not-a-playlist, entries-with-no-resolvable-url-are-skipped) plus 2 integration-shaped tests in `queue_manager.rs` (`add_download_expanding_creates_one_independent_row_per_playlist_entry`, `add_download_expanding_leaves_a_lone_video_without_a_playlist_id`) that spawn a fake shell-script `yt-dlp` answering `-J --flat-playlist` with canned JSON — `n_slots: 0` keeps every row `queued` so the test exercises expansion + row creation without also spawning a real download against the fake binary. `cargo build`/`cargo build --lib` clean; `cargo test --lib` 73 passed (was 71 pre-T19), 1 ignored (pre-existing network test); `npm run build` clean and unchanged. Not visually walked in a running `tauri dev` window — backend-only task, no UI changed, and per this skill's opt-in-visual-verification rule that's the human's call at the next demo gate (T21) regardless.

---

### T20 — Build flavors + packaging  *(PLAN Chunk 13)*

- **Objective:** the `bundled` vs `light` compile-time flavors and cross-platform packaged builds.
- **Inputs:** binary_manager, tauri.conf.
- **Outputs:** bundled sidecars + seeding + flavor-aware S1 skip; AppImage/deb/msi artifacts.
- **Dependencies:** T19.
- **Files:** `src-tauri/tauri.conf.json` (+ per-flavor config), `src-tauri/build.rs` (or cargo feature `bundled`), `src-tauri/src/binary_manager.rs` (bundled seeding), `src-tauri/binaries/`, packaging config.
- **Acceptance criteria:**
  1. A bundled build on a machine with no system yt-dlp/ffmpeg completes a download (V5-AC2) and never shows the S1 wizard (K1-AC6).
  2. S7 About matches the installed flavor (V5-AC1).
  3. All four artifacts (linux/windows × bundled/light) build; light on Windows resolves `where yt-dlp` correctly (NFR-6 cross-platform paths verified).
- **Difficulty:** L.
- **Context pack:** ARCHITECTURE §9 (build flavor constant + bundled seeding), §2 (binary_manager). Load: T1/T16 `binary_manager.rs`, T0 `tauri.conf.json`. Cross-platform path verification is the risk — test Linux + Windows.

---

### T21 — 🚦 DEMO GATE 5 — v1 exit

- **Objective:** the 001-core exit bar across both flavors and OSes where hardware allows.
- **Dependencies:** T20.
- **Journey to walk:** kernel journey + one playlist + duplicate-URL warn + delete-default-preset promotion + `kill -9` durability + keyboard-only pass + WCAG AA spot-check, on both flavors and both OSes.
- **Observations required:** every listed behavior passing; AA contrast confirmed with a tool; keyboard-only completion.
- **Completion artifact:** screenshots of both flavors' About + a passing kernel journey on each OS available; a short pass/fail checklist.
- **Acceptance:** anything failing is fixed before the cycle closes. If an OS is unavailable, mark that leg `GATE SKIPPED` with the reason (never delete it).
- **Difficulty:** M.
- **Context pack:** PLAN "Demo Gate 5"; full UX.md. No code.

---

## Task graph (dependency order)

```
T0 → T1 → T2 → T3 → T4 → [T5 gate]
                          → T6 → T7 → [T8 gate]
                                      → T9 → T10 → T11 → [T12 gate]
                                                         → T13 → T14 → T15 → T16 → T17 → [T18 gate]
                                                                                         → T19 → T20 → [T21 gate]
```

Strictly sequential; each task is one implementation prompt. Gates (T5/T8/T12/T18/T21) are walk-and-capture, not code — but a failing gate blocks the next milestone.
