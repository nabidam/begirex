// Queue store (ARCHITECTURE §2) — hydrates from list_items, then patches
// items in place from progress/stage_changed events. Frontend never
// computes durable truth: every field written here is a value the backend
// already emitted or returned.
import { addDownload, listItems, onProgress, onStageChanged } from "../ipc";
import type { AddDownloadRequest, AppError, Item } from "../types";

function createQueueStore() {
  let items = $state<Item[]>([]);
  let error = $state<string | null>(null);
  let subscribed = false;

  function patch(id: number, fields: Partial<Item>) {
    // Spreading a key whose value is `undefined` still overwrites (object
    // spread keeps the key), so strip those before merging — only backend-
    // reported values should ever replace what's rendered.
    const defined = Object.fromEntries(
      Object.entries(fields).filter(([, value]) => value !== undefined),
    );
    items = items.map((item) => (item.id === id ? { ...item, ...defined } : item));
  }

  async function init() {
    error = null;
    try {
      items = await listItems();
    } catch (err) {
      error = (err as AppError).message;
    }

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
  }

  async function add(request: AddDownloadRequest) {
    error = null;
    try {
      const { items: added } = await addDownload(request);
      items = [...items, ...added];
    } catch (err) {
      error = (err as AppError).message;
      throw err;
    }
  }

  return {
    get items() {
      return items;
    },
    get error() {
      return error;
    },
    init,
    add,
  };
}

export const queueStore = createQueueStore();
