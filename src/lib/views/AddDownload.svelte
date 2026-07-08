<script lang="ts">
  // S3 — Add Download overlay (UX.md, TASKS.md T9). Progressive disclosure:
  // URL input is always visible; the format region unfolds after a probe
  // attempt (success or failure); Advanced (output template/proxy/extra
  // args) stays collapsed by default. Mounted once by Queue.svelte, opened
  // via its "+ Add" button or Ctrl/Cmd+N, closed via Esc or Cancel/Add.
  import { onMount } from "svelte";
  import { queueStore } from "../stores/queue.svelte";
  import { presetsStore } from "../stores/presets.svelte";
  import { probeFormats } from "../ipc";
  import type { AppError, ProbeFormatsResponse } from "../types";
  import FormatQuickPicks from "../components/FormatQuickPicks.svelte";
  import FormatPicker from "../components/FormatPicker.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  // Fallback for the rare moment presetsStore hasn't hydrated yet (or the
  // seeded "Default" preset was somehow deleted, which preset_service
  // refuses via LAST_PRESET) — matches migrations/001_init.sql's seed().
  const FALLBACK_FORMAT_EXPR = "bv*+ba/b";

  let url = $state("");
  let expression = $state(FALLBACK_FORMAT_EXPR);
  let selectedQuickPickId = $state<string | null>(null);
  let probeState = $state<"idle" | "loading" | "success" | "error">("idle");
  let probeResult = $state<ProbeFormatsResponse | null>(null);
  let probeError = $state<AppError | null>(null);
  let adding = $state(false);

  let formatPickerOpen = $state(false);

  let advancedOpen = $state(false);
  let outputTemplate = $state("");
  let proxyOverride = $state("");
  let extraArgs = $state("");

  // S3's preset dropdown (UX.md Flow C step 3, K4-AC3/AC4): applying a
  // preset fills the fields below (still overridable); editing afterwards
  // only changes this plain local state, not the preset row itself — no
  // live binding back to `presetsStore`.
  let selectedPresetId = $state<number | null>(null);

  let urlInputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (open) urlInputEl?.focus();
  });

  // presetsStore may still be hydrating when this component first mounts
  // (Queue.svelte mounts it once, unconditionally) — re-apply the default
  // preset whenever the store's data changes, as long as the user hasn't
  // picked one explicitly (`selectedPresetId` stays null until they do, or
  // until this effect itself sets it).
  $effect(() => {
    if (selectedPresetId == null && presetsStore.defaultPreset) {
      applyPreset(presetsStore.defaultPreset.id);
    }
  });

  function applyPreset(presetId: number) {
    const preset = presetsStore.presets.find((p) => p.id === presetId);
    if (!preset) return;
    selectedPresetId = presetId;
    expression = preset.format_expr;
    selectedQuickPickId = null;
    outputTemplate = preset.output_template;
    proxyOverride = preset.proxy ?? "";
    extraArgs = preset.extra_args ?? "";
  }

  function handlePresetChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value;
    applyPreset(Number(value));
  }

  function reset() {
    url = "";
    expression = FALLBACK_FORMAT_EXPR;
    selectedQuickPickId = null;
    probeState = "idle";
    probeResult = null;
    probeError = null;
    formatPickerOpen = false;
    advancedOpen = false;
    outputTemplate = "";
    proxyOverride = "";
    extraArgs = "";
    // null, not undefined: the reactive default-preset effect above only
    // re-applies when this is exactly null, restoring the seeded default on
    // every close/reopen cycle.
    selectedPresetId = null;
  }

  function close() {
    open = false;
    reset();
  }

  async function probe() {
    if (!url.trim()) return;
    probeState = "loading";
    probeError = null;
    try {
      probeResult = await probeFormats({ url, proxy: proxyOverride.trim() || null });
      probeState = "success";
    } catch (err) {
      probeError = err as AppError;
      probeState = "error";
    }
  }

  async function addClick() {
    if (!url.trim()) return;
    adding = true;
    try {
      await queueStore.add({
        url,
        format_expr: expression.trim() || FALLBACK_FORMAT_EXPR,
        output_template: outputTemplate.trim() || null,
        proxy: proxyOverride.trim() || null,
        extra_args: extraArgs.trim() || null,
        preset_id: selectedPresetId,
      });
      close();
    } catch {
      // error already surfaced via queueStore.error
    } finally {
      adding = false;
    }
  }

  // NFR-5: Ctrl/Cmd+N opens S3 from anywhere; Esc closes the topmost overlay.
  function handleKeydown(e: KeyboardEvent) {
    const isModN = (e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "n";
    if (isModN) {
      e.preventDefault();
      open = true;
    } else if (e.key === "Escape" && open) {
      e.preventDefault();
      close();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scrim" onclick={close}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="overlay"
      role="dialog"
      aria-modal="true"
      aria-label="Add Download"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
    >
      <header>
        <h2>Add Download</h2>
        <button type="button" class="icon-btn" onclick={close} aria-label="Close">✕</button>
      </header>

      <label class="url-field">
        <span>URL(s)</span>
        <div class="url-row">
          <input
            type="text"
            bind:value={url}
            bind:this={urlInputEl}
            placeholder="https://…"
          />
          <button type="button" disabled={!url.trim() || probeState === "loading"} onclick={probe}>
            {probeState === "loading" ? "Probing…" : "Probe ▸"}
          </button>
        </div>
        <p class="hint">Paste a playlist and it expands to N items on Add.</p>
      </label>

      <label class="preset-field">
        <span>Preset</span>
        <select value={selectedPresetId ?? ""} onchange={handlePresetChange}>
          {#each presetsStore.presets as preset (preset.id)}
            <option value={preset.id}>{preset.name}{preset.is_default ? " (default)" : ""}</option>
          {/each}
        </select>
      </label>

      {#if probeState !== "idle"}
        <div class="format-section">
          <div class="format-section-header">
            <span class="section-label">Format</span>
            {#if probeState === "success" && probeResult}
              <button type="button" class="format-picker-link" onclick={() => (formatPickerOpen = true)}>
                Format Picker
              </button>
            {/if}
          </div>
          {#if probeState === "loading"}
            <div class="skeleton" aria-hidden="true">
              <div class="skeleton-row"></div>
              <div class="skeleton-row"></div>
            </div>
          {:else if probeState === "error"}
            <div class="probe-error">
              <pre class="stderr">{probeError?.stderr || probeError?.message}</pre>
              <button type="button" onclick={probe}>Retry</button>
            </div>
          {:else if probeState === "success" && probeResult}
            <FormatQuickPicks
              formats={probeResult.formats}
              bind:expression
              bind:selectedQuickPickId
            />
          {/if}
        </div>
      {/if}

      {#if probeResult}
        <FormatPicker
          bind:open={formatPickerOpen}
          formats={probeResult.formats}
          title={probeResult.title}
          bind:expression
          bind:selectedQuickPickId
        />
      {/if}

      <button
        type="button"
        class="advanced-toggle"
        aria-expanded={advancedOpen}
        onclick={() => (advancedOpen = !advancedOpen)}
      >
        {advancedOpen ? "▾" : "▸"} Advanced
      </button>
      {#if advancedOpen}
        <div class="advanced">
          <label>
            <span>Output template</span>
            <input type="text" class="mono" bind:value={outputTemplate} placeholder="%(title)s.%(ext)s" />
          </label>
          <label>
            <span>Proxy override</span>
            <input type="text" bind:value={proxyOverride} placeholder="http://…" />
          </label>
          <label>
            <span>Extra CLI args</span>
            <input type="text" class="mono" bind:value={extraArgs} placeholder="--no-mtime" />
          </label>
        </div>
      {/if}

      <footer>
        <button type="button" onclick={close}>Cancel</button>
        <button type="button" disabled={!url.trim() || adding} onclick={addClick}>Add to queue</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: color-mix(in srgb, var(--surface-lowest) 70%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }
  .overlay {
    background: var(--card);
    color: var(--card-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 1.25rem;
    width: 32rem;
    max-width: calc(100vw - 2rem);
    max-height: calc(100vh - 4rem);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  header h2 {
    margin: 0;
    font-size: 1.1em;
  }
  .icon-btn {
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    font-size: 1em;
    padding: 0.2rem 0.4rem;
  }
  .icon-btn:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .url-field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .url-row {
    display: flex;
    gap: 0.5rem;
  }
  .url-row input {
    flex: 1;
  }
  .hint {
    margin: 0;
    color: var(--muted-foreground);
    font-size: 0.8em;
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
  input.mono {
    font-family: var(--font-mono);
  }
  input:focus-visible,
  button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  button {
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .format-section {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .section-label {
    color: var(--muted-foreground);
    font-size: 0.85em;
  }
  .format-section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .format-picker-link {
    background: transparent;
    border: none;
    color: var(--primary);
    padding: 0;
    font-size: 0.85em;
    text-decoration: underline;
  }
  .skeleton {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .skeleton-row {
    height: 1.6rem;
    border-radius: var(--radius);
    background: var(--accent);
    opacity: 0.5;
  }
  .probe-error {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .stderr {
    margin: 0;
    color: var(--error-token);
    background: var(--muted);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.6rem;
    font-family: var(--font-mono);
    font-size: 0.85em;
    white-space: pre-wrap;
    max-height: 8rem;
    overflow-y: auto;
  }
  .advanced-toggle {
    align-self: flex-start;
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    padding: 0;
    font-size: 0.85em;
  }
  .advanced {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .advanced label,
  .url-field span,
  .preset-field,
  .format-section span {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85em;
    color: var(--muted-foreground);
  }
  .preset-field select {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4rem 0.6rem;
    font-family: var(--font-sans);
  }
  .preset-field select:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
