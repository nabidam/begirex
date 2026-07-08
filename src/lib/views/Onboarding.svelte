<script lang="ts">
  // S1 — First-run Onboarding, full wizard (UX.md S1, TASKS.md T17,
  // replacing T4's minimal blocking version). Region 1: engine check, one
  // BinaryRow per binary with found/downloading/error states. Region 2:
  // optional global proxy. Continue is gated on both binaries resolved;
  // "I'll set it later" is the degraded-mode escape hatch (AC2) — App.svelte
  // lands on S2 without requiring bothFound.
  import { settingsStore } from "../stores/settings.svelte";
  import { pickBinaryPath } from "../ipc";
  import BinaryRow from "../components/BinaryRow.svelte";

  let { onContinue, onSkip }: { onContinue: () => void; onSkip: () => void } = $props();

  let proxy = $state(settingsStore.settings?.global_proxy ?? "");
  let continuing = $state(false);

  const BINARIES: Array<["ytdlp" | "ffmpeg", string]> = [
    ["ytdlp", "yt-dlp"],
    ["ffmpeg", "ffmpeg"],
  ];

  const bothFound = $derived(
    settingsStore.settings?.build_flavor === "bundled" ||
      (settingsStore.binaries?.ytdlp.found === true && settingsStore.binaries?.ffmpeg.found === true),
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
  <header>
    <h1>BegireX · First-time setup</h1>
  </header>

  {#if settingsStore.error}
    <p class="error">{settingsStore.error}</p>
  {/if}

  <section class="region">
    <h2>Engine check</h2>
    {#if settingsStore.settings?.build_flavor === "bundled"}
      <!-- UX.md S1 density note: the bundled build skips detection entirely
           (ARCHITECTURE §9 seeds ytdlp_path/ffmpeg_path to its shipped
           binaries) — this screen only fully appears for the light build. -->
      <p class="bundled-line">Engine bundled ✓</p>
    {:else}
      <div class="rows">
        {#each BINARIES as [which, label] (which)}
          <BinaryRow
            {label}
            status={settingsStore.binaries?.[which]}
            onSetPath={() => setPath(which)}
            onDownload={() => settingsStore.downloadBinary(which)}
            downloadState={settingsStore.downloads[which]}
          />
        {/each}
      </div>
    {/if}
  </section>

  <section class="region">
    <h2>Network <span class="optional">(optional)</span></h2>
    <label class="proxy-field">
      <span>Proxy</span>
      <input type="text" bind:value={proxy} placeholder="socks5://user:pass@host:port" />
    </label>
  </section>

  <footer>
    <button type="button" class="ghost" onclick={onSkip}>I'll set it later</button>
    <button type="button" class="primary" disabled={!bothFound || continuing} onclick={continueClick}>
      {continuing ? "Continuing…" : "Continue"}
    </button>
  </footer>
</main>

<style>
  .onboarding {
    max-width: 34rem;
    margin: 3rem auto;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    background: var(--card);
    color: var(--card-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  header h1 {
    margin: 0;
    font-size: 1.1em;
  }
  .region {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .region h2 {
    margin: 0;
    font-size: 0.9em;
    text-transform: uppercase;
    letter-spacing: 0.02em;
    color: var(--muted-foreground);
  }
  .optional {
    text-transform: none;
    letter-spacing: normal;
    font-weight: 400;
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .bundled-line {
    margin: 0;
    color: var(--primary);
    font-family: var(--font-mono);
  }
  .proxy-field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  input {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4rem 0.6rem;
    font-family: var(--font-sans);
  }
  input:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
  }
  button {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.5rem 0.9rem;
    font-family: var(--font-sans);
    cursor: pointer;
  }
  button.ghost {
    background: transparent;
  }
  button.primary {
    background: var(--primary);
    color: var(--primary-foreground);
    border-color: var(--primary);
    font-weight: 700;
  }
  button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .error {
    color: var(--error-token);
    margin: 0;
  }
</style>
