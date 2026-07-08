<script lang="ts">
  // S1 minimal onboarding (ARCHITECTURE §10 Flow A steps 1-2, TASKS.md T4).
  // Blocks until both binaries resolve found:true, then hands off to Queue.
  import { settingsStore } from "../stores/settings.svelte";
  import { pickBinaryPath } from "../ipc";

  let { onContinue }: { onContinue: () => void } = $props();

  let proxy = $state(settingsStore.settings?.global_proxy ?? "");
  let continuing = $state(false);

  const bothFound = $derived(
    settingsStore.binaries?.ytdlp.found === true && settingsStore.binaries?.ffmpeg.found === true,
  );

  async function setPath(which: "ytdlp" | "ffmpeg") {
    const path = await pickBinaryPath();
    if (path) {
      await settingsStore.resolveBinaryPath(which, path);
    }
  }

  async function continueClick() {
    continuing = true;
    try {
      await settingsStore.saveProxy(proxy);
      if (!settingsStore.error) onContinue();
    } finally {
      continuing = false;
    }
  }
</script>

<main class="onboarding">
  <h1>Welcome to BegireX</h1>
  <p>Locate the required binaries before you can start downloading.</p>

  {#if settingsStore.error}
    <p class="error">{settingsStore.error}</p>
  {/if}

  {#each [["ytdlp", "yt-dlp"], ["ffmpeg", "ffmpeg"]] as [key, label] (key)}
    {@const status = settingsStore.binaries?.[key as "ytdlp" | "ffmpeg"]}
    <div class="binary-row">
      <span class="label">{label}</span>
      {#if status?.found}
        <span class="status ok">found — {status.path} {status.version ? `(${status.version})` : ""}</span>
      {:else}
        <span class="status missing">not found</span>
        <button onclick={() => setPath(key as "ytdlp" | "ffmpeg")}>Set path…</button>
      {/if}
    </div>
  {/each}

  <label class="proxy-field">
    Proxy (optional)
    <input type="text" bind:value={proxy} placeholder="http://host:port" />
  </label>

  <button disabled={!bothFound || continuing} onclick={continueClick}>Continue</button>
</main>

<style>
  .onboarding {
    max-width: 32rem;
    margin: 4rem auto;
    padding: 1.5rem;
    background: var(--card);
    color: var(--card-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .binary-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin: 0.75rem 0;
  }
  .label {
    width: 5rem;
    font-weight: 700;
  }
  .status.ok {
    color: var(--primary);
  }
  .status.missing {
    color: var(--warning);
  }
  .proxy-field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin: 1.25rem 0;
  }
  input,
  button {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4rem 0.6rem;
    font-family: var(--font-sans);
  }
  button {
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .error {
    color: var(--error-token);
  }
</style>
