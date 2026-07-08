<script lang="ts">
  // S5 — Download Detail drawer (UX.md, TASKS.md T15). Docked beside the
  // live queue (never a scrim/modal — ARCHITECTURE/UX §2 "S3 and S5 are
  // overlays over S2... sits beside S5"), opened via queueStore.openDetail
  // (T14's row click/Enter) and closed via ✕ or Esc. Mounted once,
  // unconditionally, from Shell.svelte so its Esc listener registers before
  // AddDownload's (S3) — UX §2 "Esc closes the topmost overlay (S5 → S3)".
  import { onMount } from "svelte";
  import { queueStore } from "../stores/queue.svelte";
  import { presetsStore } from "../stores/presets.svelte";
  import { settingsStore } from "../stores/settings.svelte";
  import { openPath } from "../ipc";
  import StageToken from "../components/StageToken.svelte";
  import FactsGrid from "../components/FactsGrid.svelte";
  import LogDisclosure from "../components/LogDisclosure.svelte";

  const ACTIVE_STAGES = new Set(["downloading", "merging"]);
  const TERMINAL_STAGES = new Set(["completed", "cancelled"]);
  const TOAST_WINDOW_MS = 5000;

  const item = $derived(
    queueStore.activeDetailId != null
      ? queueStore.items.find((i) => i.id === queueStore.activeDetailId) ?? null
      : null,
  );
  const presetName = $derived(
    item?.preset_id != null
      ? presetsStore.presets.find((p) => p.id === item.preset_id)?.name ?? null
      : null,
  );

  let toast = $state<{ message: string; undo: () => void } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  let removeTimer: ReturnType<typeof setTimeout> | undefined;

  // Item removed (e.g. a row-level Remove elsewhere) while its drawer is
  // open — nothing left to show, so close.
  $effect(() => {
    if (queueStore.activeDetailId != null && item === null) {
      queueStore.closeDetail();
    }
  });

  function close() {
    queueStore.closeDetail();
  }

  function showUndoToast(message: string, undo: () => void) {
    clearTimeout(toastTimer);
    toast = {
      message,
      undo: () => {
        undo();
        toast = null;
      },
    };
    toastTimer = setTimeout(() => {
      toast = null;
    }, TOAST_WINDOW_MS);
  }

  // Cancel is real immediately (the process must actually stop); Undo
  // restores via retry_item (stage -> queued) — same ponytail precedent as
  // Queue.svelte's row/bulk Cancel, since retry_item is the only backend
  // verb that reverses a cancel.
  async function requestCancel(id: number) {
    if (!confirm("Cancel this download?")) return;
    await queueStore.cancel(id);
    showUndoToast("Cancelled.", () => queueStore.retry(id));
  }

  // Remove defers the real bulk(remove) call for the toast window, same as
  // Queue.svelte — Undo just clears the timer, nothing ever reached the
  // backend. The row itself stays visible in the queue list during the
  // window (this component has no access to Queue.svelte's local
  // hiddenIds); the drawer closing is this action's immediate feedback.
  function requestRemove(id: number) {
    if (!confirm("Remove this download?")) return;
    close();
    removeTimer = setTimeout(() => {
      queueStore.bulk("remove", [id]);
    }, TOAST_WINDOW_MS);
    showUndoToast("Removed.", () => clearTimeout(removeTimer));
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && queueStore.activeDetailId != null) {
      e.preventDefault();
      e.stopImmediatePropagation();
      close();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });
</script>

{#if item}
  <aside class="drawer" aria-label="Download detail">
    <header>
      <div class="title-row">
        <h2 title={item.title ?? item.url}>{item.title ?? item.url}</h2>
        <button type="button" class="icon-btn" onclick={close} aria-label="Close">✕</button>
      </div>
      <div class="header-progress">
        <StageToken stage={item.stage} />
        <div class="bar-track" class:thick={ACTIVE_STAGES.has(item.stage)}>
          <div class="bar-fill" style:width="{Math.min(100, Math.max(0, item.percent))}%"></div>
        </div>
        <span class="figures mono">{item.percent.toFixed(0)}%</span>
      </div>
    </header>

    <div class="body">
      <FactsGrid
        {item}
        globalProxy={settingsStore.settings?.global_proxy ?? null}
        {presetName}
        onOpenFolder={(dir) => openPath(dir)}
      />

      <LogDisclosure itemId={item.id} stage={item.stage} />
    </div>

    <footer>
      {#if item.stage === "completed"}
        <button type="button" onclick={() => item.output_path && openPath(item.output_path)}>Open file</button>
        <button type="button" onclick={() => item.output_path && openPath(item.output_path, true)}>Open folder</button>
      {:else}
        {#if ACTIVE_STAGES.has(item.stage)}
          <button type="button" onclick={() => queueStore.pause(item.id)}>Pause</button>
        {:else if item.stage === "paused"}
          <button type="button" onclick={() => queueStore.resume(item.id)}>Resume</button>
        {/if}
        {#if item.stage === "error" || item.stage === "cancelled"}
          <button type="button" class="emphasized" onclick={() => queueStore.retry(item.id)}>Retry</button>
        {/if}
        {#if !TERMINAL_STAGES.has(item.stage)}
          <button type="button" onclick={() => requestCancel(item.id)}>Cancel</button>
        {/if}
      {/if}
      <button type="button" onclick={() => requestRemove(item.id)}>Remove</button>
    </footer>
  </aside>
{/if}

{#if toast}
  <div class="toast" role="status">
    <span>{toast.message}</span>
    <button type="button" class="undo" onclick={() => toast?.undo()}>Undo</button>
  </div>
{/if}

<style>
  .drawer {
    position: fixed;
    inset-block: 0;
    inset-inline-end: 0;
    width: 24rem;
    max-width: calc(100vw - 4rem);
    background: var(--surface-lowest);
    border-inline-start: 1px solid var(--border);
    box-shadow: -4px 0 12px color-mix(in srgb, var(--surface-lowest) 60%, transparent);
    display: flex;
    flex-direction: column;
    z-index: 40;
  }
  header {
    padding: 1rem 1.1rem 0.75rem;
    border-block-end: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  h2 {
    margin: 0;
    font-size: 1em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .icon-btn {
    flex-shrink: 0;
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    font-size: 1em;
    padding: 0.2rem 0.4rem;
  }
  .icon-btn:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .header-progress {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .bar-track {
    flex: 1;
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
  .mono {
    font-family: var(--font-mono);
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 1rem 1.1rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  footer {
    padding: 0.75rem 1.1rem;
    border-block-start: 1px solid var(--border);
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.5rem;
  }
  footer button {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4rem 0.7rem;
    font-family: var(--font-sans);
    font-size: 0.85em;
    cursor: pointer;
  }
  footer button.emphasized {
    background: var(--primary);
    color: var(--primary-foreground);
    border-color: var(--primary);
  }
  footer button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .toast {
    position: fixed;
    inset-block-end: 1.5rem;
    inset-inline-start: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 0.75rem;
    background: var(--surface-high);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.6rem 1rem;
    z-index: 45;
  }
  .undo {
    background: transparent;
    border: none;
    color: var(--primary);
    font-weight: 700;
    cursor: pointer;
    padding: 0;
  }
  .undo:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>
