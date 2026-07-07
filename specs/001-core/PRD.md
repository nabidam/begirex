# PRD — BegireX (001-core)

> Product requirements derived from `SPEC.md` + `UX.md`. Acceptance criteria are **falsifiable** — each is observable behavior a human can verify in the running app. No implementation detail lives here (that is `ARCHITECTURE.md`). The **Kernel / v1 / Backlog** split from SPEC is preserved and load-bearing; backlog items are not promoted.

Screen ids (S1–S7) refer to `UX.md`.

---

## 1. Personas & scope

- **Primary user:** a technical-but-terminal-averse media collector who knows what "1080p mp4" and occasionally `bv*[height<=1080]+ba/b` mean, runs Linux or Windows, and wants yt-dlp's power without the CLI.
- **In scope (001-core):** Kernel (§3) + v1 (§4).
- **Out of scope:** browser extension, cloud/sync, accounts, non-yt-dlp engines, media playback/conversion beyond yt-dlp+ffmpeg's own merge, background daemon, mobile, macOS. (SPEC §8.)

---

## 2. Definitions

- **Item** — one download unit (one media file). A playlist URL expands into N items.
- **Job / queue** — the persisted set of all items and their state.
- **Stage** — an item's live phase: `queued`, `downloading`, `merging`, `completed`, `paused`, `error`, `cancelled`.
- **N** — max concurrent active (`downloading`/`merging`) items. Default 2, user-configurable.
- **Preset** — a named bundle: `{format expression, output template, proxy, extra CLI args}`.
- **Engine** — the `yt-dlp` binary; **ffmpeg** is its merge dependency.
- **Build flavor** — *bundled* (ships yt-dlp + ffmpeg) or *light* (detects/downloads them).

---

## 3. KERNEL requirements

The five features without which the product is pointless. Each has functional requirements (FR) and falsifiable acceptance criteria (AC).

### K1 — Binary onboarding & health (S1, S7)

**FR-K1**
- On first launch, the app detects whether `yt-dlp` and `ffmpeg` are present and runnable.
- For each missing binary the user can either (a) set an explicit filesystem path, or (b) download it in-app.
- The same onboarding captures an optional global proxy.
- Onboarding is re-runnable from S7 Settings.
- The *bundled* flavor skips detection but still exposes binary paths + proxy in S7.
- If a binary becomes unavailable mid-session, the app surfaces a persistent, app-wide warning and offers to fix it.

**AC-K1**
1. On a machine with no `ffmpeg` on PATH, launching the *light* build shows S1 with an `ffmpeg` row reading "not found" and a `yt-dlp` row reflecting its true presence.
2. Clicking "Download for me" on the ffmpeg row and waiting shows a progress indicator that reaches 100%, after which the row reads "found" and Continue becomes enabled.
3. Choosing "Set path…" and selecting a valid `ffmpeg` binary flips the row to "found" and enables Continue without any download.
4. Entering a proxy string, clicking Continue, then reopening S7 shows the same proxy string persisted.
5. From S7, clicking "Re-run onboarding" reopens S1 with current detected state.
6. Launching the *bundled* build does **not** show the S1 detection wizard; S7 still lists both binary paths.
7. Renaming/removing the `yt-dlp` binary while the app is open, then attempting a download, produces a visible app-wide "yt-dlp is no longer at its path" banner with a Fix action, and no download starts.

### K2 — Download queue (S2, S5)

**FR-K2**
- User can add a single URL or a playlist URL; a playlist expands into N distinct items.
- The app runs up to **N** downloads in parallel (N configurable, default 2); items beyond N wait as `queued`.
- Each item shows live progress: percent, speed, ETA, and stage.
- The queue and per-item progress **persist across restart**.
- Partial downloads **resume** from their byte offset after restart or after pause (`yt-dlp -c` semantics).
- Queue actions: pause, resume, cancel, remove, reorder.
- The list virtualizes above 50 items.

