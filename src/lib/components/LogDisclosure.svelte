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

<Collapsible.Root bind:open class="flex flex-col gap-[0.4rem]">
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
    <div
      class="max-h-48 overflow-y-auto rounded-lg border border-border bg-muted p-2.5 font-mono text-[0.78em]"
      bind:this={logEl}
      role="log"
    >
      {#if lines.length === 0}
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
