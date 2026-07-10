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
    scope,
    canResume,
    canPause,
    canCancel,
    canRemove,
    canReorder,
    onResume,
    onPause,
    onCancel,
    onRemove,
    onMoveUp,
    onMoveDown,
    onClear,
  }: {
    count: number;
    scope: string;
    canResume: boolean;
    canPause: boolean;
    canCancel: boolean;
    canRemove: boolean;
    canReorder: boolean;
    onResume: () => void;
    onPause: () => void;
    onCancel: () => void;
    onRemove: () => void;
    onMoveUp: () => void;
    onMoveDown: () => void;
    onClear: () => void;
  } = $props();
</script>

{#if count > 0}
  <div class="border-y border-border bg-[var(--surface-high)] px-4 py-2" role="toolbar" aria-label="Bulk actions">
    <div class="flex items-center gap-2">
      <span class="me-2 font-mono text-xs text-muted-foreground">{count} selected · {scope}</span>
      <span title={canResume ? undefined : "Resume is available only when every selected item is paused."}>
        <Button type="button" variant="outline" size="sm" disabled={!canResume} onclick={onResume}>
          <Play aria-hidden="true" />
          Resume
        </Button>
      </span>
      <span title={canPause ? undefined : "Pause is available only when every selected item is queued or active."}>
        <Button type="button" variant="outline" size="sm" disabled={!canPause} onclick={onPause}>
          <Pause aria-hidden="true" />
          Pause
        </Button>
      </span>
      <span title={canCancel ? undefined : "Cancel is unavailable when the selection includes completed or cancelled items."}>
        <Button type="button" variant="outline" size="sm" disabled={!canCancel} onclick={onCancel}>
          <Ban aria-hidden="true" />
          Cancel
        </Button>
      </span>
      <span title={canRemove ? undefined : "Remove is unavailable until every selected item is available."}>
        <Button type="button" variant="outline" size="sm" disabled={!canRemove} onclick={onRemove}>
          <Trash2 aria-hidden="true" />
          Remove
        </Button>
      </span>
      <span class="flex gap-0.5" title={canReorder ? undefined : "Reorder is available only when every selected item is queued."}>
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Move up" disabled={!canReorder} onclick={onMoveUp}>
          <ArrowUp aria-hidden="true" />
        </Button>
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Move down" disabled={!canReorder} onclick={onMoveDown}>
          <ArrowDown aria-hidden="true" />
        </Button>
      </span>
      <Button type="button" variant="ghost" size="sm" class="ms-auto text-muted-foreground" onclick={onClear}>
        Clear selection
      </Button>
    </div>
    <p class="mt-1 text-xs text-muted-foreground">
      An action is enabled only when it applies to every selected download.
    </p>
  </div>
{/if}
