# DESIGN — BegireX (adoption map)

> Living document. **Single source of truth for every exact value is `DESIGN_SYSTEM.md`** (purple Material handoff — authoritative per SPEC §7). This file never repeats a hex/px/rem value; it maps design-system tokens onto shadcn-svelte's theme variables and onto the screens UX.md defines. If a value seems missing, add it to DESIGN_SYSTEM.md, then reference it here by name.

## 1. Direction

- **Adjectives:** efficient, precise, controlled (SPEC §7).
- **References:** a high-end terminal rendered as clean GUI; Linear-grade density; native download-manager clarity (IDM structure per UX.md, never its skin).
- **Visual signature:** the **inline progress pill** — a fully-rounded `primary`-filled bar living inside every queue row (taller variant on the active download), paired with `label-mono` stage tokens. Progress is read in-row, never in a separate panel.
- Dark-only in 001-core (light mode deferred per DESIGN_SYSTEM.md; no toggle — SPEC backlog #7).

## 2. Token adoption — shadcn-svelte CSS variables

Set in `src/app.css` (`:root`), values copied from DESIGN_SYSTEM.md frontmatter by token name. This table is the complete mapping; components consume only shadcn variables or Tailwind classes bound to them.

| shadcn variable | DESIGN_SYSTEM token |
|---|---|
| `--background` | `surface` |
| `--foreground` | `on-surface` |
| `--card` | `surface-container-low` |
| `--card-foreground` | `on-surface` |
| `--popover` | `surface-container-high` |
| `--popover-foreground` | `on-surface` |
| `--primary` | `primary` |
| `--primary-foreground` | `on-primary` |
| `--secondary` | `secondary-container` |
| `--secondary-foreground` | `on-secondary-container` |
| `--muted` | `surface-container` |
| `--muted-foreground` | `on-surface-variant` |
| `--accent` | `surface-container-highest` |
| `--accent-foreground` | `on-surface` |
| `--destructive` | `error-container` |
| `--destructive-foreground` | `on-error-container` |
| `--border` | `outline-variant` |
| `--input` | `outline-variant` |
| `--ring` | `primary` |
| `--radius` | `rounded.DEFAULT` |

Additional custom properties (same file, same naming style) for roles shadcn lacks:

| Custom property | DESIGN_SYSTEM token | Used for |
|---|---|---|
| `--surface-lowest` | `surface-container-lowest` | base layer behind the queue list |
| `--surface-high` | `surface-container-high` | top layer: modals, drawers, popovers (+1px `--border` edge) |
| `--warning` / `--warning-foreground` | `tertiary` / `on-tertiary` | paused/queued-attention accents, GlobalBanner |
| `--error-token` / `--error-token-foreground` | `error` / `on-error` | stage-token error text on dark rows |

**Typography:** Instrument Sans + JetBrains Mono packaged locally; Tailwind `font-sans` → Instrument Sans, `font-mono` → JetBrains Mono. Type roles used exactly as DESIGN_SYSTEM.md defines: `display` (view titles), `headline` (section headers, drawer title), `body-lg` (onboarding copy), `body-sm` (default UI text), `label-mono` (sizes, formats, CLI flags, stage tokens, versions, log), `caption` (count badges, hints).

**Spacing / radii / shape:** 4px baseline and `space-*` names from DESIGN_SYSTEM.md; buttons+inputs `rounded.DEFAULT`, cards+queue rows `rounded.lg`, progress bars `rounded.full` (pill). Element gaps default `space-md`.

**Motion:** DESIGN_SYSTEM.md defines no motion tokens — adopt shadcn-svelte/bits-ui defaults (their built-in enter/exit transitions) and Tailwind's default easing; durations ≤200ms; progress-bar width changes transition linearly. No bespoke animation in 001-core. Respect `prefers-reduced-motion` (transitions off).

**Elevation:** tonal layers per DESIGN_SYSTEM.md (lowest → container → high + 1px border). **No backdrop blur, no heavy shadows** (WebKitGTK, NFR-7); overlay scrims are plain translucent `surface-container-lowest`.

## 3. Component inventory — shadcn-svelte serves UX.md

Add via shadcn MCP; theme comes free through §2.

| UX element (screen) | shadcn-svelte component |
|---|---|
| S3 Add Download overlay | `dialog` (centered sheet) |
| S4 Format Picker | `dialog` + `table` |
| S5 Detail drawer | `sheet` (right side) |
| Buttons everywhere | `button` (default=primary solid, `outline`=ghost secondary, `ghost` row actions, `destructive`) |
| Inputs (URL, expression, proxy, template, args, search) | `input`; expression/args get `font-mono` |
| Preset / N / default-preset selects | `select` |
| Row overflow (⋯), selection-bar menus | `dropdown-menu` |
| Row selection, S6 default toggle, S4 filter chips | `checkbox` / `toggle` |
| Progress (rows, S1 binary fetch, S5 header) | `progress` (restyled to pill heights per DESIGN_SYSTEM Progress Bars) |
| Undo/confirm toasts | `sonner` (toast) |
| Truncated-title hover, rail icon labels | `tooltip` |
| Advanced disclosure (S3), Log (S5) | `collapsible` |
| Confirmations (cancel/remove/delete-preset) | `alert-dialog` |
| S1 wizard framing, S6/S7 section cards | `card` |
| Sidebar count badges, stage token chrome | `badge` (token variant per §5) |
| Icons | `lucide-svelte` (ARCHITECTURE: supersedes Material Symbols) |

## 4. Gap list — needs shadcn-svelte can't serve

Flagged per workflow; substitutes below are the proposal and need user sign-off at the Phase-3 human pass before implementation.

1. **VirtualList** (S2 >50 rows, S4 100+ formats) — no shadcn virtualizer. Proposal: one small hand-rolled windowing component (`src/lib/components/VirtualList.svelte`, fixed row height) — no new dependency.
2. **StageToken** — icon + `label-mono` text chip per stage; composed from `badge` + lucide icon. Labels equal stage names: `downloading / merging / queued / paused / completed / error / cancelled`. Never color alone (NFR-4): each token = icon + text; colors: active states `primary`, paused/queued `on-surface-variant`, done `secondary`, error `--error-token`, cancelled `on-surface-variant`.
3. **Inline row progress region** (the signature) — composed from `progress` + `label-mono` figures; standard row uses the thin bar, the active download the thick bar (heights per DESIGN_SYSTEM Progress Bars).
4. **Drag-reorder with movement threshold** (V4-AC3) — no shadcn primitive. Proposal: native pointer events, ~6px threshold, no dnd library.
5. **Sidebar filter tree + collapsing rail** — layout composed from `button`/`badge`/`tooltip`; collapse behavior hand-written.
6. **Thumbnails:** DESIGN_SYSTEM.md's queue-item sketch mentions a thumbnail; UX.md's S2 row (authoritative for structure) has none. **Resolution: no thumbnails in 001-core** — row = checkbox · title · size · progress region · ETA. Revisit post-v1 if wanted.

## 5. Component states

Every interactive element (buttons, inputs, selects, checkboxes, rows, sidebar items, chips):
- **default** → per §2 mapping; **hover** → background lightens one tonal step (DESIGN_SYSTEM Buttons rule; e.g. `surface-container` → `surface-container-high`); **focus-visible** → 2px solid `--ring` ring, 2px offset, always visible (never `outline: none` without replacement); **active** → 1px translate-down (DESIGN_SYSTEM); **disabled** → reduced opacity + `not-allowed` cursor, still AA-readable.
- Queue row adds: **selected** (checkbox on + `surface-container-high` background) and **focused** (keyboard ring, distinct from selected).

Every data view (queue list, format table, preset list, log tail, binary rows):
- **empty** → per PRD §8 copy, icon + text, one action (e.g. "Show all", Add CTA).
- **loading** → skeleton rows (S3/S4 probe), spinner token (S1 detection), rehydrate-with-checkpoint + `resuming…` token (S2 launch).
- **error** → yt-dlp stderr verbatim in `label-mono` + Retry; never a bare "something went wrong".

## 6. Layout

- Desktop-only: min window ~960×600. Sidebar fixed 240px (DESIGN_SYSTEM Desktop Strategy), collapsing to ~56px icon rail below ~1100px window width or by toggle (UX §2).
- Main area fluid; settings pages use the 12-column grid, queue is a single-column list (DESIGN_SYSTEM Grid). Content max-width in S6/S7 forms: 12-column with fields spanning ≤8 columns; `layout-margin` safe zones on large windows.
- Density: rows are compact (Linear-grade) — one line per queue row, `body-sm` default, `space-sm` intra-row gaps, `space-md` between regions.
- Logical properties only (RTL-ready per DESIGN_SYSTEM).

## 7. Hard rules

1. Tokens only in components — no raw hex/px/font values anywhere outside the §2 mapping block in `src/app.css`. (CONVENTIONS enforces.)
2. WCAG AA contrast on every pair (NFR-4); verify at demo gates.
3. Focus always visible; keyboard order = visual order.
4. No state by color alone — stage tokens always icon + text.
5. No backdrop blur / heavy shadows (NFR-7).
6. No template clichés: no gradient hero, no glassmorphism, no emoji in shipped UI (UX §3 S2 note).
7. Single-source rule: exact values live only in DESIGN_SYSTEM.md; this doc and all code refer by token name.
