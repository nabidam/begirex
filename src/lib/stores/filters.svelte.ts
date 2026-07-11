// Filters store (ARCHITECTURE §2) — pure frontend UI state, never
// hydrated from the backend: the status filter, title search, and sidebar
// rail collapse are view concerns only, not durable data. queue.svelte.ts
// remains the single source of truth for item state; this store only
// decides which of those items are currently visible.
import type { Item } from "../types";

export type StatusFilter = "all" | "downloading" | "queued" | "paused" | "completed" | "failed" | "cancelled";

export const STATUS_FILTERS: StatusFilter[] = [
  "all",
  "downloading",
  "queued",
  "paused",
  "completed",
  "failed",
  "cancelled",
];

// "Active" in the sidebar covers both `downloading` and `merging`; terminal
// cancellation remains separately recoverable rather than disappearing into All.
const STAGES_BY_FILTER: Record<Exclude<StatusFilter, "all">, string[]> = {
  downloading: ["downloading", "merging"],
  queued: ["queued"],
  paused: ["paused"],
  completed: ["completed"],
  failed: ["error"],
  cancelled: ["cancelled"],
};

function createFiltersStore() {
  let status = $state<StatusFilter>("all");
  let search = $state("");
  let collapsed = $state(false);

  function matchesStatus(item: Item, filter: StatusFilter): boolean {
    if (filter === "all") return true;
    return STAGES_BY_FILTER[filter].includes(item.stage);
  }

  function matchesSearch(item: Item, term: string): boolean {
    const needle = term.trim().toLowerCase();
    if (!needle) return true;
    return (item.title ?? item.url).toLowerCase().includes(needle);
  }

  function matches(item: Item): boolean {
    return matchesStatus(item, status) && matchesSearch(item, search);
  }

  function countFor(filter: StatusFilter, items: Item[]): number {
    return filter === "all" ? items.length : items.filter((item) => matchesStatus(item, filter)).length;
  }

  function setStatus(next: StatusFilter) {
    status = next;
  }

  function setSearch(term: string) {
    search = term;
  }

  function reset() {
    status = "all";
    search = "";
  }

  function toggleCollapsed() {
    collapsed = !collapsed;
  }

  return {
    get status() {
      return status;
    },
    get search() {
      return search;
    },
    get collapsed() {
      return collapsed;
    },
    matches,
    countFor,
    setStatus,
    setSearch,
    reset,
    toggleCollapsed,
  };
}

export const filtersStore = createFiltersStore();
