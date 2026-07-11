# UX — BegireX (001-core)

> Living document. Screens, navigation, wireframes, flows, and density decisions for the 001-core cycle. No visual styling or color lives here — that is `DESIGN_SYSTEM.md`'s job. This file encodes _intent_: what the user sees, where the eye lands, what is one click away, and what is buried.

## Pattern source

The reference pack (`specs/001-core/references/idm.png`) contributes **structure, not skin**. Four patterns are adapted; nothing is cloned:

1. **Persistent left rail as the spine of the app.** A single vertical column owns global navigation and the one primary action. It is always present, never scrolls away. In IDM this is a thin icon strip beside a wide category panel; BegireX collapses both into **one sidebar** — 240px with labels, collapsing to a ~56px icon rail (per SPEC + design system).
2. **Status/category tree as the queue filter.** IDM's "All Downloads → Compressed/Video/…" tree and "Unfinished/Finished/Queues" groups become BegireX's **status filter tree**: one click narrows the queue without leaving the view.
3. **Dense, columnar list with progress living inside the row.** Progress %, speed, and stage are read from the row itself, not a separate panel — the list is the product's center of gravity.
4. **Focused detail surface for a single item.** IDM's floating detail window (Address, File size, Downloaded, Transfer rate, Resume Capability, progress, Pause/Cancel) becomes a **right-side detail drawer** — same information density, docked instead of floating so it composes with keyboard nav and window resize.

What is deliberately _not_ borrowed: IDM's green accent (we use the purple Material system), its two-column split rail, its toolbar of 12 icons, its file-type-first categorization (BegireX filters by _status_, since a download manager's user cares about "what is running / failed" before "what type").

---

## 1. Screen inventory

| id     | Screen / surface     | Type                                               | Purpose                                                                                                                                                    | Entry points                                                                           |
| ------ | -------------------- | -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| **S1** | First-run Onboarding | Full-window wizard (blocking)                      | Detect `yt-dlp`/`ffmpeg`; let user set path or download in-app; capture global proxy                                                                       | Auto on first launch; **light** build with missing binaries; re-run from S7            |
| **S2** | Queue (Home)         | Primary view                                       | The dense list of all downloads; live progress; the app's default and center                                                                               | App launch (after S1 passes); sidebar "Queue"; closing any overlay                     |
| **S3** | Add Download         | Overlay panel (centered sheet / dialog)            | Paste URL(s), probe, choose format, apply preset, set per-download overrides, add to queue                                                                 | Sidebar **+ Add** (primary CTA); `Ctrl/Cmd+N`; global paste when a URL is on clipboard |
| **S4** | Format Picker        | Modal dialog opened from S3 (quick-pick region lives in S3) | Show probed formats as a selectable table **and** expose the raw format-selector expression field                                                          | "Format Picker" button in S3's format region (revealed after a successful probe)       |
| **S5** | Download Detail      | Right-side drawer                                  | Everything about one item: address, sizes, rate, ETA, stage, resume capability, live log tail, per-item actions                                            | Click/Enter on a queue row in S2; "Details" in row overflow menu                       |
| **S6** | Presets              | Secondary view                                     | List, create, edit, delete named config bundles; mark the global default                                                                                   | Sidebar "Presets"; "Manage presets…" link in S3's preset dropdown                      |
| **S7** | Settings             | Secondary view                                     | Binaries + health (re-run onboarding), global proxy, default concurrency N, default output dir + filename template, default preset, build-info, theme note | Sidebar "Settings" (bottom); `Ctrl/Cmd+,`                                              |

**Cross-cutting states** (not screens, but every list/async surface must render them): **empty**, **loading/probing**, **error**. Specified inline per screen below.

---

## 2. Navigation map

