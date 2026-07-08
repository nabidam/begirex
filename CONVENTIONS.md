# CONVENTIONS — BegireX

> Living document. Read by every implementation task. Keep under 2 pages — every line here is context each future task pays for.

## Naming

- **Rust:** modules/files `snake_case` matching ARCHITECTURE.md module names exactly (`queue_manager.rs`, `engine_supervisor.rs`, `binary_manager.rs`, `preset_service.rs`, `settings_service.rs`, `persistence.rs`, `ipc.rs`, `error.rs`). Types `PascalCase`, fields/fns `snake_case`. IPC command fn names = wire names (`add_download`, `probe_formats`).
- **TypeScript/Svelte:** components `PascalCase.svelte`; stores `camelCase.svelte.ts` under `src/lib/stores/`; one store per ARCHITECTURE §2 store name (`queue`, `filters`, `presets`, `settings`, `binaryHealth`, `activeDetailId` lives in `queue`). Wire types mirror ARCHITECTURE §7 shapes verbatim in `src/lib/types.ts` — snake_case fields on the wire, no renaming layer.
- **DB:** exactly the DDL in ARCHITECTURE §3. Schema changes = new numbered migration in `src-tauri/migrations/`, never edit an applied one.
- **Screens:** refer to surfaces by UX id (S1–S7) in comments, commits, and task text.

## Error handling

- **Rust:** one `AppError` enum (`error.rs`) matching ARCHITECTURE §7.1 codes exactly; implement via `thiserror`. Every command returns `Result<T, AppError>`. Engine stderr goes in the `stderr` field verbatim — never paraphrase yt-dlp.
- **Engine failures are data, not exceptions:** non-zero exit → stage `error` + `error_message`; never `panic!`/`unwrap` on child-process or DB paths (`expect` allowed only in setup code that should abort launch).
- **Frontend:** ipc client rethrows typed `AppError`; views map `code` → inline message or toast. No swallowed promises — every `invoke` is awaited and errors surfaced.
- **Validation at the trust boundary only** (ipc layer): types, `n ≥ 1`, non-empty url/name. Modules assume validated input.

## Folder rules

- `src-tauri/src/` — flat modules per ARCHITECTURE §2; no `mod.rs` trees until a module exceeds ~500 lines.
- `src/lib/ipc.ts` — the **only** file importing `@tauri-apps/api`.
- `src/lib/views/` — one file per screen/overlay (S1–S7). `src/lib/components/` — shared pieces used by ≥2 views or extracted for size. shadcn-svelte copies live in `src/lib/components/ui/` (generated — don't hand-edit beyond theming).
- No file > ~400 lines without a split; no util/helpers grab-bag files — colocate helpers with their one caller until a second caller exists.

## Styling

- Tokens only, via the DESIGN.md → shadcn CSS-variable mapping. **No raw hex/px/font values in components** — if a needed token is missing, add it to the mapping in `src/app.css` + DESIGN.md first.
- Logical properties (`margin-inline`, `padding-block`) per DESIGN_SYSTEM.md; no `left/right` CSS.

## Tests

- **Rust:** unit tests inline (`#[cfg(test)] mod tests`) for pure logic — progress parser, scheduler pick-next, preset invariants, expression pass-through. Integration tests in `src-tauri/tests/` may spawn real yt-dlp; mark network-dependent ones `#[ignore]` and run them at demo gates.
- **Frontend:** no component test suite in 001-core (demo gates + Vitest for pure ts logic only, e.g. expression compose). Don't add testing-library/playwright without a decision.
- Test names describe the behavior: `resume_reports_bytes_gte_checkpoint`, not `test_resume2`.

## Commits

- Conventional commits: `feat(scope): …`, `fix:`, `chore:`, `docs:`; scope = module or screen id (`feat(queue_manager): …`, `feat(s3): …`). One chunk ≈ 1–3 commits; demo gate passes get `chore(gate): demo gate N passed`.
- Living docs (ARCHITECTURE/UX/DESIGN/FILE_STRUCTURE/CONVENTIONS) are patched **in the same commit** as the code that makes them true.

## Process

- Backend is the single source of truth (ARCHITECTURE boundary law). If a frontend task seems to need durable state, the task is wrong — fix the contract first.
- Deliberate shortcuts get a `ponytail:` comment naming the ceiling and upgrade path.
