<script lang="ts">
  // Root: detect binaries on mount; render Onboarding (S1) until both binaries
  // resolve, then the Queue skeleton (S2). Queue store owns the app-wide
  // progress/stage_changed subscriptions (set up once in its own init()).
  import { onMount } from "svelte";
  import { settingsStore } from "./lib/stores/settings.svelte";
  import { queueStore } from "./lib/stores/queue.svelte";
  import Onboarding from "./lib/views/Onboarding.svelte";
  import Queue from "./lib/views/Queue.svelte";

  let ready = $state(false);
  let showQueue = $state(false);

  onMount(async () => {
    await settingsStore.init();
    const binaries = settingsStore.binaries;
    if (binaries?.ytdlp.found && binaries?.ffmpeg.found) {
      showQueue = true;
      await queueStore.init();
    }
    ready = true;
  });

  async function handleContinue() {
    showQueue = true;
    await queueStore.init();
  }
</script>

{#if !ready}
  <main class="loading">Loading…</main>
{:else if showQueue}
  <Queue />
{:else}
  <Onboarding onContinue={handleContinue} />
{/if}

<style>
  .loading {
    color: var(--muted-foreground);
    padding: 2rem;
  }
</style>