```
                          ┌─────────────────────────────────────────┐
        ┌──────────┐      │              MAIN CONTENT AREA           │
        │ SIDEBAR  │      │                                         │
        │ (rail)   │      │   S2 Queue  ⇄  S6 Presets  ⇄  S7 Settings│
        │          │      │      │                                  │
        │ + Add ───┼──────┼──► S3 Add Download (overlay) ──► S4 Format
        │          │      │      │            picker (in-panel)     │
        │ ◈ Queue ─┼──────┼──► filters the S2 list in place:        │
        │   All    │      │      All / Active / Queued / Paused /    │
        │   Active │      │      Completed / Failed / Cancelled      │
        │   Queued │      │      │                                  │
        │   Paused │      │   S2 row click ──► S5 Detail (drawer)    │
        │   Done   │      │                                         │
        │   Failed │      └─────────────────────────────────────────┘
        │          │
        │ ⛃ Presets┤   S1 Onboarding sits ABOVE everything on first
        │ ⚙ Setting┤   run — blocking wizard, dismissed only when a
        └──────────┘   usable yt-dlp path is confirmed (or user opts
                       into "I'll set it later", degrading S2 to a
                       read-only queue that cannot start downloads).
```

- **Sidebar is persistent** across S2/S6/S7. It collapses to an icon rail below a window-width threshold (or by user toggle); labels become tooltips, the filter tree becomes icons with count badges.
- **S3 and S5 are overlays over S2** — they never replace the queue; the list stays visible/dimmed behind S3, and sits beside S5. This preserves context (you can watch item 2 download while inspecting item 1).
- **Back behavior:** `Esc` closes the topmost overlay (S5 → S3 → nothing). Sidebar selection is the only "navigation stack"; there is no deep history to break.

---

## 3. Screens — wireframes, states, hierarchy

Notation: regions listed **top-to-bottom, start-to-end** (LTR/RTL-agnostic — the design system uses logical properties). "**→ eye first**" marks the intended first fixation. "**1-click**" = reachable without disclosure; "**buried**" = behind a chevron/menu/drawer.

### S1 — First-run Onboarding

```
┌───────────────────────────────────────────────────────────┐
│  BegireX · First-time setup                                │
├───────────────────────────────────────────────────────────┤
│                                                           │
│   ● Engine check                              → eye first │
│   ┌─────────────────────────────────────────────────────┐ │
│   │  yt-dlp     ✓ found  /usr/bin/yt-dlp   [Change…]     │ │
│   │  ffmpeg     ✗ not found                              │ │
│   │             ( ) Download for me   ( ) Set path…      │ │
│   │             [ Download ffmpeg  ▸ ]   ▓▓▓▓▁▁ 62%      │ │
│   └─────────────────────────────────────────────────────┘ │
│                                                           │
│   ● Network (optional)                                    │
│   ┌─────────────────────────────────────────────────────┐ │
│   │  Proxy   [ socks5://user:pass@host:port          ]  │ │
│   └─────────────────────────────────────────────────────┘ │
│                                                           │
│                          [ I'll set it later ]  [ Continue ]│
└───────────────────────────────────────────────────────────┘
```

- **Region 1 — Engine check** (primary): one row per binary with a live status token (`found` / `not found` / `downloading` / `failed`). Each missing binary offers two mutually exclusive resolutions: **Download for me** (in-app fetch with a determinate progress bar) or **Set path…** (native file picker). → eye first: the first _unresolved_ binary row.
- **Region 2 — Network:** single proxy field, clearly optional. Applies globally; editable later per-download and in presets.
- **Primary action:** **Continue** — enabled only when every binary is resolved (found or path set). **I'll set it later** is a subordinate escape hatch that lets the user reach S2 in a degraded, cannot-download state.
- **States:**
  - _loading:_ detection runs on open — binary rows show a spinner token until resolved; Continue disabled.
  - _download-in-progress:_ the fetch shows a determinate bar + cancel; failure flips the row to an error token with **Retry** and stderr in a disclosure.
  - _error (download failed):_ inline under the row — "Couldn't download ffmpeg: <reason>. Retry, or set a path." Never blocks the whole wizard; the other binary can still resolve.
- **Density:** exactly two decisions on screen. The _bundled_ build skips detection entirely and shows a single confirming line ("Engine bundled ✓") — this screen only fully appears for the **light** build.

### S2 — Queue (Home)

