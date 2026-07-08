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

<div class="shell">
  <Sidebar
    items={queueStore.items}
    {collapsed}
    onAdd={openAdd}
    onOpenPresets={() => (showPresets = true)}
    onOpenSettings={() => (showSettings = true)}
    addDisabled={downloadsDisabled}
  />

  <div class="main">
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
  <div class="scrim" onclick={() => (showPresets = false)}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="presets-overlay" onclick={(e) => e.stopPropagation()}>
      <button type="button" class="icon-btn" onclick={() => (showPresets = false)} aria-label="Close">✕</button>
      <Presets />
    </div>
  </div>
{/if}

{#if showSettings}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scrim" onclick={() => (showSettings = false)}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="presets-overlay" onclick={(e) => e.stopPropagation()}>
      <button type="button" class="icon-btn" onclick={() => (showSettings = false)} aria-label="Close">✕</button>
      <Settings
        onReRunOnboarding={() => {
          showSettings = false;
          onReRunOnboarding();
        }}
      />
    </div>
  </div>
{/if}

<style>
  .shell {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }
  .scrim {
    position: fixed;
    inset: 0;
    background: color-mix(in srgb, var(--surface-lowest) 70%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }
  .presets-overlay {
    position: relative;
    background: var(--surface-lowest);
    border-radius: var(--radius);
    max-height: calc(100vh - 4rem);
    overflow-y: auto;
  }
  .icon-btn {
    position: absolute;
    top: 0.75rem;
    inset-inline-end: 0.75rem;
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    font-size: 1em;
    padding: 0.2rem 0.4rem;
    z-index: 1;
  }
  .icon-btn:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>
