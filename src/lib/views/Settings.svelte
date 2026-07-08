<script lang="ts">
  // S7 — Settings (UX.md, TASKS.md T17). Region 1 Engine & health (→ eye
  // first): BinaryRow per binary (no onDownload — S7 never re-triggers an
  // in-app fetch, missing-mid-session is GlobalBanner's job) + Re-check +
  // Re-run onboarding. Region 2 Downloads: N/output dir/template/default
  // preset. Region 3 Network: global proxy. Region 4 About: build flavor +
  // versions.
  import { settingsStore } from "../stores/settings.svelte";
  import { presetsStore } from "../stores/presets.svelte";
  import { pickBinaryPath, pickDirectory } from "../ipc";
  import BinaryRow from "../components/BinaryRow.svelte";

  let { onReRunOnboarding }: { onReRunOnboarding: () => void } = $props();

  const BINARIES: Array<["ytdlp" | "ffmpeg", string]> = [
    ["ytdlp", "yt-dlp"],
    ["ffmpeg", "ffmpeg"],
  ];

  let concurrency = $state(settingsStore.settings?.default_concurrency ?? 2);
  let outputDir = $state(settingsStore.settings?.default_output_dir ?? "");
  let outputTemplate = $state(settingsStore.settings?.default_output_template ?? "");
  let proxy = $state(settingsStore.settings?.global_proxy ?? "");
  // T17 ambiguity note (internal, no user-visible behavior change): S7's
  // "Default preset" field is wired to presetsStore.setDefault (the
  // `is_default` flag S3/S6 actually read — AddDownload.svelte picks
  // presetsStore.defaultPreset, not settings.default_preset_id) rather than
  // settings.update_settings{default_preset_id}, so there is exactly one
  // source of truth for "the default preset" instead of two disconnected
  // ones with the same name.
  let defaultPresetId = $state(presetsStore.defaultPreset?.id ?? "");

  let savedField = $state<string | null>(null);
  let recheckState = $state<"idle" | "checking">("idle");

  function flashSaved(field: string) {
    savedField = field;
    setTimeout(() => {
      if (savedField === field) savedField = null;
    }, 1500);
  }

  async function changePath(which: "ytdlp" | "ffmpeg") {
    const path = await pickBinaryPath();
    if (path) await settingsStore.resolveBinaryPath(which, path);
  }

  async function recheck() {
    recheckState = "checking";
    try {
      await settingsStore.recheck();
    } finally {
      recheckState = "idle";
    }
  }

  async function saveConcurrency() {
    if (Number.isInteger(concurrency) && concurrency >= 1) {
      const ok = await settingsStore.update({ default_concurrency: concurrency });
      if (ok) flashSaved("concurrency");
    }
  }

  async function pickOutputDir() {
    const dir = await pickDirectory();
    if (dir) {
      outputDir = dir;
      const ok = await settingsStore.update({ default_output_dir: dir });
      if (ok) flashSaved("output_dir");
    }
  }

  async function saveOutputTemplate() {
    if (outputTemplate.trim()) {
      const ok = await settingsStore.update({ default_output_template: outputTemplate.trim() });
      if (ok) flashSaved("output_template");
    }
  }

  async function saveProxy() {
    const ok = await settingsStore.update({ global_proxy: proxy.trim() || null });
    if (ok) flashSaved("proxy");
  }

  async function handleDefaultPresetChange(event: Event) {
    const id = Number((event.currentTarget as HTMLSelectElement).value);
    defaultPresetId = id;
    const ok = await presetsStore.setDefault(id);
    if (ok) flashSaved("default_preset");
  }
</script>

<main class="settings">
  <h1>Settings</h1>

  {#if settingsStore.error}
    <p class="error">{settingsStore.error}</p>
  {/if}

  <section class="card">
    <h2>Engine &amp; health</h2>
    <div class="rows">
      {#each BINARIES as [which, label] (which)}
        <BinaryRow label={label} status={settingsStore.binaries?.[which]} onSetPath={() => changePath(which)} />
      {/each}
    </div>
    <div class="engine-actions">
      <button type="button" onclick={recheck} disabled={recheckState === "checking"}>
        {recheckState === "checking" ? "Re-checking…" : "Re-check"}
      </button>
      <button type="button" onclick={onReRunOnboarding}>Re-run onboarding</button>
    </div>
  </section>

  <section class="card">
    <h2>Downloads</h2>
    <label class="field">
      <span>Parallel downloads (N)</span>
      <input
        type="number"
        min="1"
        bind:value={concurrency}
        onchange={saveConcurrency}
        aria-label="Default parallel downloads"
      />
      {#if savedField === "concurrency"}<span class="saved">Saved</span>{/if}
    </label>
    <label class="field">
      <span>Default output dir</span>
      <div class="with-button">
        <input type="text" bind:value={outputDir} readonly />
        <button type="button" onclick={pickOutputDir}>…</button>
      </div>
      {#if savedField === "output_dir"}<span class="saved">Saved</span>{/if}
    </label>
    <label class="field">
      <span>Default filename tmpl</span>
      <input
        type="text"
        class="mono"
        bind:value={outputTemplate}
        onchange={saveOutputTemplate}
        placeholder="%(title)s.%(ext)s"
      />
      {#if savedField === "output_template"}<span class="saved">Saved</span>{/if}
    </label>
    <label class="field">
      <span>Default preset</span>
      <select value={defaultPresetId} onchange={handleDefaultPresetChange}>
        {#each presetsStore.presets as preset (preset.id)}
          <option value={preset.id}>{preset.name}</option>
        {/each}
      </select>
      {#if savedField === "default_preset"}<span class="saved">Saved</span>{/if}
    </label>
  </section>

  <section class="card">
    <h2>Network</h2>
    <label class="field">
      <span>Global proxy</span>
      <input type="text" bind:value={proxy} onchange={saveProxy} placeholder="socks5://user:pass@host:port" />
      {#if savedField === "proxy"}<span class="saved">Saved</span>{/if}
    </label>
  </section>

  <section class="card">
    <h2>About</h2>
    <p class="about-line mono">
      Build: {settingsStore.settings?.build_flavor ?? "—"} · BegireX 0.1.0
      {#if settingsStore.settings?.ytdlp_version}· yt-dlp {settingsStore.settings.ytdlp_version}{/if}
      {#if settingsStore.settings?.ffmpeg_version}· ffmpeg {settingsStore.settings.ffmpeg_version}{/if}
    </p>
  </section>
</main>

<style>
  .settings {
    max-width: 42rem;
    margin: 2rem auto;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }
  h1 {
    margin: 0;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    background: var(--card);
    color: var(--card-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 1rem;
  }
  .card h2 {
    margin: 0;
    font-size: 0.9em;
    text-transform: uppercase;
    letter-spacing: 0.02em;
    color: var(--muted-foreground);
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .engine-actions {
    display: flex;
    gap: 0.5rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85em;
    color: var(--muted-foreground);
    max-width: 26rem;
  }
  .with-button {
    display: flex;
    gap: 0.4rem;
  }
  .with-button input {
    flex: 1;
    min-width: 0;
  }
  .saved {
    color: var(--primary);
    font-size: 0.85em;
  }
  .about-line {
    margin: 0;
    color: var(--foreground);
  }
  input,
  select,
  button {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4rem 0.6rem;
    font-family: var(--font-sans);
  }
  input.mono {
    font-family: var(--font-mono);
  }
  .mono {
    font-family: var(--font-mono);
  }
  input:focus-visible,
  select:focus-visible,
  button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  button {
    cursor: pointer;
  }
  .error {
    color: var(--error-token);
    margin: 0;
  }
</style>
