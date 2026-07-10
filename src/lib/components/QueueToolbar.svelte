<script lang="ts">
  // Toolbar (UX.md S2) — title search, inline concurrency (N) control, and
  // global Start all / Pause all, all operating on the *visible* (filtered)
  // queue per UX.md's "operate on the visible queue".
  import { filtersStore } from "../stores/filters.svelte";
  import { queueStore } from "../stores/queue.svelte";
  import { settingsStore } from "../stores/settings.svelte";
  import type { Item } from "../types";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import Search from "lucide-svelte/icons/search";
  import Play from "lucide-svelte/icons/play";
  import Pause from "lucide-svelte/icons/pause";

  let { visibleItems }: { visibleItems: Item[] } = $props();

  const PAUSABLE = new Set(["downloading", "merging", "queued"]);

  const pauseIds = $derived(visibleItems.filter((i) => PAUSABLE.has(i.stage)).map((i) => i.id));
  const resumeIds = $derived(visibleItems.filter((i) => i.stage === "paused").map((i) => i.id));

  const displayedN = $derived(queueStore.concurrency ?? settingsStore.settings?.default_concurrency ?? 2);

  function onNInput(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    if (Number.isInteger(value) && value >= 1) {
      queueStore.setConcurrency(value);
    }
  }
</script>

<div class="flex items-center gap-3 border-b border-border px-4 py-2.5">
  <div class="relative min-w-0 max-w-96 flex-1">
    <Search aria-hidden="true" class="pointer-events-none absolute inset-y-0 start-2.5 my-auto size-4 text-muted-foreground" />
    <Input
      type="search"
      placeholder="Search title…"
      value={filtersStore.search}
      oninput={(e) => filtersStore.setSearch((e.currentTarget as HTMLInputElement).value)}
      aria-label="Search queue by title"
      class="ps-8"
    />
  </div>

  <label class="flex items-center gap-1.5 text-sm text-muted-foreground">
    <span>N</span>
    <Input
      type="number"
      min="1"
      value={displayedN}
      onchange={onNInput}
      aria-label="Concurrent downloads"
      class="w-14 font-mono"
    />
  </label>

  <div class="ms-auto flex gap-1.5">
    <Button
      type="button"
      variant="outline"
      disabled={resumeIds.length === 0}
      onclick={() => queueStore.resumeAll(resumeIds)}
    >
      <Play aria-hidden="true" />
      Start all
    </Button>
    <Button
      type="button"
      variant="outline"
      disabled={pauseIds.length === 0}
      onclick={() => queueStore.pauseAll(pauseIds)}
    >
      <Pause aria-hidden="true" />
      Pause all
    </Button>
  </div>
</div>