```
┌──────────┬────────────────────────────────────────────────────────────┐
│ SIDEBAR  │  TOOLBAR:  [ Search ⌕ ]        N=2 ▾   [Start all][Pause all]│
│          ├────────────────────────────────────────────────────────────┤
│ [＋ Add] │  ▾ COLUMNS:  Title              Size     Status      ETA    │
│          │ ┌────────────────────────────────────────────────────────┐ │
│ QUEUE    │ │ ▣ Big Buck Bunny 1080p   1.4 GB  ▓▓▓▓▁▁ 44% 3.2MB/s 6m │ │ → eye first
│  ◈ All 8 │ │ ▣ Interview.webm         220 MB  ▓▓▓▓▓▓▓▁ 89% 1.1MB/s 40s│ │
│  ↓ Down 2│ │ ▣ Lecture 12             —      ⏸ Paused 0.2%        —  │ │
│  ‖ Queue3│ │ ▣ setup.exe              6.6 MB  ○ Queued            —  │ │
│  ⏸ Paus 1│ │ ▣ song.opus             7.2 MB  ✓ Completed          —  │ │
│  ✓ Done 1│ │ ▣ private.mp4            —       ⚠ Error: 403         ↻  │ │
│  ⚠ Fail 1│ │ …                                                      │ │
│          │ └────────────────────────────────────────────────────────┘ │
│ ⛃ Presets│  SELECTION BAR (when rows selected): 2 selected             │
│ ⚙ Setting│     [Start] [Pause] [Cancel] [Remove] [Move ▲▼]             │
└──────────┴────────────────────────────────────────────────────────────┘
```

