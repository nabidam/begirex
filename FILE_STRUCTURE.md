# FILE_STRUCTURE — BegireX

> Living document. The full tree of files that will exist at 001-core completion. Generated files marked *(gen)* — scaffolded or shadcn-copied, themed but not hand-authored. Chunk numbers refer to `specs/001-core/PLAN.md`.

```
begirex/
├── ARCHITECTURE.md                  # living: system contract
├── CONVENTIONS.md                   # living: how code is written
├── DESIGN.md                        # living: design-system adoption map
├── DESIGN_SYSTEM.md                 # authoritative token source (purple Material)
├── FILE_STRUCTURE.md                # this file
├── UX.md                            # living: screens + flows
├── specs/
│   └── 001-core/
│       ├── SPEC.md
│       ├── PRD.md
│       ├── PLAN.md
│       ├── TASKS.md                 # Phase 4 output (not yet written)
│       └── references/idm.png
├── package.json
├── package-lock.json
├── components.json                  # (gen) shadcn-svelte config          [C1]
├── vite.config.ts                   # (gen)                               [C1]
├── svelte.config.js                 # (gen)                               [C1]
├── tsconfig.json                    # (gen)                               [C1]
├── index.html                       # (gen)                               [C1]
├── .mcp.json
├── .gitignore
│
├── src/                             # Svelte 5 frontend
│   ├── main.ts                      # (gen) mount                          [C1]
│   ├── app.css                      # Tailwind + §2 token mapping (the ONLY
│   │                                #   file with raw values, copied from
│   │                                #   DESIGN_SYSTEM.md)                  [C1]
│   ├── App.svelte                   # root: stores + event listeners + route [C3,C9]
│   └── lib/
│       ├── ipc.ts                   # typed invoke/listen — sole @tauri-apps/api import [C3]
│       ├── types.ts                 # wire types mirroring ARCHITECTURE §7  [C3]
│       ├── stores/
│       │   ├── queue.svelte.ts      # items, activeDetailId, event application [C3]
│       │   ├── filters.svelte.ts    # status filter + search               [C9]
│       │   ├── presets.svelte.ts    #                                      [C8]
│       │   ├── settings.svelte.ts   #                                      [C3]
│       │   └── binaryHealth.svelte.ts #                                    [C11]
│       ├── views/
│       │   ├── Onboarding.svelte    # S1                                   [C3,C11]
│       │   ├── Shell.svelte         # sidebar + main-area chrome           [C9]
│       │   ├── Queue.svelte         # S2                                   [C3,C9]
│       │   ├── AddDownload.svelte   # S3 (dialog)                          [C6]
│       │   ├── FormatPicker.svelte  # S4 (dialog)                          [C7]
│       │   ├── DetailDrawer.svelte  # S5 (sheet)                           [C10]
│       │   ├── Presets.svelte       # S6                                   [C8]
│       │   └── Settings.svelte      # S7                                   [C11]
│       └── components/
│           ├── ui/…                 # (gen) shadcn-svelte copies (button, dialog,
│           │                        #   sheet, table, input, select, dropdown-menu,
│           │                        #   checkbox, toggle, progress, sonner, tooltip,
│           │                        #   collapsible, alert-dialog, card, badge) [C1+]
│           ├── Sidebar.svelte       # rail + filter tree + counts          [C9]
│           ├── QueueToolbar.svelte  # search, N, start/pause-all           [C9]
│           ├── QueueRow.svelte      # row + inline progress region         [C9]
│           ├── StageToken.svelte    # icon + label-mono chip               [C9]
│           ├── SelectionBar.svelte  # bulk actions                         [C9]
│           ├── VirtualList.svelte   # shared windowing (S2, S4)            [C7]
│           ├── FormatQuickPicks.svelte # S3 quick picks + expression group [C6]
│           ├── FactsGrid.svelte     # S5 facts                             [C10]
│           ├── LogDisclosure.svelte # S5 log tail                          [C10]
│           ├── BinaryRow.svelte     # S1/S7 binary status row              [C3,C11]
│           └── GlobalBanner.svelte  # binary-missing banner                [C11]
│
└── src-tauri/                       # Rust backend
    ├── Cargo.toml                   # (gen+deps: tokio, reqwest, thiserror, sqlx-or-rusqlite per tauri-plugin-sql) [C1]
    ├── Cargo.lock                   # (gen)
    ├── tauri.conf.json              # (gen; flavor variants)               [C1,C13]
    ├── build.rs                     # (gen; `bundled` feature wiring)      [C13]
    ├── capabilities/default.json    # (gen) Tauri 2 permissions            [C1]
    ├── icons/…                      # (gen) app icons                      [C1]
    ├── binaries/                    # bundled-flavor sidecars (yt-dlp, ffmpeg per target) [C13]
    ├── migrations/
    │   └── 001_init.sql             # DDL from ARCHITECTURE §3 + seed      [C1]
    ├── src/
    │   ├── main.rs                  # (gen) entry                          [C1]
    │   ├── lib.rs                   # app setup, plugin + handler registration [C1]
    │   ├── error.rs                 # AppError (ARCHITECTURE §7.1)         [C2]
    │   ├── ipc.rs                   # command handlers + event emitters    [C2–C11]
    │   ├── persistence.rs           # SQLite open/migrate/seed, CRUD, checkpoints, log ring buffer [C1,C5]
    │   ├── binary_manager.rs        # detect/set-path/download/health      [C2,C11]
    │   ├── engine_supervisor.rs     # spawn, stream, pause/cancel, probe, dry-parse [C2,C4,C6]
    │   ├── progress_parser.rs       # yt-dlp output → progress ticks       [C2]
    │   ├── queue_manager.rs         # scheduler, lifecycle, reorder, playlist expand [C3,C4,C12]
    │   ├── preset_service.rs        # CRUD + invariants                    [C8]
    │   └── settings_service.rs      # settings read/write                  [C2]
    └── tests/
        ├── engine_integration.rs    # real yt-dlp spawn (#[ignore] network) [C2]
        └── queue_lifecycle.rs       # pause/resume/cancel/reorder          [C4]
```

Not in tree by design: no `utils/` grab-bag, no frontend test suite (CONVENTIONS), no config files beyond the SQLite `settings` table (ARCHITECTURE §9), no CI workflow unless Chunk 13 opts in.
