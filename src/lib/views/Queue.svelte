<script lang="ts">
  // S2 list (UX.md S2, TASKS.md T14) — the queue's rows region: virtualized
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

  let {
    items,
    totalCount,
    onAdd,
    onShowAll,
  }: {
    items: Item[];
    totalCount: number;
    onAdd: () => void;
    onShowAll: () => void;
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

  let toast = $state<{ message: string; undo: () => void } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

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

  function pluralize(n: number, noun: string): string {
    return `${n} ${noun}${n === 1 ? "" : "s"}`;
  }

  // ponytail: native confirm() stands in for a proper alert-dialog
  // component — same precedent as Presets.svelte's delete confirm.
  // Upgrade path: a shared ConfirmDialog if a second real need appears.
  async function requestCancel(ids: number[]) {
    if (ids.length === 0) return;
    if (!confirm(`Cancel ${pluralize(ids.length, "download")}?`)) return;
    await queueStore.bulk("cancel", ids);
    ids.forEach((id) => selectedIds.delete(id));
    selectedIds = new Set(selectedIds);
    // ponytail: undo restores via retry_item (stage -> queued), the only
    // backend verb that reverses a cancel — not necessarily the item's
    // exact prior stage (e.g. a paused item restores to queued, not
    // paused). Upgrade path: a dedicated uncancel command if that gap
    // matters in practice.
    showUndoToast(`Cancelled ${pluralize(ids.length, "item")}.`, () => {
      ids.forEach((id) => queueStore.retry(id));
    });
  }

  function requestRemove(ids: number[]) {
    if (ids.length === 0) return;
    if (!confirm(`Remove ${pluralize(ids.length, "download")}?`)) return;
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

    showUndoToast(`Removed ${pluralize(ids.length, "item")}.`, () => {
      clearTimeout(timer);
      ids.forEach((id) => removeTimers.delete(id));
      const restored = new Set(hiddenIds);
      ids.forEach((id) => restored.delete(id));
      hiddenIds = restored;
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

<main class="queue">
  {#if queueStore.error}
    <p class="error">{queueStore.error}</p>
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
    <div class="columns" role="row">
      <span class="col-checkbox"></span>
      <span class="col-title">Title</span>
      <span class="col-size">Size</span>
      <span class="col-progress">Status</span>
      <span class="col-eta">ETA</span>
      <span class="col-actions"></span>
    </div>
  {/if}

  <div class="list-wrap" bind:clientHeight={listHeight}>
    {#if visible.length === 0}
      {#if totalCount === 0}
        <p class="empty">
          No downloads yet. Paste a link or press
          <button type="button" class="link-btn" onclick={onAdd}>Add</button> to start.
        </p>
      {:else}
        <p class="empty">
          Nothing here. <button type="button" class="link-btn" onclick={onShowAll}>Show all</button>
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

  {#if toast}
    <div class="toast" role="status">
      <span>{toast.message}</span>
      <button type="button" class="undo" onclick={() => toast?.undo()}>Undo</button>
    </div>
  {/if}
</main>

<style>
  .queue {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .list-wrap {
    flex: 1;
    min-height: 0;
    padding: 0.5rem 1rem;
  }
  .columns {
    display: grid;
    grid-template-columns: 2rem minmax(6rem, 1fr) 4.5rem minmax(12rem, 2fr) 3.5rem 2.5rem;
    gap: 0.75rem;
    padding: 0 1.6rem;
    color: var(--muted-foreground);
    font-size: 0.75em;
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }
  .col-size,
  .col-eta {
    text-align: end;
  }
  .link-btn {
    background: none;
    border: none;
    color: var(--primary);
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
    font: inherit;
  }
  .link-btn:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .empty {
    color: var(--muted-foreground);
    text-align: center;
    padding: 2rem 1rem;
  }
  .error {
    color: var(--error-token);
    padding: 0.5rem 1rem;
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
    z-index: 40;
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
