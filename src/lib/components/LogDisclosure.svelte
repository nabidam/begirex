<script lang="ts">
  // LogDisclosure (UX.md S5 region 3, DESIGN.md §3 gap "collapsible") — the
  // buried yt-dlp stdout/stderr tail. Collapsed by default to keep the
  // drawer scannable; auto-expands for an errored item so the failing tail
  // is the first thing a debugging user sees (K3-AC6/K5-AC4).
  // ponytail: hand-rolled disclosure, same no-shadcn-pipeline precedent as
  // every prior task (AddDownload.svelte's "Advanced" toggle) — no
  // `collapsible` dep added.
  import { onMount } from "svelte";
  import { getItemLog, onLogLine, watchLog } from "../ipc";
  import type { LogLine } from "../types";

  let { itemId, stage }: { itemId: number; stage: string } = $props();

  let open = $state(false);
  let lines = $state<LogLine[]>([]);
  let logEl: HTMLDivElement | undefined = $state();
  let lastItemId: number | undefined;

  // Re-open (and re-fetch) whenever the drawer switches to a different item,
  // or the item transitions into `error` while already viewed.
  $effect(() => {
    if (itemId !== lastItemId) {
      lastItemId = itemId;
      open = stage === "error";
    } else if (stage === "error") {
      open = true;
    }
  });

  $effect(() => {
    if (!open) return;
    const id = itemId;
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    (async () => {
      lines = await getItemLog(id, 200);
      await watchLog(id, true);
      if (cancelled) return;
      unlisten = await onLogLine((payload) => {
        if (payload.id !== id) return;
        lines = [...lines, { ts: Date.now(), stream: payload.stream, line: payload.line }];
      });
    })();

    return () => {
      cancelled = true;
      unlisten?.();
      watchLog(id, false);
    };
  });

  $effect(() => {
    lines;
    if (logEl) logEl.scrollTop = logEl.scrollHeight;
  });

  onMount(() => {
    return () => {
      // Belt-and-suspenders: the `$effect` cleanup above already turns
      // watching off when `open` flips false, but a hard unmount while
      // still open (drawer closed) needs the same cleanup.
      if (open) watchLog(itemId, false);
    };
  });
</script>

<div class="disclosure">
  <button
    type="button"
    class="toggle"
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    {open ? "▾" : "▸"} Log (yt-dlp stdout/stderr, tailing)
  </button>
  {#if open}
    <div class="log mono" bind:this={logEl} role="log">
      {#if lines.length === 0}
        <p class="empty">No output yet.</p>
      {:else}
        {#each lines as l, i (i)}
          <p class="line" class:stderr={l.stream === "stderr"}>{l.line}</p>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .disclosure {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .toggle {
    align-self: flex-start;
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    padding: 0;
    font-size: 0.85em;
    cursor: pointer;
  }
  .toggle:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .log {
    max-height: 12rem;
    overflow-y: auto;
    background: var(--muted);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.5rem 0.6rem;
    font-size: 0.78em;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .line {
    margin: 0;
    white-space: pre-wrap;
    color: var(--foreground);
  }
  .line.stderr {
    color: var(--error-token);
  }
  .empty {
    margin: 0;
    color: var(--muted-foreground);
  }
</style>