**AC-K2**
1. Adding two URLs with N=2 shows both items reach stage `downloading` at the same time (both show live, independently changing percent).
2. Adding a third URL with N=2 while two are downloading shows the third at stage `queued` until one of the first two leaves the active state, at which point the third transitions to `downloading`.
3. Adding a playlist URL of M entries produces M separate rows in S2.
4. A downloading item shows a percent that increases over time, a non-empty speed value, and an ETA.
5. Quitting the app (including via kill/hard-close) mid-download and relaunching shows the same items with their last-known progress, and previously-active items resume downloading rather than restarting from 0% (observable: reported downloaded-bytes on resume is ≥ the value shown before quit).
6. Pausing an item flips its stage to `paused` and stops its percent from advancing; resuming returns it to `downloading` continuing from where it paused.
7. Cancelling an item flips it to `cancelled` and frees a concurrency slot for a `queued` item.
8. Removing an item deletes its row from S2; after restart the removed item does not reappear.
9. Reordering a `queued` item above another changes which one starts next when a slot frees.
10. A queue of 60 items scrolls without the row count in the DOM/list exceeding what is visible plus a small buffer (virtualization observable via smooth scroll and no whole-list render stall).

### K3 — Advanced format selection (S3, S4)

**FR-K3**
- On demand, the app probes available formats for a URL (`yt-dlp -F`) and presents them as a selectable list/table.
- The app **always** exposes a raw format-selector expression field (e.g. `bv*[height<=1080]+ba/b`), editable directly.
- Selecting formats in the picker composes the expression; editing the expression is authoritative for the download.
- Probe failure surfaces yt-dlp's stderr to the user.

**AC-K3**
1. Pasting a valid media URL in S3 and clicking Probe shows, within the format region, at least one selectable format entry with a resolution/size.
2. The raw expression field is visible in S3 after probe without opening any additional disclosure.
3. Typing `bv*[height<=1080]+ba/b` into the expression field and adding the item results in an item whose recorded format expression equals that string (verifiable in S5 "Format").
4. In S4, selecting a video row and an audio row composes an expression of the form `<videoid>+<audioid>` in the expression field.
5. Probing a URL that yt-dlp cannot resolve shows the yt-dlp stderr text in the format region (not a generic "error").
6. Adding an item with a syntactically invalid expression results in that item entering stage `error` on start, with the yt-dlp stderr visible in its S5 log.

### K4 — Config presets (S6, S3, S7)

**FR-K4**
- User can create, edit, delete, and duplicate named presets bundling `{format expression, output template, proxy, extra CLI args}`.
- Exactly one preset is the global default at any time.
- Any download can select a preset in S3, and can override any field after applying it.
- At least one preset always exists (the last preset cannot be deleted).

**AC-K4**
1. Creating a preset named "4K" with expression `bv*[height<=2160]+ba/b`, saving, then reopening S6 shows "4K" in the list with that expression.
2. Marking "4K" as default un-marks whatever was previously default (only one preset shows the default indicator).
3. In S3, choosing preset "4K" populates the format expression field with `bv*[height<=2160]+ba/b`.
4. After applying a preset in S3, editing the expression field and adding the item records the edited value, not the preset's value (override wins).
5. Presets and their default flag persist across restart (reopen S6 after relaunch — the list and default are unchanged).
6. Attempting to delete the only remaining preset is prevented (the delete action is unavailable or blocked with an explanation).

### K5 — Proxy & arbitrary CLI args (S1, S3, S6, S7)

**FR-K5**
- A global proxy applies to all downloads by default.
- Proxy is overridable per-download (S3) and per-preset (S6).
- A free-form "extra CLI args" field accepts any additional yt-dlp flags, editable per-download and per-preset.
- Extra args and proxy are passed to the engine for the affected download.

