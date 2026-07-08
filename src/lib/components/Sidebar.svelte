<script lang="ts">
  // Sidebar (UX.md S2, DESIGN.md gap #5) — + Add CTA, the status filter tree
  // (live count badges), Presets/Settings pinned bottom. Collapses to a
  // ~56px icon rail below ~1100px window width or by manual toggle
  // (DESIGN.md §6). No shadcn `tooltip` component exists in this repo (no
  // shadcn/Tailwind pipeline, per T10/T11's precedent) — collapsed labels
  // fall back to the native `title` attribute.
  // ponytail: unicode glyphs stand in for lucide-svelte icons (not
  // installed, same no-new-dependency precedent as every prior task).
  // Upgrade path: swap for `lucide-svelte` if/when the icon set grows.
  import { filtersStore, STATUS_FILTERS, type StatusFilter } from "../stores/filters.svelte";
  import type { Item } from "../types";

  let { items, onAdd, onOpenPresets, onOpenSettings, collapsed, addDisabled = false }: {
    items: Item[];
    onAdd: () => void;
    onOpenPresets: () => void;
    onOpenSettings: () => void;
    collapsed: boolean;
    addDisabled?: boolean;
  } = $props();

  const ICON: Record<StatusFilter, string> = {
    all: "◈",
    downloading: "↓",
    queued: "‖",
    paused: "⏸",
    completed: "✓",
    failed: "⚠",
  };

  const LABEL: Record<StatusFilter, string> = {
    all: "All",
    downloading: "Downloading",
    queued: "Queued",
    paused: "Paused",
    completed: "Completed",
    failed: "Failed",
  };
</script>

<nav class="sidebar" class:collapsed aria-label="Queue navigation">
  <button
    type="button"
    class="collapse-toggle"
    onclick={() => filtersStore.toggleCollapsed()}
    aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
    title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
  >
    <span class="glyph" aria-hidden="true">{collapsed ? "»" : "«"}</span>
  </button>

  <button
    type="button"
    class="add-btn"
    onclick={onAdd}
    disabled={addDisabled}
    title={addDisabled
      ? "Set up yt-dlp/ffmpeg in Settings to enable downloads."
      : collapsed
        ? "Add"
        : undefined}
  >
    <span class="glyph" aria-hidden="true">＋</span>
    {#if !collapsed}<span>Add</span>{/if}
  </button>

  <ul class="filter-tree">
    {#each STATUS_FILTERS as filter (filter)}
      {@const count = filtersStore.countFor(filter, items)}
      {@const active = filtersStore.status === filter}
      <li>
        <button
          type="button"
          class="filter-row"
          class:active
          onclick={() => filtersStore.setStatus(filter)}
          title={collapsed ? LABEL[filter] : undefined}
          aria-current={active ? "true" : undefined}
        >
          <span class="glyph" aria-hidden="true">{ICON[filter]}</span>
          {#if !collapsed}<span class="label">{LABEL[filter]}</span>{/if}
          <span class="count">{count}</span>
        </button>
      </li>
    {/each}
  </ul>

  <div class="pinned">
    <button type="button" class="pinned-row" onclick={onOpenPresets} title={collapsed ? "Presets" : undefined}>
      <span class="glyph" aria-hidden="true">⛃</span>
      {#if !collapsed}<span>Presets</span>{/if}
    </button>
    <button
      type="button"
      class="pinned-row"
      onclick={onOpenSettings}
      title={collapsed ? "Settings (Ctrl/Cmd+,)" : "Settings (Ctrl/Cmd+,)"}
    >
      <span class="glyph" aria-hidden="true">⚙</span>
      {#if !collapsed}<span>Settings</span>{/if}
    </button>
  </div>
</nav>

<style>
  .sidebar {
    width: 240px;
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 0.75rem;
    background: var(--card);
    color: var(--card-foreground);
    border-inline-end: 1px solid var(--border);
    overflow-y: auto;
  }
  .sidebar.collapsed {
    width: 56px;
    padding-inline: 0.4rem;
    align-items: center;
  }
  button {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    background: transparent;
    color: inherit;
    border: 1px solid transparent;
    border-radius: var(--radius);
    padding: 0.4rem 0.6rem;
    font-family: var(--font-sans);
    font-size: 0.9em;
    cursor: pointer;
    text-align: start;
  }
  .collapsed button {
    justify-content: center;
    padding: 0.5rem;
  }
  button:hover {
    background: var(--accent);
  }
  button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .add-btn {
    background: var(--primary);
    color: var(--primary-foreground);
    font-weight: 700;
    justify-content: center;
  }
  .collapse-toggle {
    justify-content: center;
    color: var(--muted-foreground);
  }
  .glyph {
    width: 1.2em;
    text-align: center;
    font-family: var(--font-mono);
  }
  .filter-tree {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  .filter-row {
    border-inline-start: 2px solid transparent;
  }
  .filter-row.active {
    font-weight: 700;
    border-inline-start-color: var(--primary);
    background: var(--accent);
  }
  .label {
    flex: 1;
  }
  .count {
    font-family: var(--font-mono);
    font-size: 0.85em;
    color: var(--muted-foreground);
    background: var(--muted);
    border-radius: 999px;
    padding: 0 0.4rem;
    min-width: 1.4em;
    text-align: center;
  }
  .collapsed .count {
    display: none;
  }
  .pinned {
    margin-block-start: auto;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
</style>
