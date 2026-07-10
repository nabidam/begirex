<script lang="ts">
  // S2 list (UX.md S2, TASKS.md T14/T24) — the queue's rows region: virtualized
  // list (T10's VirtualList), selection + bulk actions (SelectionBar),
  // per-row drag-reorder with a movement threshold, roving-tabindex keyboard
  // nav, and the confirm+undo-toast flow for Cancel/Remove (ARCHITECTURE §8
  // "soft-delete... hard-delete on toast expiry"). Shell.svelte (T13) owns
  // the sidebar/toolbar/Add/Presets overlays; this component only renders
  // whatever `items` (post filter/search) it's handed.
  import { queueStore } from "../stores/queue.svelte";
  import QueueRow from "../components/QueueRow.svelte";
  import SelectionBar from "../components/SelectionBar.svelte";
  import VirtualList from "../components/VirtualList.svelte";
  import type { Item } from "../types";
  import { Button } from "$lib/components/ui/button";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import { toast } from "svelte-sonner";

  let {
    items,
    totalCount,
    onAdd,
    onShowAll,
    addDisabled = false,
  }: {
    items: Item[];
    totalCount: number;
    onAdd: () => void;
    onShowAll: () => void;
    addDisabled?: boolean;
  } = $props();

  const ROW_HEIGHT = 56;
  const TOAST_WINDOW_MS = 5000;

  let listHeight = $state(0);
  let selectedIds = $state<Set<number>>(new Set());
  let focusedId = $state<number | null>(null);
  // Optimistically-hidden ids pending a deferred hard-delete (Remove's
  // client-side soft-delete window — ARCHITECTURE §8).
  let hiddenIds = $state<Set<number>>(new Set());
  let removeTimers = new Map<number, ReturnType<typeof setTimeout>>();

  // Cancel/Remove confirm (shadcn alert-dialog, replacing the hand-rolled
  // confirm()) — set by requestCancel/requestRemove, cleared once the user
  // picks an option; the dialog itself never calls the store directly.
  let confirmState = $state<{ kind: "cancel" | "remove"; ids: number[] } | null>(null);

  // Drag-reorder (DESIGN.md §4 gap #4): native pointer events, ~6px
  // movement threshold — below it, pointerup is treated as a row click.
  let dragCandidateId = $state<number | null>(null);
  let dragStartY = 0;
  let dragActive = $state(false);
  let dropId = $state<number | null>(null);

  const visible = $derived(items.filter((i) => !hiddenIds.has(i.id)));

  $effect(() => {
    if (visible.length === 0) {
      focusedId = null;
    } else if (focusedId === null || !visible.some((i) => i.id === focusedId)) {
      focusedId = visible[0].id;
    }
  });

  function toggleSelect(id: number) {
    const next = new Set(selectedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selectedIds = next;
  }

  function clearSelection() {
    selectedIds = new Set();
  }

  function focusRow(id: number) {
    focusedId = id;
  }

  function onArrow(direction: "up" | "down") {
    const index = visible.findIndex((i) => i.id === focusedId);
    if (index === -1) return;
    const nextIndex = direction === "down" ? index + 1 : index - 1;
    const next = visible[nextIndex];
    if (next) focusedId = next.id;
  }

  $effect(() => {
    if (focusedId === null) return;
    const el = document.querySelector<HTMLElement>(`[data-row-id="${focusedId}"]`);
    el?.focus();
  });

  function openDetail(id: number) {
    queueStore.openDetail(id);
  }

  function onRowPointerDown(id: number, e: PointerEvent) {
    if (e.button !== 0) return;
    dragCandidateId = id;
    dragStartY = e.clientY;
    dragActive = false;
  }

  $effect(() => {
    if (dragCandidateId === null) return;
    const candidateId = dragCandidateId;

    function onMove(e: PointerEvent) {
      if (!dragActive && Math.abs(e.clientY - dragStartY) > 6) {
        dragActive = true;
      }
      if (dragActive) {
        const el = document.elementFromPoint(e.clientX, e.clientY);
        const rowEl = el instanceof Element ? el.closest("[data-row-id]") : null;
        dropId = rowEl ? Number(rowEl.getAttribute("data-row-id")) : null;
      }
    }

    function onUp() {
      if (dragActive && dropId !== null && dropId !== candidateId) {
        const targetItem = queueStore.items.find((i) => i.id === dropId);
        const targetIndex = targetItem ? queueStore.items.indexOf(targetItem) : -1;
        if (targetIndex !== -1) queueStore.reorderTo(candidateId, targetIndex);
      } else if (!dragActive) {
        openDetail(candidateId);
      }
      dragCandidateId = null;
      dragActive = false;
      dropId = null;
    }

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp, { once: true });
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  });

  function pluralize(n: number, noun: string): string {
    return `${n} ${noun}${n === 1 ? "" : "s"}`;
  }

  function requestCancel(ids: number[]) {
    if (ids.length === 0) return;
    confirmState = { kind: "cancel", ids };
  }

  function requestRemove(ids: number[]) {
    if (ids.length === 0) return;
    confirmState = { kind: "remove", ids };
  }

  // Runs only once the alert-dialog's destructive action is confirmed.
  // Cancel is real immediately (the process must actually stop) with Undo
  // calling `retry_item` to restore to `queued` — not necessarily the
  // item's exact prior stage; ponytail: `retry_item` is the only backend
  // verb that reverses a cancel. Remove hides ids client-side only and
  // defers the real `bulk("remove")` call for TOAST_WINDOW_MS — Undo within
  // the window just clears the timer and un-hides; nothing ever reaches the
  // backend unless the window lapses.
  async function confirmProceed() {
    if (!confirmState) return;
    const { kind, ids } = confirmState;
    confirmState = null;

    if (kind === "cancel") {
      await queueStore.bulk("cancel", ids);
      ids.forEach((id) => selectedIds.delete(id));
      selectedIds = new Set(selectedIds);
      toast(`Cancelled ${pluralize(ids.length, "item")}.`, {
        duration: TOAST_WINDOW_MS,
        action: {
          label: "Undo",
          onClick: () => ids.forEach((id) => queueStore.retry(id)),
        },
      });
      return;
    }

    const next = new Set(hiddenIds);
    ids.forEach((id) => next.add(id));
    hiddenIds = next;
    ids.forEach((id) => selectedIds.delete(id));
    selectedIds = new Set(selectedIds);

    const timer = setTimeout(() => {
      queueStore.bulk("remove", ids);
      ids.forEach((id) => removeTimers.delete(id));
    }, TOAST_WINDOW_MS);
    ids.forEach((id) => removeTimers.set(id, timer));

    toast(`Removed ${pluralize(ids.length, "item")}.`, {
      duration: TOAST_WINDOW_MS,
      action: {
        label: "Undo",
        onClick: () => {
          clearTimeout(timer);
          ids.forEach((id) => removeTimers.delete(id));
          const restored = new Set(hiddenIds);
          ids.forEach((id) => restored.delete(id));
          hiddenIds = restored;
        },
      },
    });
  }

  function moveSelected(direction: "up" | "down") {
    const ids = [...selectedIds];
    const ordered = direction === "up"
      ? ids.sort((a, b) => queueStore.items.findIndex((i) => i.id === a) - queueStore.items.findIndex((i) => i.id === b))
      : ids.sort((a, b) => queueStore.items.findIndex((i) => i.id === b) - queueStore.items.findIndex((i) => i.id === a));
    ordered.forEach((id) => (direction === "up" ? queueStore.moveUp(id) : queueStore.moveDown(id)));
  }