**AC-K5**
1. Setting a global proxy in S7 and adding a download without overriding it results in an item whose S5 shows that proxy in effect.
2. Overriding the proxy in S3's Advanced section for one item, while leaving the global proxy set, results in that item using the override (S5 reflects the override) and other items using the global value.
3. Entering `--limit-rate 500K` in extra CLI args for an item and adding it results in that flag being applied to the download (observable: the item's speed does not exceed ~500K/s).
4. An extra-args string that yt-dlp rejects surfaces the yt-dlp stderr in the item's S5 log and puts the item in stage `error`.

---

## 4. v1 requirements (in scope, beyond Kernel)

### V1 — Output configuration
**FR:** Global default output directory + filename template, editable in S7; per-download override in S3 Advanced.
**AC:**
1. Setting a default output dir in S7 and adding a download (no override) results in the completed file existing under that directory.
2. Setting a filename template `%(title)s.%(ext)s` results in a completed file whose name is the media title with the merged extension.
3. Overriding the output dir in S3 for one item places that item's file in the override dir while others use the default.

### V2 — Playlist expansion
**FR:** A playlist URL expands to one item per entry; each item is independently controllable.
**AC:**
1. Adding a playlist of M entries yields M rows, each pausable/cancellable independently.
2. Cancelling one playlist-derived item does not affect the others.

### V3 — Error state with retry
**FR:** Any item that fails shows stage `error` with the reason; user can retry; retry resumes partial bytes where possible.
**AC:**
1. An item that fails (e.g. network drop) shows stage `error` and a reason string in S2 and S5.
2. Clicking Retry on an errored item returns it to `queued`/`downloading`.
3. Retrying an item that had partial bytes on disk continues from the partial offset rather than 0% (observable downloaded-bytes ≥ pre-failure value).

### V4 — Queue actions (pause/cancel/remove/reorder)
**FR:** As K2, exposed both per-row (S2/S5) and as bulk actions on multi-selection (S2 selection bar).
**AC:**
1. Selecting two rows and clicking bulk Pause flips both to `paused`.
2. Bulk Remove on two selected rows deletes both; neither reappears after restart.
3. Drag-reordering requires a movement threshold (a plain click on a row opens S5 rather than initiating a drag).

### V5 — Two build flavors
**FR:** *bundled* ships yt-dlp + ffmpeg and skips S1 detection; *light* detects/downloads. Both expose binary settings in S7 and show build flavor in S7 About.
**AC:**
1. S7 About displays "bundled" or "light" matching the installed build.
2. The bundled build completes a download on a machine with no system yt-dlp/ffmpeg on PATH.

---

## 5. Non-functional requirements

- **NFR-1 Persistence durability:** queue + presets + settings survive a hard kill (no clean shutdown). *AC:* `kill -9` the app mid-download, relaunch → queue and presets intact (covers K2-AC5).
- **NFR-2 UI responsiveness:** progress parsing must not freeze the UI. *AC:* while ≥2 items download, the sidebar filters, search field, and Add button remain interactive with no perceptible input lag.
- **NFR-3 Virtualization:** queues >50 items stay scrollable. *AC:* K2-AC10.
- **NFR-4 Accessibility floor WCAG AA:** *AC:* every text/background pair meets 4.5:1 (3:1 for large text/glyphs); every interactive control is reachable and operable by keyboard alone (Tab order matches visual order); focus is always visibly indicated; no state is conveyed by color alone (stage tokens carry text + icon).
- **NFR-5 Keyboard operability:** *AC:* a user can, without a mouse — open Add (`Ctrl/Cmd+N`), paste + probe + choose format + add, select a queue row (arrow keys) + open detail (Enter) + pause (keyboard-reachable action), open Settings (`Ctrl/Cmd+,`), and close any overlay (`Esc`).
- **NFR-6 Cross-platform paths:** *AC:* on both Linux and Windows, binary discovery and the default output dir resolve to valid OS-appropriate paths, and completed files land at the templated location on both.
- **NFR-7 WebKitGTK safety:** no heavy backdrop-blur effects; depth via tonal layers. *AC:* the UI renders without blur artifacts or visible repaint lag on Linux WebKitGTK.

---

## 6. Validation rules

| Field | Rule | On violation |
|-------|------|--------------|
| URL (S3) | Non-empty; Add disabled until present | Add stays disabled |
| Format expression (S3/S4/S6) | Passed to engine as-is; validated by yt-dlp at start (K3) or at preset-save dry-parse (K4) | Item → `error` with stderr / Save blocked with stderr |
| Concurrency N (S2/S7) | Integer ≥ 1 | Reject non-positive; keep previous value |
| Preset name (S6) | Non-empty; unique | Save blocked with inline message |
| Output template (S3/S6/S7) | Non-empty string | Fall back to default template if blank |
| Binary path (S1/S7) | Must point at a runnable binary | Row stays "not found"; Continue disabled for that binary |
| Proxy (S1/S3/S6/S7) | Free-form; validated only by the engine at download time | Proxy failure → item `error` with stderr (K5-AC4 style) |

---

## 7. Error cases & recovery

| Case | Behavior | Screen |
|------|----------|--------|
| Binary missing at launch (light) | S1 blocks Continue until resolved (or "I'll set it later" → degraded read-only queue) | S1 |
| Binary missing mid-session | App-wide banner + Fix (reopens S1); active downloads pause | S2/S7 |
| In-app binary download fails | Inline error on the binary row + Retry; other binary still resolvable | S1 |
| Invalid format expression | Item → `error`, yt-dlp stderr in S5 log | S2/S5 |
| Probe failure (unreachable / not a media page) | stderr shown in S3 format region; user may still Add blind | S3/S4 |
| Network drop mid-download | Item → `error` (resumable); Retry continues from partial | S2/S5 |
| Disk full | Item → `error` with the disk-full stderr; no partial data loss of other items | S5 |
| Duplicate URL already queued | Warn on Add ("already in queue"); user may add anyway or cancel | S3 |
| Playlist with mixed availability | Available entries download; unavailable entries become individual `error` items with their own stderr | S2 |
| Proxy auth failure | Item → `error` with proxy stderr; fixable via S7 proxy + Retry (Flow D) | S2/S5/S7 |
| App hard-killed | Queue state survives (NFR-1); items resume on relaunch | S2 |

---

## 8. Edge & empty states

- **Empty queue:** S2 shows "No downloads yet" + Add CTA; all sidebar counts read 0.
- **Empty filter result:** S2 shows a neutral "nothing here" line + "Show all".
- **Probe returns no formats:** S4 shows "No formats returned — the URL may require auth or is not a media page."
- **First run, bundled build:** S1 detection wizard is skipped entirely.
- **Only one preset left:** delete is unavailable (K4-AC6).
- **Very long queue (>50):** virtualized (NFR-3).
- **Offline at launch:** the app still opens; the persisted queue is visible and browsable; Add/probe/download surface network errors on attempt rather than blocking the app.

---

## 9. Constraints

- Stack locked: Tauri 2 + Svelte 5; Rust backend spawns yt-dlp as child, streams stdout; SQLite for queue + presets in app data dir. (Detailed in ARCHITECTURE.md — not here.)
- Platforms: Linux (AppImage/deb) + Windows.
- Dark theme ships default; no light/dark toggle in 001-core.
- Design authority: `DESIGN_SYSTEM.md` (purple Material). The IDM reference contributes structure only (see UX.md).

---

## 10. Out of scope (001-core)

Browser extension · cloud/sync · accounts · non-yt-dlp downloaders · media playback/conversion beyond yt-dlp+ffmpeg merge · background daemon (app must be open to download) · mobile · macOS build.

---

## 11. Backlog (ranked, NOT in 001-core)

Preserved from SPEC §3 — do not promote:

1. Scheduling (one-shot start-time, then bandwidth windows).
2. Cookies / auth for private videos.
3. Subtitle / thumbnail / metadata toggles as first-class UI (available now via raw extra args).
4. yt-dlp self-update button.
5. Per-download bandwidth limit; global concurrency scheduling.
6. macOS build.
7. Light/dark toggle (dark ships default).

---

## 12. Future improvements (post-backlog ideas)

- Bandwidth graph per item / aggregate.
- Import/export presets as files.
- Notification on queue-complete.
- Per-site profiles auto-selecting presets.

(These are unranked and non-committal; listed to capture direction, not to schedule.)
