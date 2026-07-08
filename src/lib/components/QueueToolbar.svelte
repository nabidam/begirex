<script lang="ts">
  // Toolbar (UX.md S2) — title search, inline concurrency (N) control, and
  // global Start all / Pause all, all operating on the *visible* (filtered)
  // queue per UX.md's "operate on the visible queue".
  import { filtersStore } from "../stores/filters.svelte";
  import { queueStore } from "../stores/queue.svelte";
  import { settingsStore } from "../stores/settings.svelte";
  import type { Item } from "../types";

  let { visibleItems }: { visibleItems: Item[] } = $props();

  const PAUSABLE = new Set(["downloading", "merging", "queued"]);

  const pauseIds = $derived(visibleItems.filter((i) => PAUSABLE.has(i.stage)).map((i) => i.id));
  const resumeIds = $derived(visibleItems.filter((i) => i.stage === "paused").map((i) => i.id));

  const displayedN = $derived(queueStore.concurrency ?? settingsStore.settings?.default_concurrency ?? 2);

  function onNInput(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    if (Number.isInteger(value) && value >= 1) {
      queueStore.setConcurrency(value);
    }
  }
</script>

<div class="toolbar">
  <input
    type="search"
    class="search"
    placeholder="Search title…"
    value={filtersStore.search}
    oninput={(e) => filtersStore.setSearch((e.currentTarget as HTMLInputElement).value)}
    aria-label="Search queue by title"
  />

  <label class="n-control">
    <span>N</span>
    <input type="number" min="1" value={displayedN} onchange={onNInput} aria-label="Concurrent downloads" />
  </label>

  <div class="bulk-actions">
    <button type="button" disabled={resumeIds.length === 0} onclick={() => queueStore.resumeAll(resumeIds)}>
      Start all
    </button>
    <button type="button" disabled={pauseIds.length === 0} onclick={() => queueStore.pauseAll(pauseIds)}>
      Pause all
    </button>
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 1rem;
    border-block-end: 1px solid var(--border);
  }
  .search {
    flex: 1;
    min-width: 0;
    max-width: 24rem;
  }
  input {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4rem 0.6rem;
    font-family: var(--font-sans);
  }
  input:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .n-control {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85em;
    color: var(--muted-foreground);
  }
  .n-control input {
    width: 3.5rem;
    font-family: var(--font-mono);
  }
  .bulk-actions {
    display: flex;
    gap: 0.4rem;
    margin-inline-start: auto;
  }
  button {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4rem 0.7rem;
    font-family: var(--font-sans);
    cursor: pointer;
  }
  button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
