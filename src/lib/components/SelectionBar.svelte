<script lang="ts">
  // SelectionBar (UX.md S2) — appears only when >=1 row is selected; hosts
  // bulk start/pause/cancel/remove/reorder. Cancel/Remove route through
  // Queue.svelte's confirm+undo-toast flow (ARCHITECTURE §8), same as each
  // row's own overflow menu — this component never calls the ipc/store
  // layer directly for those two verbs.
  let {
    count,
    onStart,
    onPause,
    onCancel,
    onRemove,
    onMoveUp,
    onMoveDown,
    onClear,
  }: {
    count: number;
    onStart: () => void;
    onPause: () => void;
    onCancel: () => void;
    onRemove: () => void;
    onMoveUp: () => void;
    onMoveDown: () => void;
    onClear: () => void;
  } = $props();
</script>

{#if count > 0}
  <div class="selection-bar" role="toolbar" aria-label="Bulk actions">
    <span class="count">{count} selected</span>
    <button type="button" onclick={onStart}>Start</button>
    <button type="button" onclick={onPause}>Pause</button>
    <button type="button" onclick={onCancel}>Cancel</button>
    <button type="button" onclick={onRemove}>Remove</button>
    <span class="move-group">
      <button type="button" aria-label="Move up" onclick={onMoveUp}>▲</button>
      <button type="button" aria-label="Move down" onclick={onMoveDown}>▼</button>
    </span>
    <button type="button" class="clear" onclick={onClear}>Clear selection</button>
  </div>
{/if}

<style>
  .selection-bar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 1rem;
    background: var(--surface-high);
    border-block: 1px solid var(--border);
  }
  .count {
    font-family: var(--font-mono);
    font-size: 0.85em;
    color: var(--muted-foreground);
    margin-inline-end: 0.5rem;
  }
  .move-group {
    display: flex;
    gap: 0.15rem;
  }
  .clear {
    margin-inline-start: auto;
    color: var(--muted-foreground);
  }
  button {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.35rem 0.6rem;
    font-family: var(--font-sans);
    font-size: 0.85em;
    cursor: pointer;
  }
  button:hover {
    background: var(--accent);
  }
  button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>
