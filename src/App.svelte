<script lang="ts">
  // Root: detect binaries on mount; render Onboarding (S1) until both binaries
  // resolve, then the Queue skeleton (S2). Queue store owns the app-wide
  // progress/stage_changed subscriptions (set up once in its own init()).
  import { onMount } from "svelte";
  import { settingsStore } from "./lib/stores/settings.svelte";
  import { queueStore } from "./lib/stores/queue.svelte";
  import { presetsStore } from "./lib/stores/presets.svelte";
  import { binaryHealthStore } from "./lib/stores/binaryHealth.svelte";
  import Onboarding from "./lib/views/Onboarding.svelte";
  import Shell from "./lib/views/Shell.svelte";
  import GlobalBanner from "./lib/components/GlobalBanner.svelte";
  import { Toaster } from "$lib/components/ui/sonner";

  let ready = $state(false);
  let showQueue = $state(false);

  onMount(async () => {
    binaryHealthStore.init();
    await settingsStore.init();
    const binaries = settingsStore.binaries;
    if (binaries?.ytdlp.found && binaries?.ffmpeg.found) {
      showQueue = true;
      await Promise.all([queueStore.init(), presetsStore.init()]);
    }
    ready = true;
  });

  async function handleContinue() {
    showQueue = true;
    await Promise.all([queueStore.init(), presetsStore.init()]);
  }

  // T17 AC2: "I'll set it later" lands on S2 in degraded read-only mode
  // (Shell/Sidebar/Queue disable Add while a binary is unresolved) rather
  // than blocking on bothFound like Continue does.
  async function handleSkip() {
    showQueue = true;
    await Promise.all([queueStore.init(), presetsStore.init()]);
  }

  // S7 AC3: "Re-run onboarding reopens S1 with current state."
  async function handleReRunOnboarding() {
    showQueue = false;
    await settingsStore.init();
  }

  // K1-AC7: GlobalBanner's Fix button reopens S1 (UX.md S7 states). Re-runs
  // detect_binaries first (Onboarding only reads settingsStore's already-
  // hydrated state, it doesn't detect on its own) so S1 shows the real
  // current status rather than what was true at app launch.
  async function handleFix() {
    binaryHealthStore.clear();
    showQueue = false;
    await settingsStore.init();
  }
</script>

<GlobalBanner onFix={handleFix} />
<Toaster />

{#if !ready}
  <main class="loading">Loading…</main>
{:else if showQueue}
  <Shell onReRunOnboarding={handleReRunOnboarding} />
{:else}
  <Onboarding onContinue={handleContinue} onSkip={handleSkip} />
{/if}

<style>
  .loading {
    color: var(--muted-foreground);
    padding: 2rem;
  }
</style>
