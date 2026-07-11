<script lang="ts">
  // LogDisclosure (UX.md S5 region 3, DESIGN.md §3 gap "collapsible",
  // migrated to shadcn `Collapsible` at T26) — the buried yt-dlp
  // stdout/stderr tail. Collapsed by default to keep the drawer scannable;
  // auto-expands for an errored item so the failing tail is the first
  // thing a debugging user sees (K3-AC6/K5-AC4).
  import { onMount } from "svelte";
  import { getItemLog, onLogLine, watchLog } from "../ipc";
  import type { LogLine } from "../types";
  import * as Collapsible from "$lib/components/ui/collapsible";
  import { Button } from "$lib/components/ui/button";
  import { cn } from "$lib/utils";
  import ChevronRight from "lucide-svelte/icons/chevron-right";

  let { itemId, stage }: { itemId: number; stage: string } = $props();

  let open = $state(false);
  let lines = $state<LogLine[]>([]);
  let logEl: HTMLDivElement | undefined = $state();
  let lastItemId: number | undefined;
  let isFollowing = $state(true);
  let unseenLines = $state(0);
  let previousLineCount = 0;
  let loading = $state(false);
  let loadError = $state<string | null>(null);
  let reloadVersion = $state(0);
  const MAX_LOG_LINES = 1000;

  // Re-open (and re-fetch) whenever the drawer switches to a different item,
  // or the item transitions into `error` while already viewed.
  $effect(() => {
    if (itemId !== lastItemId) {
      lastItemId = itemId;
      open = stage === "error";
      lines = [];
      isFollowing = true;
      unseenLines = 0;
      previousLineCount = 0;
    } else if (stage === "error") {
      open = true;
    }
  });

  $effect(() => {
    reloadVersion;
    if (!open) return;
    const id = itemId;
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    (async () => {
      loading = true;
      loadError = null;
      try {
        const initialLines = await getItemLog(id, 200);
        if (cancelled) return;
        lines = initialLines.slice(-MAX_LOG_LINES);

        await watchLog(id, true);
        if (cancelled) {
          await watchLog(id, false);
          return;
        }

        unlisten = await onLogLine((payload) => {
          if (payload.id !== id) return;
          lines = [...lines, { ts: Date.now(), stream: payload.stream, line: payload.line }].slice(-MAX_LOG_LINES);
        });
      } catch (err) {
        if (!cancelled) loadError = err instanceof Error ? err.message : "Unable to load the log.";
      } finally {
        if (!cancelled) loading = false;
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
      void watchLog(id, false).catch(() => undefined);
    };
  });

  $effect(() => {
    const lineCount = lines.length;
    const newLineCount = Math.max(0, lineCount - previousLineCount);
    if (logEl && isFollowing) {
      logEl.scrollTop = logEl.scrollHeight;
      unseenLines = 0;
    } else if (newLineCount > 0) {
      unseenLines += newLineCount;
    }
    previousLineCount = lineCount;
  });

  function handleLogScroll() {
    if (!logEl) return;
    isFollowing = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight <= 8;
    if (isFollowing) unseenLines = 0;
  }

  function jumpToLatest() {
    if (logEl) logEl.scrollTop = logEl.scrollHeight;
    isFollowing = true;
    unseenLines = 0;
  }

  function retryLog() {
    lines = [];
    unseenLines = 0;
    isFollowing = true;
    previousLineCount = 0;
    reloadVersion += 1;
  }

  onMount(() => {
    return () => {
      // Belt-and-suspenders: the `$effect` cleanup above already turns
      // watching off when `open` flips false, but a hard unmount while
      // still open (drawer closed) needs the same cleanup.
      if (open) void watchLog(itemId, false).catch(() => undefined);
    };
  });
</script>

<Collapsible.Root bind:open class="flex flex-col gap-1">
  <Collapsible.Trigger>
    {#snippet child({ props })}
      <Button
        {...props}
        type="button"
        variant="ghost"
        size="sm"
        class="w-fit gap-1 self-start px-0 text-muted-foreground hover:bg-transparent"
        aria-expanded={open}
      >
        <ChevronRight aria-hidden="true" class={cn("size-3.5 transition-transform", open && "rotate-90")} />
        Log (yt-dlp stdout/stderr, tailing)
      </Button>
    {/snippet}
  </Collapsible.Trigger>
  <Collapsible.Content>
    {#if loadError}
      <div class="mb-2 flex flex-wrap items-center gap-2 rounded-lg border border-border bg-muted p-2 text-xs" role="alert">
        <span class="min-w-0 flex-1">Couldn’t load the log. {loadError}</span>
        <Button type="button" variant="outline" size="xs" onclick={retryLog}>Retry</Button>
      </div>
    {/if}
    {#if !isFollowing && unseenLines > 0}
      <Button type="button" variant="outline" size="xs" class="mb-2" onclick={jumpToLatest}>
        {unseenLines} new {unseenLines === 1 ? "line" : "lines"}
      </Button>
    {/if}
    <div
      class="max-h-48 overflow-y-auto rounded-lg border border-border bg-muted p-2.5 font-mono text-xs"
      bind:this={logEl}
      role="log"
      aria-live={isFollowing ? "polite" : "off"}
      onscroll={handleLogScroll}
    >
      {#if loading && lines.length === 0}
        <p class="m-0 text-muted-foreground">Loading log…</p>
      {:else if lines.length === 0}
        <p class="m-0 text-muted-foreground">No output yet.</p>
      {:else}
        {#each lines as l, i (i)}
          <p class={cn("m-0 whitespace-pre-wrap text-foreground", l.stream === "stderr" && "text-[var(--error-token)]")}>
            {l.line}
          </p>
        {/each}
      {/if}
    </div>
  </Collapsible.Content>
</Collapsible.Root>
