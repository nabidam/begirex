# SPEC — BegireX (001-core)

## 1. Core promise
A desktop GUI that gives yt-dlp's full download power — parallel queue, advanced format control, reusable presets — without touching a terminal.

## Kernel

The 5 features without which the product is pointless:

1. **Binary onboarding & health** — On first run (esp. the *light* build), detect `yt-dlp` and `ffmpeg`. If missing: let user set an explicit path, or download them in-app. Same step captures a global **proxy**. Re-runnable from settings. (The *bundled* build ships both and skips detection but keeps the settings surface.)
2. **Download queue** — Add one URL or a playlist (expands to N items). Run **N parallel** downloads (N configurable, default 2). Live per-item progress (%, speed, ETA, stage: downloading/merging/done/error). **Persists across restart**; partial downloads **resume** (`yt-dlp -c`).
3. **Advanced format selection** — Probe available formats (`yt-dlp -F`) into a picker AND expose a raw **format-selector expression** field (e.g. `bv*[height<=1080]+ba/b`). This is the differentiator; it is kernel or nothing.
4. **Config presets** — Save/apply named bundles of `{format expression, output template, proxy, extra CLI args}`. One preset is the global default; any download can override.
5. **Proxy & arbitrary CLI args** — Global proxy plus a free-form field for any yt-dlp flag, editable per-download and inside presets.

### Kernel journey (walking-skeleton + demo-gate script)
Launch light build → onboarding detects no ffmpeg → user clicks "Download" (or sets path) + enters proxy → paste a video URL → app probes formats → user picks 1080p via the picker *or* types a format expression → applies the "Archive" preset → clicks Add → item joins queue, a second URL added, **both download in parallel** with live progress → quit app mid-download → relaunch → queue restored, partial item **resumes** from where it stopped → completes; file is on disk at the templated path.

## 2. v1 (in scope)
Kernel above, plus: output dir + filename template config; playlist expansion; per-download override of format/preset/args; error state with retry; queue actions (pause/cancel/remove/reorder); two build flavors (bundled / light).

## 3. Backlog (ranked)
1. **Scheduling** (one-shot start-time, then bandwidth windows) — cut from v1 per scope challenge.
2. Cookies / auth for private videos.
3. Subtitle / thumbnail / metadata toggles as first-class UI (available now via raw args).
4. yt-dlp self-update button.
5. Per-download bandwidth limit; global concurrency scheduling.
6. macOS build.
7. Light/dark toggle (dark ships default).

## 4. Edge cases
Binary missing mid-session; invalid format expression (surface yt-dlp stderr); network drop → resumable; disk full; duplicate URL already queued; playlist with mixed availability; proxy auth failure; very long queue (virtualize list); app killed hard (no clean shutdown) → queue state must survive.

## 5. Non-functional + tech constraints
- **Stack locked:** Tauri 2 + Svelte 5 (frontend), Rust backend spawns yt-dlp as sidecar/child, streams stdout → parses progress. SQLite for queue + presets in app data dir.
- **Platforms:** Linux (AppImage/deb) + Windows. Handle per-OS paths and binary discovery.
- **WebKitGTK-safe:** no heavy backdrop blur (per design system); tonal layers for depth.
- Accessibility floor **WCAG AA**; full keyboard nav; visible focus rings.
- Progress parsing must not block UI; virtualize queue >50 items.

## 6. Tech stack
As locked above. No new heavy deps without cause (yt-dlp/ffmpeg are the engine; Rust owns process mgmt; SQLite via Tauri plugin).

## 7. Design direction
**Design system: `DESIGN_SYSTEM.md` (authoritative — purple Material handoff).** Personality: *efficient, precise, controlled*. Reference look: a high-end terminal rendered as clean GUI + Linear-grade density; native download-manager clarity. Desktop-dense, dark-first, fixed 240px sidebar collapsing to a rail. Accessibility floor WCAG AA. Deltas from design system: none pending.

## 8. Out of scope
No browser extension, no cloud/sync, no account system, no non-yt-dlp downloaders, no media playback/conversion beyond yt-dlp+ffmpeg's own merge, no daemon/background service (app must be open to download), no mobile.