</script>

<main class="flex min-h-0 flex-1 flex-col">
  {#if queueStore.error}
    <p class="px-4 py-2 text-[var(--error-token)]">{queueStore.error}</p>
  {/if}

  <SelectionBar
    count={selectedIds.size}
    onStart={() => queueStore.bulk("resume", [...selectedIds])}
    onPause={() => queueStore.bulk("pause", [...selectedIds])}
    onCancel={() => requestCancel([...selectedIds])}
    onRemove={() => requestRemove([...selectedIds])}
    onMoveUp={() => moveSelected("up")}
    onMoveDown={() => moveSelected("down")}
    onClear={clearSelection}
  />

  {#if visible.length > 0}
    <div
      class="grid grid-cols-[2rem_minmax(6rem,1fr)_4.5rem_minmax(12rem,2fr)_3.5rem_2.5rem] gap-3 px-[1.6rem] text-[0.75em] tracking-wide text-muted-foreground uppercase"
      role="row"
    >
      <span></span>
      <span>Title</span>
      <span class="text-end">Size</span>
      <span>Status</span>
      <span class="text-end">ETA</span>
      <span></span>
    </div>
  {/if}

  <div class="min-h-0 flex-1 px-4 py-2" bind:clientHeight={listHeight}>
    {#if visible.length === 0}
      {#if addDisabled}
        <p class="px-4 py-8 text-center text-muted-foreground">
          Downloads are disabled until yt-dlp/ffmpeg are set up — see Settings (sidebar) to finish setup.
        </p>
      {:else if totalCount === 0}
        <p class="px-4 py-8 text-center text-muted-foreground">
          No downloads yet. Paste a link or press
          <Button type="button" variant="link" class="h-auto p-0 align-baseline" onclick={onAdd}>Add</Button>
          to start.
        </p>
      {:else}
        <p class="px-4 py-8 text-center text-muted-foreground">
          Nothing here.
          <Button type="button" variant="link" class="h-auto p-0 align-baseline" onclick={onShowAll}>Show all</Button>
        </p>
      {/if}
    {:else}
      <VirtualList items={visible} itemHeight={ROW_HEIGHT} height={listHeight}>
        {#snippet row(item: Item)}
          <QueueRow
            {item}
            selected={selectedIds.has(item.id)}
            focused={focusedId === item.id}
            onToggleSelect={toggleSelect}
            onPointerDown={onRowPointerDown}
            {onArrow}
            onOpenDetail={openDetail}
            onFocusRow={focusRow}
            onCancelRequest={requestCancel}
            onRemoveRequest={requestRemove}
          />
        {/snippet}
      </VirtualList>
    {/if}
  </div>
</main>

<AlertDialog.Root
  open={confirmState !== null}
  onOpenChange={(open) => {
    if (!open) confirmState = null;
  }}
>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>
        {confirmState?.kind === "cancel"
          ? `Cancel ${pluralize(confirmState.ids.length, "download")}?`
          : `Remove ${pluralize(confirmState?.ids.length ?? 0, "download")}?`}
      </AlertDialog.Title>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Keep</AlertDialog.Cancel>
      <AlertDialog.Action variant="destructive" onclick={confirmProceed}>
        {confirmState?.kind === "cancel" ? "Cancel downloads" : "Remove"}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
