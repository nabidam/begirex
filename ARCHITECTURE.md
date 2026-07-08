# ARCHITECTURE — BegireX (001-core)

> Single current-truth for the system. Derived from `PRD.md` + `UX.md`. Every UX flow is traceable through the IPC contract (§7). One committed stack, one design per decision — no open alternatives. Living document; patched on every change.

---

## 1. System overview

BegireX is a **single-process desktop app** with two halves bridged by Tauri IPC:

- **Frontend** (WebView): Svelte 5 UI. Owns rendering, local UI state, and user input. Talks to the backend only through `invoke(command)` calls and by subscribing to backend `events`. Holds **no** durable state — it is a projection of backend state.
- **Backend** (Rust, native): owns everything durable and everything privileged — the SQLite database, the yt-dlp/ffmpeg child processes, binary discovery, and the filesystem. It is the **single source of truth**.

```
┌──────────────────────── Tauri Process ─────────────────────────┐
│                                                                │
│  WebView (Svelte 5)                Rust core                   │
│  ┌───────────────────┐             ┌────────────────────────┐  │
│  │ views S1–S7       │  invoke →   │ command handlers       │  │
│  │ stores (runes)    │             │ ─ queue manager        │  │
│  │ event listeners   │  ← emit     │ ─ engine supervisor    │  │
│  └───────────────────┘             │ ─ binary manager       │  │
│         ▲                          │ ─ preset/settings svc  │  │
│         │ projection               │ ─ persistence (SQLite) │  │
│         └────────────────────────  └──────────┬─────────────┘  │
│                                               │ spawn/stream   │
│                                     ┌─────────▼──────────┐     │
│                                     │ yt-dlp child procs │     │
│                                     │  (ffmpeg for merge)│     │
│                                     └────────────────────┘     │
└────────────────────────────────────────────────────────────────┘
        SQLite file  +  partial download files  (app data / disk)
```