- **Sidebar** (adapted rail): **+ Add** primary CTA pinned top; **Queue** group is the status filter tree (All / Active / Queued / Paused / Completed / Failed / Cancelled), each with a live count badge — this is the IDM category-tree pattern re-pointed at status. **Active** covers both `downloading` and `merging` (labelled "Active" rather than "Downloading" so the merge stage isn't orphaned); **Cancelled** is a first-class filter so cancelled items stay recoverable instead of only surfacing under All. **Presets** and **Settings** pinned bottom. Active filter is highlighted (weight + indicator, not color alone).
- **Toolbar:** search/filter-by-title, the **concurrency N** control (default 2, editable inline), and global **Start all / Pause all**. One row tall.
- **List (the core):** virtualized (SPEC: >50 items). Each row = selection checkbox, title (truncates with tooltip), size, an **inline progress region** carrying the pill bar + % + speed + stage token, and ETA. Stage token uses `label-mono` text _plus_ an icon (never color alone): `downloading / merging / queued / paused / completed / error / cancelled` (labels equal stage names; `cancelled` items appear under both the **All** and the dedicated **Cancelled** filter). → eye first: the topmost **active** (downloading) row — motion draws the eye, and that is correct.
- **Selection bar:** appears only when ≥1 row is selected; hosts bulk actions (start/pause/cancel/remove/reorder). Reorder also available by drag with a movement threshold.
- **Primary action on screen:** **+ Add** (sidebar). Row-level primary is "open detail" (click).
- **States:**
  - _empty (no downloads):_ centered — "No downloads yet. Paste a link or press **Add** to start." with the Add CTA echoed. The sidebar counts all read 0.
  - _empty (filtered):_ e.g. Failed filter with none — "Nothing failed. 🎉"-style neutral line, plus "Show all" to clear the filter. (No emoji in shipped UI — icon + text.)
  - _loading (restore on launch):_ queue rehydrates from SQLite; rows render immediately with last-known progress and a subtle "resuming…" token on items that were mid-download, replaced by live data as the backend reattaches.
  - _error (row-level):_ the row's stage token becomes `error` with a one-line reason (yt-dlp stderr, truncated); a **retry** affordance sits in the row; full stderr is in S5.
- **Density:** everything needed to triage a queue is 1-click (status, progress, speed, retry, bulk actions). **Buried:** per-item log, full address, format detail, resume capability → S5. Column config and N are 1-click but visually quiet.

### S3 — Add Download (overlay)

Progressive disclosure per design system: URL input starts large; on a valid paste it commits and the format + options regions unfold beneath.

```
┌───────────────────────────── Add Download ───────────────── [✕] ┐
│                                                                 │
│  URL(s)                                              → eye first│
│  ┌───────────────────────────────────────────────────────────┐ │
│  │  https://…                                     [ Probe ▸ ] │ │
│  └───────────────────────────────────────────────────────────┘ │
│  (paste a playlist and it expands to N items on Add)            │
│ ───────────────── revealed after successful probe ───────────── │
│  Format                                    [ S4 Format Picker ] │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │ ( ) 1080p mp4 · 1.4 GB   ( ) 720p · 780 MB   ( ) audio…    │ │
│  │ (•) Expression:  [ bv*[height<=1080]+ba/b            ]     │ │
│  └───────────────────────────────────────────────────────────┘ │
│  Preset   [ Archive ▾ ]   (default: Archive)  Manage presets…   │
│  ▸ Advanced   output template · proxy override · extra CLI args │
│                                                                 │
│                                   [ Cancel ]    [ Add to queue ]│
└─────────────────────────────────────────────────────────────────┘
```

- **Region 1 — URL(s):** large, focused on open. Accepts one URL or a playlist URL. **Probe** triggers `yt-dlp -F`. → eye first: this field (cursor auto-placed).
- **Region 2 — Format (S4):** hidden until probe succeeds. Offers a few **named quick picks** derived from probed formats **and** the always-present **raw expression field** — the two are one selectable group (picking a quick pick fills the expression; editing the expression deselects quick picks). This is the SPEC differentiator; the expression field is never hidden behind a second chevron.
- **Region 3 — Preset:** dropdown defaulting to the global default; "Manage presets…" jumps to S6. Applying a preset fills format/template/proxy/args (all still overridable here).
- **Region 4 — Advanced (buried):** chevron reveals output template, per-download proxy override, and the free-form extra-CLI-args field. Collapsed by default to keep the common path clean.
- **Primary action:** **Add to queue** — enabled once a URL is present (probe not strictly required; user may add blind with an expression + accept later error). Adds N items for a playlist.
- **States:**
  - _loading (probing):_ Probe button → spinner; format region shows skeleton rows; Add disabled until probe resolves or user dismisses.
  - _empty (no URL):_ only Region 1 visible; Add disabled.
  - _error (probe failed):_ format region replaced by an inline error carrying yt-dlp stderr verbatim (SPEC edge case: invalid expression / unreachable / proxy auth fail) + **Retry**; user may still expand Advanced and Add blind.
- **Density:** URL + format + preset are the whole common path, all 1-click. **Buried:** output template, proxy override, extra args (Advanced). Full format table (all resolutions/codecs) is one chevron deeper via **Format Picker**.

### S4 — Format Picker (region in S3; modal fallback)

```
┌────────────── Formats for “Big Buck Bunny” ───────────── [✕] ┐
│  ⌕ filter   [ ] video only  [ ] audio only  [ ] free-merge   │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ ID    RES     EXT   FPS  SIZE     CODEC        NOTE        │ │ → eye first (best row highlighted)
│ │ 137   1080p   mp4   30   1.4 GB   avc1         ✓ pick      │ │
│ │ 248   1080p   webm  30   980 MB   vp9                      │ │
│ │ 140   audio   m4a   —    130 MB   aac          bestaudio   │ │
│ │ …virtualized…                                             │ │
│ └──────────────────────────────────────────────────────────┘ │
│  Expression  [ 137+140                                 ]      │
│                                   [ Cancel ]   [ Use format ] │
└──────────────────────────────────────────────────────────────┘
```

- **Region 1 — filter chips:** video-only / audio-only / free-merge toggles + text filter over the probed table.
- **Region 2 — format table:** sortable columns (id, resolution, ext, fps, size, codec, note), virtualized (some sites return 100+ formats). Selecting rows composes the **expression** below (e.g. picking a video + an audio row yields `137+140`). → eye first: the recommended "best" row, pre-highlighted.
- **Region 3 — expression:** the same raw field mirrored from S3 — edits here flow back to S3.
- **Primary action:** **Use format** — writes the expression back into S3 and closes.
- **States:** inherits S3's probe result. _empty:_ "No formats returned — the site may require auth (backlog) or the URL is not a media page." _error:_ stderr passthrough + Retry.
- **Density:** the full format universe lives here so S3 stays quiet. Codec/fps/note columns are 1-click here, invisible in S3.

### S5 — Download Detail (right drawer)

Adapts IDM's floating detail surface as a docked drawer beside the live queue.

```
┌──────────────────────────── Detail ─────────────── [→ dock][✕]┐
│  Big Buck Bunny 1080p                          ▓▓▓▓▓it 44%     │ → eye first (title + bar)
│  ───────────────────────────────────────────────────────────  │
│  Address        https://…/watch?v=…              [copy]       │
│  Saving to      ~/Downloads/%(title)s.%(ext)s    [open dir]   │
│  Status         downloading                                   │
│  Size           1.42 GB          Downloaded   0.63 GB         │
│  Speed          3.2 MB/s         ETA          6 min           │
│  Resume         Yes (partial on disk)                         │
│  Format         137+140  ·  Preset: Archive                   │
│  ───────────────────────────────────────────────────────────  │
│  ▸ Log (yt-dlp stdout/stderr, tailing)                        │
│  ───────────────────────────────────────────────────────────  │
│              [ Pause ]   [ Cancel ]   [ Retry ]   [ Remove ]   │
└───────────────────────────────────────────────────────────────┘
```

- **Region 1 — header:** title + the same live pill bar (continuity with the row). → eye first.
- **Region 2 — facts grid:** address (copyable), resolved output path (open-dir), status, size/downloaded, speed/ETA, **resume capability** (SPEC-critical), and the format + preset used. `label-mono` for sizes/format/flags.
- **Region 3 — Log (buried):** collapsed disclosure that tails the process's stdout/stderr — this is where a curious or debugging user reads the raw truth. Off by default to keep the drawer scannable.
- **Region 4 — actions:** contextual — Pause/Resume toggles on state; Retry appears only for errored/cancelled; Remove always. Cancel/Remove are destructive → confirm + undo toast.
- **States:** mirrors the item. _error:_ status token `error`, the reason surfaces above the log, log auto-expands to the failing tail, Retry emphasized. _completed:_ actions collapse to **Open file / Open folder / Remove**.
- **Density:** the "why/where/how" of one item, all here so the row stays thin. **Buried:** raw log.

### S6 — Presets

```
┌──────────┬────────────────────────────────────────────────────┐
│ SIDEBAR  │  Presets                              [ + New preset ]│
│          ├────────────────────────────────────────────────────┤
│          │ ┌────────────────────────────────────────────────┐ │
│          │ │ ★ Archive   (default)   bv*[h<=1080]+ba/b   ⋯  │ │ → eye first (default)
│          │ │   Audio     bestaudio/best · %(title)s.%(ext)s ⋯│ │
│          │ │   4K        bv*[h<=2160]+ba/b               ⋯  │ │
│          │ └────────────────────────────────────────────────┘ │
│          │  EDITOR (inline, on select):                        │
│          │   Name [Archive]  ☑ default                         │
│          │   Format expr  [ bv*[height<=1080]+ba/b        ]    │
│          │   Output tmpl   [ %(title)s.%(ext)s            ]    │
│          │   Proxy         [ (inherit global)             ]    │
│          │   Extra args    [ --embed-thumbnail            ]    │
│          │                        [ Delete ]      [ Save ]     │
└──────────┴────────────────────────────────────────────────────┘
```

- **Region 1 — list:** named presets; the **default** is starred and sorts first (→ eye first). Row overflow (⋯): duplicate, set-default, delete.
- **Region 2 — inline editor:** selecting a preset opens the four editable fields (format expression, output template, proxy, extra args) + name + default toggle.
- **Primary action:** **Save** (per edit) / **+ New preset** (list level). Delete confirms; deleting the default promotes the next preset and warns.
- **States:** _empty:_ one seeded "Default" preset always exists (cannot delete the last one) — so a true-empty state never occurs; new installs show the seeded default selected. _error (bad expression):_ validated on Save via a dry `yt-dlp` parse; invalid expression blocks Save with stderr inline.
- **Density:** create/apply/set-default all 1-click. Nothing buried — presets are inherently a config surface.

### S7 — Settings

```
┌──────────┬────────────────────────────────────────────────────┐
│ SIDEBAR  │  Settings                                           │
│          ├────────────────────────────────────────────────────┤
│          │  Engine & health                        → eye first │
│          │   yt-dlp  ✓ /usr/bin/yt-dlp   ver 2025.x  [Change…] │
│          │   ffmpeg  ✓ /usr/bin/ffmpeg              [Re-check] │
│          │      [ Re-run onboarding ]                          │
│          │  Downloads                                          │
│          │   Parallel downloads (N)   [ 2  ▾ ]                 │
│          │   Default output dir       [ ~/Downloads    ][…]    │
│          │   Default filename tmpl    [ %(title)s.%(ext)s ]    │
│          │   Default preset           [ Archive ▾ ]            │
│          │  Network                                            │
│          │   Global proxy             [ socks5://…         ]   │
│          │  About                                              │
│          │   Build: light · BegireX 0.1.0 · yt-dlp 2025.x      │
└──────────┴────────────────────────────────────────────────────┘
```

- **Region 1 — Engine & health** (→ eye first): current binary paths + versions + **Re-run onboarding** (reopens S1). This keeps the SPEC's "re-runnable from settings" promise.
- **Region 2 — Downloads:** default N, default output dir + filename template, default preset (links to S6).
- **Region 3 — Network:** the global proxy (same value captured in S1).
- **Region 4 — About:** build flavor (bundled/light), app + engine versions. (Light/dark toggle is Backlog — dark ships default, so no toggle here in 001-core.)
- **States:** _error (binary went missing mid-session):_ the engine row flips to `not found` with a persistent banner across the app ("yt-dlp is no longer at its path — downloads are paused") and a **Fix** button that reopens S1. This satisfies the "binary missing mid-session" edge case.
- **Density:** the four things a user retunes (N, path, template, proxy) are flat and 1-click. Binary surgery is behind **Re-run onboarding**.

---

## 4. Key flows

Written as **user sees X → does Y → system responds Z**, with exact screen ids. Flow A is the SPEC kernel journey / demo-gate script.

### Flow A — Kernel journey (light build, cold start → resumed completion)

1. **User sees** S1 Onboarding, engine check showing `ffmpeg ✗ not found` (→ eye first). **Does:** clicks **Download for me** → **Download ffmpeg**. **System responds:** determinate bar fills; on success the row flips to `✓ found`, Continue enables. (Alt: **Set path…** → native picker → same resolution.)
2. **User** enters a proxy in Network → clicks **Continue**. **System** persists proxy globally, dismisses S1, lands on **S2** (empty state — "No downloads yet").
3. **User** clicks **＋ Add** (sidebar). **System** opens **S3** with the URL field focused.
4. **User** pastes a video URL → clicks **Probe**. **System** runs `yt-dlp -F`; format region (S4-in-panel) unfolds with quick picks + expression field.
5. **User** either selects **1080p mp4** _or_ types `bv*[height<=1080]+ba/b` in the expression field → picks a preset (the seeded **Default** on first run; "Archive" in SPEC's journey stands for whichever preset is selected) → clicks **Add to queue**. **System** creates the item, closes S3, the row appears in **S2** and begins downloading (live % / speed / stage).
6. **User** clicks **＋ Add** again, pastes a second URL, Adds. **System** — with **N=2** — runs **both in parallel**; both rows show live independent progress. (A third add would sit `Queued` until a slot frees.)
7. **User** quits the app mid-download (hard or clean). **System** has persisted queue + partial-progress to SQLite continuously.
8. **User** relaunches. **System** rehydrates **S2**: both rows render with last-known progress and a `resuming…` token; the backend re-spawns `yt-dlp -c`, each **resumes from its byte offset**, tokens flip back to live `downloading`.
9. **User** waits. **System** flips each row to `✓ Completed`; the file exists on disk at the templated path. Opening **S5** on a completed row shows **Open file / Open folder**. — _Demo gate passes._

### Flow B — Advanced format control (the differentiator)

1. **User sees** S3 after a probe, format region showing quick picks + expression. **Does:** clicks the **Format Picker** button. **System** opens **S4** with the full probed table, best row highlighted.
2. **User** filters to **video only**, sorts by size, selects format `248` (1080p vp9), then also selects audio `140`. **System** composes `248+140` into the expression field live.
3. **User** clicks **Use format**. **System** writes `248+140` back into S3's expression field (quick picks deselect) and closes S4.
4. **User** clicks **Add to queue**. **System** queues the item with that exact expression. _(If the expression were invalid, the item would error on start and S2's row would show the yt-dlp stderr with Retry — no silent failure.)_

### Flow C — Preset create then apply

1. **User** clicks **Presets** in sidebar → **S6**. **Does:** **+ New preset**. **System** opens the inline editor with blank fields.
2. **User** names it `4K`, sets expression `bv*[height<=2160]+ba/b`, adds extra args `--embed-thumbnail`, clicks **Save**. **System** dry-parses the expression via yt-dlp; valid → saves; the preset joins the list. (Invalid → Save blocked, stderr inline.)
3. **User** goes to **S2** → **＋ Add**, pastes a URL, opens the **Preset** dropdown, chooses **4K**. **System** fills S3's format expression + args from the preset (still overridable). **User** clicks **Add to queue** → item runs under the 4K config.

### Flow D — Recover a failed download

1. **User sees** S2 with a row showing `⚠ Error: 403` (→ the eye is pulled by the only non-green/idle token). **Does:** clicks the row → **S5** opens; the **Log** disclosure is auto-expanded to the failing tail (proxy auth failure).
2. **User** reads the stderr, realizes the proxy is wrong → opens **S7 Settings**, fixes the **Global proxy**. **System** saves.
3. **User** returns to **S5** (or the S2 row) → clicks **Retry**. **System** re-spawns yt-dlp with the corrected proxy; the row flips to `downloading`; resume (`-c`) picks up any partial bytes.

---

## 5. Density & hierarchy summary

| Screen | 1-click (surfaced)                                                                            | Buried (behind disclosure)                                                           |
| ------ | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| **S1** | Per-binary resolve (download/path), proxy                                                     | stderr of a failed in-app download                                                   |
| **S2** | Status filters, progress/speed/stage per row, retry, N, start/pause-all, bulk actions, search | Per-item log, full address, format detail, resume flag (→ S5); column config         |
| **S3** | URL, probe, quick-pick formats, **raw expression**, preset select, Add                        | Output template, proxy override, extra CLI args (Advanced); full format table (→ S4) |
| **S4** | Format table, filter chips, expression compose                                                | Codec/fps/note columns exist only here (invisible in S3)                             |
| **S5** | All item facts, resume capability, contextual actions                                         | Raw yt-dlp log tail                                                                  |
| **S6** | Create / apply / set-default / edit four fields                                               | — (config surface, nothing hidden)                                                   |
| **S7** | N, output dir, filename template, default preset, global proxy                                | Binary path surgery (→ Re-run onboarding / S1)                                       |

**Global hierarchy law:** the **queue list is the product**; every other surface is an overlay or a filter over it, and the eye should always be able to return to a running download in one `Esc`. The single primary action anywhere in the shell is **＋ Add**. Destructive actions (cancel/remove/delete) are always confirmed and always undoable via toast.

---

_Next: read this file and walk Flows A–D on paper. A step that feels wrong here will be wrong in the app — this is the cheapest moment to fix it. When it reads right, proceed to PRD.md (Step 2), then ARCHITECTURE.md (Step 3)._
