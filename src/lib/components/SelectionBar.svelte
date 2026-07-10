<script lang="ts">
  // SelectionBar (UX.md S2) — appears only when >=1 row is selected; hosts
  // bulk start/pause/cancel/remove/reorder. Cancel/Remove route through
  // Queue.svelte's confirm+undo-toast flow (ARCHITECTURE §8), same as each
  // row's own overflow menu — this component never calls the ipc/store
  // layer directly for those two verbs.
  import { Button } from "$lib/components/ui/button";
  import Play from "lucide-svelte/icons/play";
  import Pause from "lucide-svelte/icons/pause";
  import Ban from "lucide-svelte/icons/ban";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import ArrowUp from "lucide-svelte/icons/arrow-up";
  import ArrowDown from "lucide-svelte/icons/arrow-down";

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
  <div class="flex items-center gap-2 border-y border-border bg-[var(--surface-high)] px-4 py-2" role="toolbar" aria-label="Bulk actions">
    <span class="me-2 font-mono text-[0.85em] text-muted-foreground">{count} selected</span>
    <Button type="button" variant="outline" size="sm" onclick={onStart}>
      <Play aria-hidden="true" />
      Start
    </Button>
    <Button type="button" variant="outline" size="sm" onclick={onPause}>
      <Pause aria-hidden="true" />
      Pause
    </Button>
    <Button type="button" variant="outline" size="sm" onclick={onCancel}>
      <Ban aria-hidden="true" />
      Cancel
    </Button>
    <Button type="button" variant="outline" size="sm" onclick={onRemove}>
      <Trash2 aria-hidden="true" />
      Remove
    </Button>
    <span class="flex gap-0.5">
      <Button type="button" variant="ghost" size="icon-sm" aria-label="Move up" onclick={onMoveUp}>
        <ArrowUp aria-hidden="true" />
      </Button>
      <Button type="button" variant="ghost" size="icon-sm" aria-label="Move down" onclick={onMoveDown}>
        <ArrowDown aria-hidden="true" />
      </Button>
    </span>
    <Button type="button" variant="ghost" size="sm" class="ms-auto text-muted-foreground" onclick={onClear}>
      Clear selection
    </Button>
  </div>
{/if}
