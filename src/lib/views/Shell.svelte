<script lang="ts">
  // Shell (UX.md S2, TASKS.md T13) — the persistent app chrome: sidebar,
  // toolbar, and the queue list region, plus the Add/Presets overlays that
  // used to live directly in Queue.svelte (T9/T11's ponytail stopgap —
  // this is that upgrade). Mounted from App.svelte; this repo has no
  // SvelteKit `src/routes/` (confirmed at T4/T9), so App.svelte is the
  // real rewire target the task's "+page.svelte" file entry maps to.
  import { onMount } from "svelte";
  import { queueStore } from "../stores/queue.svelte";
  import { filtersStore } from "../stores/filters.svelte";
  import { settingsStore } from "../stores/settings.svelte";
  import Sidebar from "../components/Sidebar.svelte";
  import QueueToolbar from "../components/QueueToolbar.svelte";
  import Queue from "./Queue.svelte";
  import DetailDrawer from "./DetailDrawer.svelte";
  import AddDownload from "./AddDownload.svelte";
  import Presets from "./Presets.svelte";
  import Settings from "./Settings.svelte";
  import { Button } from "$lib/components/ui/button";
  import X from "lucide-svelte/icons/x";

  let { onReRunOnboarding }: { onReRunOnboarding: () => void } = $props();

  let showAddDownload = $state(false);
  let showPresets = $state(false);
  let showSettings = $state(false);
  let innerWidth = $state(window.innerWidth);

  // DESIGN.md §6: rail collapses below ~1100px window width or by toggle.
  const autoCollapse = $derived(innerWidth < 1100);
  const collapsed = $derived(filtersStore.collapsed || autoCollapse);

  const visibleItems = $derived(queueStore.items.filter((item) => filtersStore.matches(item)));

  // T17 AC2: "I'll set it later" lands here in degraded read-only mode —
  // Add disabled with an explanation until both binaries resolve (via S7's
  // Re-check/Change… or Re-run onboarding).
  const downloadsDisabled = $derived(
    settingsStore.settings?.build_flavor !== "bundled" &&
      !(settingsStore.binaries?.ytdlp.found && settingsStore.binaries?.ffmpeg.found),
  );

  function openAdd() {
    if (downloadsDisabled) return;
    showAddDownload = true;
  }

  function showAll() {
    filtersStore.reset();
  }

  // UX.md S7: `Ctrl/Cmd+,` opens Settings.
  function handleKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === ",") {
      e.preventDefault();
      showSettings = true;
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });
</script>

<svelte:window bind:innerWidth />

<div class="flex h-screen overflow-hidden">
  <Sidebar
    items={queueStore.items}
    {collapsed}
    onAdd={openAdd}
    onOpenPresets={() => (showPresets = true)}
    onOpenSettings={() => (showSettings = true)}
    addDisabled={downloadsDisabled}
  />

  <div class="flex min-w-0 flex-1 flex-col overflow-y-auto">
    <QueueToolbar {visibleItems} />
    <Queue
      items={visibleItems}
      totalCount={queueStore.items.length}
      onAdd={openAdd}
      onShowAll={showAll}
      addDisabled={downloadsDisabled}
    />
  </div>
</div>

<DetailDrawer />

<AddDownload bind:open={showAddDownload} />

{#if showPresets}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-[color-mix(in_srgb,var(--surface-lowest)_70%,transparent)]"
    onclick={() => (showPresets = false)}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="relative max-h-[calc(100vh-4rem)] overflow-y-auto rounded-lg bg-[var(--surface-lowest)]"
      onclick={(e) => e.stopPropagation()}
    >
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class="absolute end-3 top-3 z-10 text-muted-foreground"
        onclick={() => (showPresets = false)}
        aria-label="Close"
      >
        <X aria-hidden="true" />
      </Button>
      <Presets />
    </div>
  </div>
{/if}

{#if showSettings}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-[color-mix(in_srgb,var(--surface-lowest)_70%,transparent)]"
    onclick={() => (showSettings = false)}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="relative max-h-[calc(100vh-4rem)] overflow-y-auto rounded-lg bg-[var(--surface-lowest)]"
      onclick={(e) => e.stopPropagation()}
    >
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class="absolute end-3 top-3 z-10 text-muted-foreground"
        onclick={() => (showSettings = false)}
        aria-label="Close"
      >
        <X aria-hidden="true" />
      </Button>
      <Settings
        onReRunOnboarding={() => {
          showSettings = false;
          onReRunOnboarding();
        }}
      />
    </div>
  </div>
{/if}
