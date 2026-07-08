<script lang="ts">
  // S2 skeleton (ARCHITECTURE §10 Flow A steps 4-9 minus probe, TASKS.md T4).
  // Plain URL + expression input -> add_download -> live rows from the
  // queue store's progress/stage_changed subscriptions.
  import { queueStore } from "../stores/queue.svelte";

  // ponytail: preset system (T11) doesn't exist yet, so this hardcodes the
  // literal seeded in migrations/001_init.sql's `seed()` ('bv*+ba/b') as the
  // prefill instead of reading a Default preset dynamically. Upgrade path:
  // once T11 lands, read this from presets store's is_default row.
  const DEFAULT_FORMAT_EXPR = "bv*+ba/b";

  let url = $state("");
  let formatExpr = $state(DEFAULT_FORMAT_EXPR);
  let adding = $state(false);

  function formatBytes(bytes: number | null): string {
    if (bytes == null) return "?";
    const mb = bytes / (1024 * 1024);
    return mb >= 1 ? `${mb.toFixed(1)} MB` : `${(bytes / 1024).toFixed(0)} KB`;
  }

  function formatSpeed(bps: number | null): string {
    if (bps == null) return "";
    return `${(bps / (1024 * 1024)).toFixed(2)} MB/s`;
  }

  function formatEta(seconds: number | null): string {
    if (seconds == null) return "";
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  async function addClick() {
    if (!url.trim() || !formatExpr.trim()) return;
    adding = true;
    try {
      await queueStore.add({ url, format_expr: formatExpr });
      url = "";
    } catch {
      // error already surfaced via queueStore.error
    } finally {
      adding = false;
    }
  }

  // Plain row action buttons (T6 — full selection-bar/drag UI is T14).
  const ACTIVE_STAGES = new Set(["downloading", "merging"]);
  const TERMINAL_STAGES = new Set(["completed", "cancelled"]);
</script>

<main class="queue">
  <h1>BegireX</h1>

  <div class="add-row">
    <input type="text" bind:value={url} placeholder="Paste a URL…" />
    <input type="text" bind:value={formatExpr} placeholder="format expression" />
    <button disabled={adding} onclick={addClick}>Add</button>
  </div>

  {#if queueStore.error}
    <p class="error">{queueStore.error}</p>
  {/if}

  <ul class="items">
    {#each queueStore.items as item, index (item.id)}
      <li class="item">
        <span class="title">{item.title ?? item.url}</span>
        <span class="stage">{item.stage}</span>
        <span class="percent">{item.percent.toFixed(1)}%</span>
        <span class="bytes">{formatBytes(item.downloaded_bytes)} / {formatBytes(item.total_bytes)}</span>
        <span class="speed">{formatSpeed(item.speed_bps)}</span>
        <span class="eta">{formatEta(item.eta_seconds)}</span>
        {#if item.error_message}
          <span class="error">{item.error_message}</span>
        {/if}
        <span class="actions">
          {#if ACTIVE_STAGES.has(item.stage)}
            <button onclick={() => queueStore.pause(item.id)}>Pause</button>
          {:else if item.stage === "paused"}
            <button onclick={() => queueStore.resume(item.id)}>Resume</button>
          {:else if item.stage === "error"}
            <button onclick={() => queueStore.retry(item.id)}>Retry</button>
          {/if}
          {#if item.stage === "queued"}
            <button disabled={index === 0} onclick={() => queueStore.moveUp(item.id)}>▲</button>
            <button
              disabled={index === queueStore.items.length - 1}
              onclick={() => queueStore.moveDown(item.id)}
            >
              ▼
            </button>
          {/if}
          {#if !TERMINAL_STAGES.has(item.stage)}
            <button onclick={() => queueStore.cancel(item.id)}>Cancel</button>
          {/if}
          <button onclick={() => queueStore.remove(item.id)}>Remove</button>
        </span>
      </li>
    {:else}
      <li class="empty">No downloads yet.</li>
    {/each}
  </ul>
</main>

<style>
  .queue {
    max-width: 48rem;
    margin: 2rem auto;
    padding: 1.5rem;
  }
  .add-row {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1.25rem;
  }
  .add-row input {
    flex: 1;
  }
  input,
  button {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4rem 0.6rem;
    font-family: var(--font-sans);
  }
  button {
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .items {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .item {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    align-items: baseline;
    background: var(--card);
    color: var(--card-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.6rem 0.9rem;
    margin-bottom: 0.5rem;
  }
  .title {
    font-weight: 700;
    flex-basis: 100%;
  }
  .stage {
    font-family: var(--font-mono);
    color: var(--primary);
  }
  .empty {
    color: var(--muted-foreground);
  }
  .error {
    color: var(--error-token);
  }
  .actions {
    display: flex;
    gap: 0.4rem;
    flex-basis: 100%;
  }
  .actions button {
    padding: 0.2rem 0.5rem;
    font-size: 0.85em;
  }
</style>