**Committed stack**
| Concern | Choice |
|---------|--------|
| Shell | Tauri 2 |
| Frontend | Svelte 5 (runes), TypeScript, Vite |
| Backend | Rust (Tauri core) |
| Persistence | SQLite via `tauri-plugin-sql` (SQLite driver), single DB file in app data dir |
| Child process mgmt | Rust `tokio::process` (async spawn + streamed stdout/stderr) |
| Async runtime | Tokio (Tauri's runtime) |
| Binary fetch (light) | Rust HTTP client (`reqwest`) for in-app yt-dlp/ffmpeg download |
| Component library | **shadcn-svelte** (the Svelte 5 port of shadcn/ui — bits-ui primitives, copy-in components sourced via the shadcn MCP) |
| Styling | Tailwind; `DESIGN_SYSTEM.md` tokens map to shadcn-svelte's CSS-variable theme (purple Material → its `--primary`/`--background`/`--foreground`/radius vars) |
| Icons | **lucide-svelte** (shadcn-svelte's default set) — supersedes Material Symbols Outlined |

No new heavy deps beyond these; yt-dlp/ffmpeg are the engine, Rust owns process mgmt, SQLite owns state. Frontend UI is assembled from shadcn-svelte components (button, dialog/sheet, table, input, select, dropdown-menu, checkbox, progress, toast, tooltip) themed by `DESIGN_SYSTEM.md`; add components via the shadcn MCP rather than hand-rolling primitives.

---

## 2. Module responsibilities & boundaries

### Backend (Rust)

| Module | Owns | Must not |
|--------|------|----------|
| **binary_manager** | Detect yt-dlp/ffmpeg (PATH + configured path); validate runnable; download-in-app (light); persist resolved paths; mid-session health re-check | Touch the queue or DB tables other than `settings` |
| **engine_supervisor** | Spawn one `yt-dlp` child per active item; hold the child handle; stream+parse stdout/stderr into progress; enforce concurrency N; pause (kill child; resume re-spawns with `-c` — no SIGSTOP, Windows has none)/cancel/resume | Decide *which* item runs next (that's queue_manager); write UI state |
| **queue_manager** | The authoritative in-memory queue + scheduling (pick next `queued` when a slot frees); apply add/pause/cancel/remove/reorder; playlist expansion; own the write path to `items` | Spawn processes directly (asks engine_supervisor); parse yt-dlp output |
| **persistence** | All SQLite reads/writes; migrations; crash-safe writes (progress checkpoints) | Contain business rules beyond CRUD + queries |
| **preset_service** | CRUD presets; enforce single-default + last-preset invariants; dry-parse format expression on save | Apply presets to a download (frontend composes the effective config; queue_manager records it) |
| **settings_service** | Global proxy, default N, default output dir, default filename template, default preset id, build flavor | — |
| **ipc** (command handlers + event emitters) | Translate `invoke` calls to module calls; validate inputs at the trust boundary; emit progress/state events | Hold state (delegates to modules) |

### Frontend (Svelte)

| Module | Owns |
|--------|------|
| **stores/** (runes) | `queue`, `filters`, `presets`, `settings`, `binaryHealth`, `activeDetailId` — all hydrated from backend, updated by events |
| **ipc client** | Thin typed wrappers over `invoke` + `listen`; the *only* place `@tauri-apps/api` is imported |
| **views/** S1–S7 | Render + input; call ipc client; never reach the DB or processes |

**Boundary law:** the frontend never computes durable truth. Progress, stage, resume capability, and "what runs next" are all backend-emitted. The frontend re-renders; it does not decide.

---

## 3. Data model (SQLite DDL)

Single DB file (`begirex.db`) in the Tauri app-data dir. All timestamps are epoch-millis integers (UTC).

```sql
PRAGMA journal_mode = WAL;      -- crash-safe, concurrent read while writing
PRAGMA foreign_keys = ON;

-- One row per download unit. Playlist entries are individual rows.
CREATE TABLE items (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  url               TEXT    NOT NULL,
  playlist_id       TEXT,                       -- groups playlist-derived items; NULL for singles
  title             TEXT,                       -- filled after probe/first metadata line
  stage             TEXT    NOT NULL
                    CHECK (stage IN ('queued','downloading','merging',
                                     'completed','paused','error','cancelled')),
  format_expr       TEXT    NOT NULL,           -- the authoritative selector passed to yt-dlp
  output_dir        TEXT    NOT NULL,           -- resolved absolute dir at add-time
  output_template   TEXT    NOT NULL,           -- e.g. %(title)s.%(ext)s
  proxy             TEXT,                       -- effective proxy (override or global snapshot); NULL = none
  extra_args        TEXT,                       -- free-form yt-dlp flags (space-delimited, stored verbatim)
  preset_id         INTEGER REFERENCES presets(id) ON DELETE SET NULL,
  total_bytes       INTEGER,                    -- known after start; NULL until then
  downloaded_bytes  INTEGER NOT NULL DEFAULT 0, -- checkpointed for resume-observability
  percent           REAL    NOT NULL DEFAULT 0, -- 0..100, last checkpoint
  speed_bps         INTEGER,                    -- last observed; NULL when not downloading
  eta_seconds       INTEGER,                    -- last observed; NULL when unknown
  resume_capable    INTEGER NOT NULL DEFAULT 1, -- 0/1; yt-dlp reports whether partial resume is possible
  output_path       TEXT,                       -- final resolved file path once known/completed
  error_message     TEXT,                       -- last stderr summary when stage='error'
  queue_position    INTEGER NOT NULL,           -- ordering among items; drives scheduling + reorder
  created_at        INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL
);
CREATE INDEX idx_items_stage    ON items(stage);           -- sidebar filters + scheduler scan
CREATE INDEX idx_items_position ON items(queue_position);  -- ordered list + next-to-run
CREATE INDEX idx_items_playlist ON items(playlist_id);     -- playlist grouping

-- Named config bundles.
CREATE TABLE presets (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  name             TEXT    NOT NULL UNIQUE,
  format_expr      TEXT    NOT NULL,
  output_template  TEXT    NOT NULL,
  proxy            TEXT,                         -- NULL = inherit global
  extra_args       TEXT,
  is_default       INTEGER NOT NULL DEFAULT 0,   -- exactly one row = 1 (enforced in preset_service)
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_presets_default ON presets(is_default) WHERE is_default = 1; -- DB-level single-default guard

-- Flat key/value app settings. One row per key.
CREATE TABLE settings (
  key    TEXT PRIMARY KEY,
  value  TEXT
);
-- Seeded keys: global_proxy, default_concurrency (N), default_output_dir,
-- default_output_template, default_preset_id, ytdlp_path, ffmpeg_path,
-- build_flavor, ytdlp_version, ffmpeg_version.

-- Ring-buffered log tail per item (S5 log disclosure). Trimmed to last K lines/item.
CREATE TABLE item_logs (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  item_id  INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  ts       INTEGER NOT NULL,
  stream   TEXT    NOT NULL CHECK (stream IN ('stdout','stderr')),
  line     TEXT    NOT NULL
);
CREATE INDEX idx_logs_item ON item_logs(item_id, id);
```

**Invariants (enforced in service layer, backed by DB where possible)**
- Exactly one preset has `is_default=1` (partial unique index above rejects a second; service promotes another when the default is deleted).
- At least one preset always exists — delete of the last preset is refused before hitting the DB.
- `downloaded_bytes` is checkpointed on every progress tick that crosses a threshold (see §8) so K2-AC5 (resume ≥ prior bytes) holds after a hard kill.
- Progress fields (`percent`, `speed_bps`, `eta_seconds`) are the frontend's read model but the DB copy is only a checkpoint; live values arrive via events.

---

## 4. Concurrency & scheduling model

- **queue_manager** holds an in-memory ordered view of `items` (source: `items` ordered by `queue_position`). A **semaphore of size N** gates active slots.
- On add / on any item leaving active (`completed`/`error`/`cancelled`/`paused`): the scheduler scans for the lowest-`queue_position` item in stage `queued` and, if a slot is free, asks **engine_supervisor** to spawn it → stage `downloading`.
- **N change** (settings): resize the semaphore; if increased, immediately schedule waiting items; if decreased, currently-active items finish (no mid-flight kill), new starts respect the lower N.
- **Pause** = stop the child (kill) but keep partial bytes on disk + `resume_capable`; **resume** = re-spawn with `yt-dlp -c` from the partial file. **Cancel** = stop child, keep row as `cancelled` (partial file removed). **Remove** = stop if active, delete row + partial file.
- **ponytail:** scheduling is a single-lock scan over the queue (fine for hundreds of items). If queues reach many thousands, index the "next queued" lookup — the `idx_items_position` + `idx_items_stage` indices already make that a cheap query.

---

## 5. Process & progress pipeline

1. engine_supervisor spawns `yt-dlp` (child) with resolved args: `--newline`/progress-template for parseable output, `-c` for resume, `--ffmpeg-location <ffmpeg_path>`, format expr, proxy, output template, extra args.
2. stdout is read line-by-line async (tokio). A parser converts progress lines → `{percent, downloaded_bytes, total_bytes, speed_bps, eta_seconds, stage}`. `merging` is detected from the post-download merge phase; `completed` from exit code 0 + final file.
3. Each parsed tick updates in-memory item state and **emits** a `progress` event (throttled, §8). stderr lines are appended to `item_logs` and, on failure, summarized into `items.error_message`.
4. On exit: code 0 → `completed` (+ resolve `output_path`); non-zero → `error` (+ stderr summary). Either way the slot frees and the scheduler runs.

Parsing runs on backend async tasks — never on the UI thread (NFR-2).

---

## 6. Component hierarchy → UX screen ids

```
App (root, mounts global stores + event listeners)
├─ OnboardingWizard ................................. S1   (blocking overlay; binary rows + proxy)
│    ├─ BinaryRow (yt-dlp) ├─ BinaryRow (ffmpeg)
│    └─ ProxyField
├─ Shell (persistent) .............................. S2/S6/S7 chrome
│   ├─ Sidebar (collapsible → rail)
│   │    ├─ AddButton (primary CTA → opens S3)
│   │    ├─ StatusFilterTree (All/Downloading/Queued/Paused/Completed/Failed + counts)
│   │    └─ NavItems (Presets → S6, Settings → S7)
│   └─ MainArea (routes on sidebar selection)
│       ├─ QueueView ............................... S2
│       │    ├─ QueueToolbar (search, N control, start/pause-all)
│       │    ├─ VirtualList
│       │    │    └─ QueueRow (title, size, InlineProgress, stage token, ETA)
│       │    └─ SelectionBar (bulk actions; shown when rows selected)
│       ├─ PresetsView .............................. S6
│       │    ├─ PresetList └─ PresetEditor
│       └─ SettingsView ............................. S7
│            ├─ EngineHealthSection (→ re-run S1)
│            ├─ DownloadsSection (N, dir, template, default preset)
│            └─ NetworkSection (global proxy) └─ AboutSection
├─ AddDownloadOverlay .............................. S3   (overlay over S2)
│    ├─ UrlInput └─ FormatRegion
│    │                 └─ FormatPicker ............. S4   (in-panel; modal fallback)
│    ├─ PresetSelect
│    └─ AdvancedDisclosure (output template, proxy override, extra args)
├─ DetailDrawer .................................... S5   (right drawer over S2)
│    ├─ DetailHeader (title + live bar)
│    ├─ FactsGrid (address, path, sizes, speed/ETA, resume, format/preset)
│    ├─ LogDisclosure (tails item_logs)
│    └─ DetailActions (contextual: pause/resume/cancel/retry/remove/open)
└─ GlobalBanner (binary-missing-mid-session warning) + ToastHost (undo/confirm)
```

---

## 7. IPC contract (the API)

Transport is Tauri IPC — **commands** (`invoke`, request/response, may error) and **events** (`emit`, backend→frontend push). No HTTP, no auth (single local user, single process). Every command returns `Result<T, AppError>`; errors carry a machine `code` + human `message` (+ optional `stderr`). Shapes are logical (TS-ish); wire format is JSON.

### 7.1 Error shape

```ts
type AppError = {
  code: 'BINARY_NOT_FOUND' | 'BINARY_DOWNLOAD_FAILED' | 'PROBE_FAILED'
      | 'INVALID_FORMAT_EXPR' | 'DUPLICATE_URL' | 'PRESET_NAME_TAKEN'
      | 'LAST_PRESET' | 'DB_ERROR' | 'PROCESS_ERROR' | 'VALIDATION' | 'IO_ERROR';
  message: string;      // human-readable, safe to show
  stderr?: string;      // raw engine stderr when relevant (probe/format/proxy failures)
};
```

### 7.2 Commands

**Binaries / onboarding (S1, S7)**
| Command | Request | Response | Errors |
|---------|---------|----------|--------|
| `detect_binaries` | — | `{ ytdlp: BinaryStatus, ffmpeg: BinaryStatus }` where `BinaryStatus = { found: bool, path?: string, version?: string }` | `DB_ERROR` |
| `set_binary_path` | `{ which: 'ytdlp'|'ffmpeg', path: string }` | `BinaryStatus` (re-validated) | `BINARY_NOT_FOUND` |
| `download_binary` | `{ which: 'ytdlp'|'ffmpeg' }` | `BinaryStatus` (after fetch) — progress via `binary_download` event | `BINARY_DOWNLOAD_FAILED`, `IO_ERROR` |
| `recheck_binaries` | — | same as `detect_binaries` | — |

**Queue (S2, S5)**
| Command | Request | Response | Errors |
|---------|---------|----------|--------|
| `list_items` | `{ filter?: Stage|'all' }` | `Item[]` | `DB_ERROR` |
| `get_item` | `{ id }` | `Item` | `DB_ERROR` |
| `add_download` | `{ url, format_expr, output_dir?, output_template?, proxy?, extra_args?, preset_id? }` | `{ items: Item[] }` (N rows for a playlist) | `DUPLICATE_URL` (warn, force flag re-submits), `VALIDATION` |
| `pause_item` / `resume_item` / `cancel_item` / `remove_item` | `{ id }` | `Item` (or `{ok:true}` for remove) | `PROCESS_ERROR`, `DB_ERROR` |
| `retry_item` | `{ id }` | `Item` (stage→queued) | `DB_ERROR` |
| `reorder_item` | `{ id, new_position }` | `{ok:true}` | `DB_ERROR` |
| `bulk_action` | `{ ids: number[], action: 'pause'|'resume'|'cancel'|'remove' }` | `{ updated: Item[] }` | partial: per-id result list |
| `set_concurrency` | `{ n: number }` | `{ n }` | `VALIDATION` (n<1) |
| `get_item_log` | `{ id, tail?: number }` | `LogLine[]` | `DB_ERROR` |

**Formats (S3, S4)**
| Command | Request | Response | Errors |
|---------|---------|----------|--------|
| `probe_formats` | `{ url, proxy? }` | `{ title: string, formats: Format[] }` where `Format = { id, resolution, ext, fps?, filesize?, codec?, note? }` | `PROBE_FAILED` (carries stderr) |

**Presets (S6, S3)**
| Command | Request | Response | Errors |
|---------|---------|----------|--------|
| `list_presets` | — | `Preset[]` | `DB_ERROR` |
| `create_preset` / `update_preset` | `Preset (partial for update)` | `Preset` — dry-parses `format_expr` via yt-dlp | `PRESET_NAME_TAKEN`, `INVALID_FORMAT_EXPR` (stderr) |
| `delete_preset` | `{ id }` | `{ presets: Preset[] }` (default may be reassigned) | `LAST_PRESET` |
| `set_default_preset` | `{ id }` | `{ presets: Preset[] }` | `DB_ERROR` |

**Settings (S7)**
| Command | Request | Response | Errors |
|---------|---------|----------|--------|
| `get_settings` | — | `Settings` (proxy, N, output_dir, output_template, default_preset_id, build_flavor, versions) | `DB_ERROR` |
| `update_settings` | `Partial<Settings>` | `Settings` | `VALIDATION` |
| `open_path` | `{ path, reveal?: bool }` | `{ok:true}` — open file / reveal in file manager (S5) | `IO_ERROR` |

### 7.3 Events (backend → frontend)

| Event | Payload | Purpose |
|-------|---------|---------|
| `progress` | `{ id, percent, downloaded_bytes, total_bytes?, speed_bps?, eta_seconds?, stage }` | Throttled per-item live progress (S2 rows, S5 header) |
| `stage_changed` | `{ id, stage, error_message? }` | Discrete transitions (queued→downloading→merging→completed/error…); drives sidebar counts |
| `item_added` / `item_removed` | `{ id }` / full `Item` | Keep S2 list in sync without re-fetching |
| `log_line` | `{ id, stream, line }` | Live tail into S5 log (only while a detail drawer is open for that id) |
| `binary_download` | `{ which, percent }` | S1 in-app binary fetch progress |
| `binary_health` | `{ which, found, path? }` | Mid-session binary-missing → GlobalBanner (K1-AC7) |

---

## 8. Error handling & durability strategy

- **Trust boundary = ipc layer.** Every command validates inputs (types, N≥1, non-empty url) before touching modules; validation failures return `VALIDATION` without side effects.
- **Engine failures are data, not exceptions.** A non-zero yt-dlp exit is a normal `error` stage with `error_message` (stderr summary) + full stderr in `item_logs`. The item stays in the queue and is retryable (V3). No engine failure ever crashes the app or blocks other items.
- **Probe / preset-save / proxy errors** pass yt-dlp stderr through verbatim (PRD K3/K4/K5) via the `stderr` field on `AppError`.
- **Crash safety (NFR-1):** WAL journal + checkpoint writes of `downloaded_bytes`/`percent`/`stage` on every progress tick that advances ≥1% or ≥2s since last write. On launch, any item left in `downloading`/`merging` (dirty, because the process died with the app) is reconciled to `paused` and offered for resume; `list_items` returns them with last-checkpointed bytes so the UI shows real progress immediately, then the scheduler re-spawns with `-c`.
- **Progress event throttling** (NFR-2): `progress` events coalesced to ≤~10/sec/item; `stage_changed` always fires (never throttled). This keeps the WebView responsive under many parallel downloads.
- **Binary health:** engine_supervisor checks the resolved binary path before each spawn; a missing binary emits `binary_health` (→ app-wide banner, active items paused) rather than spawning into failure (K1-AC7).
- **Destructive actions** (cancel/remove/delete-preset): confirmed in UI, executed via command, and paired with an **undo toast** where reversible (remove keeps the row soft-deletable within the toast window; ponytail: soft-delete = a `cancelled`+hidden flag for the toast lifetime, hard-delete on toast expiry).

---

## 9. Configuration strategy

- **Runtime config** lives in `settings` (SQLite) — proxy, N, output dir/template, default preset, binary paths, versions, build flavor. Read via `get_settings`, mutated via `update_settings`. Single source; no scattered config files.
- **Build flavor** is a compile-time constant baked into the binary (`bundled` vs `light`) and mirrored into `settings.build_flavor` on first run; the *bundled* build seeds `ytdlp_path`/`ffmpeg_path` to its shipped binaries and skips S1 detection.
- **Seed on first run:** create the DB, run migrations, insert the seeded "Default" preset (`is_default=1`), and default settings (N=2, output dir = OS Downloads dir, template `%(title)s.%(ext)s`).
- **OS-specific paths** (NFR-6) resolved in Rust via Tauri path APIs — app-data dir for the DB, OS Downloads dir as the default output — never hardcoded per platform.
- **No secrets, no auth, no env-based config** — single local user, single process.

---

## 10. Traceability — kernel journey (Flow A) → contract

Every kernel-journey step names the command/event that serves it. A step with no serving contract would be a gap; none remain.

| Flow A step (UX §4) | Served by |
|---------------------|-----------|
| 1. Onboarding detects missing ffmpeg | `detect_binaries` → `{ffmpeg:{found:false}}` |
| 1. Download ffmpeg in-app | `download_binary{which:'ffmpeg'}` + `binary_download` event (progress) |
| 1. (alt) set path | `set_binary_path` |
| 2. Save proxy, continue | `update_settings{global_proxy}` |
| 2. Land on empty queue | `list_items{filter:'all'}` → `[]` (empty state) |
| 3. Open Add | (frontend overlay; no backend call) |
| 4. Paste URL + Probe | `probe_formats{url, proxy}` → `{title, formats}` |
| 5. Pick 1080p **or** type expression | (frontend composes `format_expr`; no backend call) |
| 5. Apply "Archive" preset | `list_presets` (populates dropdown) |
| 5. Add to queue | `add_download{url, format_expr, preset_id, …}` → `{items}` + `item_added` event |
| 5–6. Live parallel progress (both) | `progress` + `stage_changed` events per item; N enforced by scheduler (`set_concurrency` sets N) |
| 6. Third URL waits | scheduler holds it `queued` (visible via `stage_changed`) |
| 7. Quit mid-download | checkpoint writes (§8); no command — durability is passive |
| 8. Relaunch → queue restored | `list_items` returns items with last-checkpointed `downloaded_bytes` (reopen-app read path) |
| 8. Resume from offset | scheduler re-spawns with `-c`; `progress` events resume; observable bytes ≥ pre-quit (K2-AC5) |
| 9. Completed, file on disk | `stage_changed{stage:'completed'}`; `open_path` to reveal (S5) |

Other flows: **B** (format control) → `probe_formats` + frontend expression compose + `add_download`; **C** (presets) → `create_preset`/`set_default_preset`/`list_presets` + `add_download`; **D** (recover fail) → `get_item_log` + `update_settings{proxy}` + `retry_item`.

---

## 11. Dependency graph (module-level, acyclic)

```
ipc (commands/events)
 ├─→ binary_manager ──→ persistence(settings) , reqwest(download)
 ├─→ queue_manager ───→ persistence(items) , engine_supervisor
 │                         └─→ engine_supervisor ──→ tokio::process(yt-dlp/ffmpeg)
 │                                                └─→ persistence(item_logs, checkpoints)
 ├─→ preset_service ──→ persistence(presets) , engine_supervisor(dry-parse)
 └─→ settings_service ─→ persistence(settings)

frontend: views → ipc client → (Tauri invoke/listen) → ipc
```

No module imports the frontend; no cycle. `engine_supervisor` is shared by `queue_manager` (spawn downloads) and `preset_service` (dry-parse) but depends on neither — dependencies point downward only.

---

*Living document. Any change to screens, data, or contract patches this file first (impact analysis reads here), then code follows. Next phase: planning (Phase 3) — not here.*
