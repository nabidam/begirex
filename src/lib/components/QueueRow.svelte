<script lang="ts">
  // QueueRow (UX.md S2, TASKS.md T14/T24) — one dense queue row: selection
  // checkbox, title, size, the inline-progress signature (StageToken + pill
  // bar + figures), ETA, and a row-overflow menu for contextual actions.
  // Drag-reorder and keyboard focus are orchestrated by Queue.svelte (needs
  // the full visible-row list); this component only reports pointer/keydown
  // events upward and renders whatever selection/focus state it's handed.
  import { queueStore } from "../stores/queue.svelte";
  import StageToken from "./StageToken.svelte";
  import type { Item } from "../types";
  import { cn } from "$lib/utils";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { Button } from "$lib/components/ui/button";
  import { Progress } from "$lib/components/ui/progress";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
  import Ellipsis from "lucide-svelte/icons/ellipsis";
  import Pause from "lucide-svelte/icons/pause";
  import Play from "lucide-svelte/icons/play";
  import RotateCcw from "lucide-svelte/icons/rotate-ccw";
  import ArrowUp from "lucide-svelte/icons/arrow-up";
  import ArrowDown from "lucide-svelte/icons/arrow-down";
  import Ban from "lucide-svelte/icons/ban";
  import Trash2 from "lucide-svelte/icons/trash-2";

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
</script>

<div
  class={cn(
    "grid h-full grid-cols-[2rem_1fr_auto] items-center gap-2 rounded-lg border border-transparent px-[0.6rem] hover:bg-accent focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring",
    selected && "bg-[var(--surface-high)]",
    focused && "ring-2 ring-inset ring-ring",
  )}
  data-row-id={item.id}
  role="row"
  aria-selected={selected}
  tabindex={focused ? 0 : -1}
  onkeydown={handleKeydown}
  onfocus={() => onFocusRow(item.id)}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span class="flex items-center" onpointerdown={(e) => e.stopPropagation()}>
    <Checkbox
      checked={selected}
      onCheckedChange={() => onToggleSelect(item.id)}
      aria-label="Select {item.title ?? item.url}"
    />
  </span>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="grid min-w-0 cursor-pointer grid-cols-[minmax(6rem,1fr)_4.5rem_minmax(12rem,2fr)_3.5rem] items-center gap-3"
    onpointerdown={(e) => onPointerDown(item.id, e)}
  >
    <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[0.9em]" title={item.title ?? item.url}>
      {item.title ?? item.url}
    </span>
    <span class="text-end font-mono text-xs text-muted-foreground">{formatBytes(item.total_bytes)}</span>

    <div class="flex min-w-0 items-center gap-2">
      <StageToken stage={item.stage} />
      <Progress
        value={Math.min(100, Math.max(0, item.percent))}
        class={cn("min-w-12 flex-1", ACTIVE_STAGES.has(item.stage) ? "h-2" : "h-1")}
      />
      <span class="shrink-0 font-mono text-xs text-muted-foreground">{item.percent.toFixed(0)}%</span>
      {#if item.speed_bps != null}
        <span class="shrink-0 font-mono text-xs text-muted-foreground">{formatSpeed(item.speed_bps)}</span>
      {/if}
      {#if item.stage === "error" && item.error_message}
        <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-xs text-[var(--error-token)]">
          {item.error_message}
        </span>
        <Button
          type="button"
          variant="outline"
          size="xs"
          class="shrink-0"
          onclick={() => queueStore.retry(item.id)}
        >
          <RotateCcw aria-hidden="true" />
          Retry
        </Button>
      {/if}
    </div>

    <span class="text-end font-mono text-xs text-muted-foreground">{formatEta(item.eta_seconds)}</span>
  </div>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span class="relative" onpointerdown={(e) => e.stopPropagation()}>
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Button {...props} type="button" variant="ghost" size="icon-sm" aria-label="Row actions">
            <Ellipsis aria-hidden="true" />
          </Button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end">
        {#if ACTIVE_STAGES.has(item.stage)}
          <DropdownMenu.Item onclick={() => queueStore.pause(item.id)}>
            <Pause aria-hidden="true" />
            Pause
          </DropdownMenu.Item>
        {:else if item.stage === "paused"}
          <DropdownMenu.Item onclick={() => queueStore.resume(item.id)}>
            <Play aria-hidden="true" />
            Resume
          </DropdownMenu.Item>
        {:else if item.stage === "error"}
          <DropdownMenu.Item onclick={() => queueStore.retry(item.id)}>
            <RotateCcw aria-hidden="true" />
            Retry
          </DropdownMenu.Item>
        {/if}
        {#if item.stage === "queued"}
          <DropdownMenu.Item onclick={() => queueStore.moveUp(item.id)}>
            <ArrowUp aria-hidden="true" />
            Move up
          </DropdownMenu.Item>
          <DropdownMenu.Item onclick={() => queueStore.moveDown(item.id)}>
            <ArrowDown aria-hidden="true" />
            Move down
          </DropdownMenu.Item>
        {/if}
        {#if !TERMINAL_STAGES.has(item.stage)}
          <DropdownMenu.Item onclick={() => onCancelRequest([item.id])}>
            <Ban aria-hidden="true" />
            Cancel
          </DropdownMenu.Item>
        {/if}
        <DropdownMenu.Item variant="destructive" onclick={() => onRemoveRequest([item.id])}>
          <Trash2 aria-hidden="true" />
          Remove
        </DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </span>
</div>
