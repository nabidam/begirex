// Queue store (ARCHITECTURE §2) — hydrates from list_items, then patches
// items in place from progress/stage_changed events. Frontend never
// computes durable truth: every field written here is a value the backend
// already emitted or returned.
import {
  addDownload,
  bulkAction,
  cancelItem,
  listItems,
  onItemAdded,
  onItemRemoved,
  onProgress,
  onStageChanged,
  pauseItem,
  removeItem,
  reorderItem,
  resumeItem,
  retryItem,
  setConcurrency,
} from "../ipc";
import type { AddDownloadRequest, AppError, Item } from "../types";

function createQueueStore() {
  let items = $state<Item[]>([]);
  let error = $state<string | null>(null);
  let concurrency = $state<number | null>(null);
  let subscribed = false;
  // ARCHITECTURE §2: activeDetailId lives in the queue store (S5's drawer
  // isn't built until T15 — T14's row click/Enter just sets this; nothing
  // reads it yet).
  let activeDetailId = $state<number | null>(null);

  function patch(id: number, fields: Partial<Item>) {
    // Spreading a key whose value is `undefined` still overwrites (object
    // spread keeps the key), so strip those before merging — only backend-
    // reported values should ever replace what's rendered.
    const defined = Object.fromEntries(
      Object.entries(fields).filter(([, value]) => value !== undefined),
    );
    items = items.map((item) => (item.id === id ? { ...item, ...defined } : item));
  }

  // Idempotent add — both the direct `add_download` response and the
  // `item_added` event can deliver the same row; whichever arrives first
  // wins the insert, the other just patches in place (no duplicate row).
  function upsert(item: Item) {
    const exists = items.some((i) => i.id === item.id);
    items = exists ? items.map((i) => (i.id === item.id ? item : i)) : [...items, item];
  }

  function removeLocally(id: number) {
    items = items.filter((i) => i.id !== id);
  }

  async function refresh() {
    error = null;
    try {
      items = await listItems();
    } catch (err) {
      error = (err as AppError).message;
    }
  }

  async function init() {
    await refresh();

    if (subscribed) return;
    subscribed = true;
    await onProgress((payload) => {
      patch(payload.id, {
        percent: payload.percent,
        downloaded_bytes: payload.downloaded_bytes ?? undefined,
        total_bytes: payload.total_bytes,
        speed_bps: payload.speed_bps,
        eta_seconds: payload.eta_seconds,
        stage: payload.stage,
      });
    });
    await onStageChanged((payload) => {
      patch(payload.id, { stage: payload.stage, error_message: payload.error_message });
    });
    await onItemAdded((item) => upsert(item));
    await onItemRemoved((payload) => removeLocally(payload.id));
  }

  async function add(request: AddDownloadRequest) {
    error = null;
    try {
      const { items: added } = await addDownload(request);
      added.forEach(upsert);
    } catch (err) {
      error = (err as AppError).message;
      throw err;
    }
  }

  async function runAction<T>(action: () => Promise<T>): Promise<T | undefined> {
    error = null;
    try {
      return await action();
    } catch (err) {
      error = (err as AppError).message;
      return undefined;
    }
  }

  async function pause(id: number) {
    const item = await runAction(() => pauseItem(id));
    if (item) upsert(item);
  }

  async function resume(id: number) {
    const item = await runAction(() => resumeItem(id));
    if (item) upsert(item);
  }

  async function cancel(id: number) {
    const item = await runAction(() => cancelItem(id));
    if (item) upsert(item);
  }

  async function remove(id: number) {
    const ok = await runAction(() => removeItem(id));
    if (ok) removeLocally(id);
  }

  async function retry(id: number) {
    const item = await runAction(() => retryItem(id));
    if (item) upsert(item);
  }

  // Buttons move an item by one slot among *all* items (matching the
  // backend's absolute-index reorder semantics) — full drag reordering is
  // T14.
  async function moveUp(id: number) {
    const index = items.findIndex((i) => i.id === id);
    if (index <= 0) return;
    const ok = await runAction(() => reorderItem(id, index - 1));
    if (ok) await refresh();
  }

  async function moveDown(id: number) {
    const index = items.findIndex((i) => i.id === id);
    if (index === -1 || index >= items.length - 1) return;
    const ok = await runAction(() => reorderItem(id, index + 1));
    if (ok) await refresh();
  }

  // Drag-reorder (T14, V4-AC3) — the drop target is resolved against the
  // full (unfiltered) item order, matching moveUp/moveDown's own semantics.
  async function reorderTo(id: number, newPosition: number) {
    const ok = await runAction(() => reorderItem(id, newPosition));
    if (ok) await refresh();
  }

  // Toolbar's Start all / Pause all (T13) — operates on whatever id set the
  // caller decides is "visible" (post filter/search); this store just runs
  // the bulk verb and reconciles the returned rows.
  async function bulk(action: "pause" | "resume" | "cancel" | "remove", ids: number[]) {
    if (ids.length === 0) return;
    const result = await runAction(() => bulkAction({ ids, action }));
    if (result) result.updated.forEach(upsert);
  }

  async function pauseAll(ids: number[]) {
    await bulk("pause", ids);
  }

  async function resumeAll(ids: number[]) {
    await bulk("resume", ids);
  }

  async function setConcurrencyLevel(n: number) {
    const result = await runAction(() => setConcurrency(n));
    if (result) concurrency = result.n;
  }

  function openDetail(id: number) {
    activeDetailId = id;
  }

  function closeDetail() {
    activeDetailId = null;
  }

  return {
    get items() {
      return items;
    },
    get error() {
      return error;
    },
    get concurrency() {
      return concurrency;
    },
    get activeDetailId() {
      return activeDetailId;
    },
    init,
    add,
    pause,
    resume,
    cancel,
    remove,
    retry,
    moveUp,
    moveDown,
    reorderTo,
    pauseAll,
    resumeAll,
    bulk,
    setConcurrency: setConcurrencyLevel,
    openDetail,
    closeDetail,
  };
}

export const queueStore = createQueueStore();
