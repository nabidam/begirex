<script lang="ts">
  // QueueRow (UX.md S2, TASKS.md T14) — one dense queue row: selection
  // checkbox, title, size, the inline-progress signature (StageToken + pill
  // bar + figures), ETA, and a row-overflow menu for contextual actions.
  // Drag-reorder and keyboard focus are orchestrated by Queue.svelte (needs
  // the full visible-row list); this component only reports pointer/keydown
  // events upward and renders whatever selection/focus state it's handed.
  import { queueStore } from "../stores/queue.svelte";
  import StageToken from "./StageToken.svelte";
  import type { Item } from "../types";

  let {
    item,
    selected,
    focused,
    onToggleSelect,
    onPointerDown,
    onArrow,
    onOpenDetail,
    onFocusRow,
    onCancelRequest,
    onRemoveRequest,
  }: {
    item: Item;
    selected: boolean;
    focused: boolean;
    onToggleSelect: (id: number) => void;
    onPointerDown: (id: number, event: PointerEvent) => void;
    onArrow: (direction: "up" | "down") => void;
    onOpenDetail: (id: number) => void;
    onFocusRow: (id: number) => void;
    onCancelRequest: (ids: number[]) => void;
    onRemoveRequest: (ids: number[]) => void;
  } = $props();

  const ACTIVE_STAGES = new Set(["downloading", "merging"]);
  const TERMINAL_STAGES = new Set(["completed", "cancelled"]);

  let menuOpen = $state(false);
  let rowEl: HTMLDivElement | undefined = $state();

  function formatBytes(bytes: number | null): string {
    if (bytes == null) return "—";
    const mb = bytes / (1024 * 1024);
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb.toFixed(0)} MB`;
  }

  function formatSpeed(bps: number | null): string {
    if (bps == null) return "";
    return `${(bps / (1024 * 1024)).toFixed(2)} MB/s`;
  }

  function formatEta(seconds: number | null): string {
    if (seconds == null) return "—";
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      onArrow("down");
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      onArrow("up");
    } else if (e.key === "Enter") {
      e.preventDefault();
      onOpenDetail(item.id);
    }
  }

  function closeMenu() {
    menuOpen = false;
  }

  $effect(() => {
    if (!menuOpen) return;
    function onWindowPointerDown(e: PointerEvent) {
      if (rowEl && !rowEl.contains(e.target as Node)) closeMenu();
    }
    window.addEventListener("pointerdown", onWindowPointerDown);
    return () => window.removeEventListener("pointerdown", onWindowPointerDown);
  });
</script>

<div
  bind:this={rowEl}
  class="row"
  class:selected
  class:focused
  data-row-id={item.id}
  role="row"
  aria-selected={selected}
  tabindex={focused ? 0 : -1}
  onkeydown={handleKeydown}
  onfocus={() => onFocusRow(item.id)}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span class="cell checkbox" onpointerdown={(e) => e.stopPropagation()}>
    <input
      type="checkbox"
      checked={selected}
      onchange={() => onToggleSelect(item.id)}
      aria-label="Select {item.title ?? item.url}"
    />
  </span>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="cell surface" onpointerdown={(e) => onPointerDown(item.id, e)}>
    <span class="title" title={item.title ?? item.url}>{item.title ?? item.url}</span>
    <span class="size mono">{formatBytes(item.total_bytes)}</span>

    <div class="progress-region">
      <StageToken stage={item.stage} />
      <div class="bar-track" class:thick={ACTIVE_STAGES.has(item.stage)}>
        <div class="bar-fill" style:width="{Math.min(100, Math.max(0, item.percent))}%"></div>
      </div>
      <span class="figures mono">{item.percent.toFixed(0)}%</span>
      {#if item.speed_bps != null}<span class="figures mono">{formatSpeed(item.speed_bps)}</span>{/if}
      {#if item.stage === "error" && item.error_message}
        <span class="error-text">{item.error_message}</span>
        <button type="button" class="retry-btn" onclick={() => queueStore.retry(item.id)}>Retry</button>
      {/if}
    </div>

    <span class="eta mono">{formatEta(item.eta_seconds)}</span>
  </div>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span class="cell actions" onpointerdown={(e) => e.stopPropagation()}>
    <button
      type="button"
      class="icon-btn"
      onclick={() => (menuOpen = !menuOpen)}
      aria-label="Row actions"
      aria-expanded={menuOpen}
    >
      ⋯
    </button>
    {#if menuOpen}
      <ul class="menu" role="menu">
        {#if ACTIVE_STAGES.has(item.stage)}
          <li><button type="button" onclick={() => { queueStore.pause(item.id); closeMenu(); }}>Pause</button></li>
        {:else if item.stage === "paused"}
          <li><button type="button" onclick={() => { queueStore.resume(item.id); closeMenu(); }}>Resume</button></li>
        {:else if item.stage === "error"}
          <li><button type="button" onclick={() => { queueStore.retry(item.id); closeMenu(); }}>Retry</button></li>
        {/if}
        {#if item.stage === "queued"}
          <li><button type="button" onclick={() => { queueStore.moveUp(item.id); closeMenu(); }}>Move up</button></li>
          <li><button type="button" onclick={() => { queueStore.moveDown(item.id); closeMenu(); }}>Move down</button></li>
        {/if}
        {#if !TERMINAL_STAGES.has(item.stage)}
          <li><button type="button" onclick={() => { onCancelRequest([item.id]); closeMenu(); }}>Cancel</button></li>
        {/if}
        <li><button type="button" onclick={() => { onRemoveRequest([item.id]); closeMenu(); }}>Remove</button></li>
      </ul>
    {/if}
  </span>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 2rem 1fr auto;
    align-items: center;
    gap: 0.5rem;
    height: 100%;
    padding-inline: 0.6rem;
    border-radius: var(--radius);
    border: 1px solid transparent;
  }
  .row:hover {
    background: var(--accent);
  }
  .row.selected {
    background: var(--secondary);
    color: var(--secondary-foreground);
  }
  .row.focused {
    border-color: var(--ring);
  }
  .row:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }
  .cell {
    display: flex;
    align-items: center;
  }
  .checkbox input {
    width: 1rem;
    height: 1rem;
    accent-color: var(--primary);
  }
  .surface {
    display: grid;
    grid-template-columns: minmax(6rem, 1fr) 4.5rem minmax(12rem, 2fr) 3.5rem;
    align-items: center;
    gap: 0.75rem;
    min-width: 0;
    cursor: pointer;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.9em;
  }
  .size,
  .eta {
    font-size: 0.8em;
    color: var(--muted-foreground);
    text-align: end;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .progress-region {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }
  .bar-track {
    flex: 1;
    min-width: 3rem;
    height: 4px;
    border-radius: 999px;
    background: var(--muted);
    overflow: hidden;
  }
  .bar-track.thick {
    height: 8px;
  }
  .bar-fill {
    height: 100%;
    background: var(--primary);
    border-radius: 999px;
    transition: width 200ms linear;
  }
  .figures {
    font-size: 0.78em;
    color: var(--muted-foreground);
    flex-shrink: 0;
  }
  .error-text {
    font-family: var(--font-mono);
    font-size: 0.78em;
    color: var(--error-token);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .retry-btn {
    flex-shrink: 0;
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.1rem 0.5rem;
    font-size: 0.78em;
    cursor: pointer;
  }
  .retry-btn:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .actions {
    position: relative;
  }
  .icon-btn {
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    padding: 0.2rem 0.5rem;
    font-size: 1em;
    border-radius: var(--radius);
  }
  .icon-btn:hover {
    background: var(--accent);
  }
  .icon-btn:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .menu {
    position: absolute;
    inset-inline-end: 0;
    inset-block-start: 100%;
    z-index: 10;
    list-style: none;
    margin: 0.2rem 0 0;
    padding: 0.25rem;
    min-width: 8rem;
    background: var(--surface-high);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }
  .menu button {
    width: 100%;
    text-align: start;
    background: transparent;
    color: var(--foreground);
    border: none;
    border-radius: var(--radius);
    padding: 0.35rem 0.5rem;
    font-size: 0.85em;
    font-family: var(--font-sans);
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--accent);
  }
  .menu button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }
</style>
