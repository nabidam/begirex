<script lang="ts">
  // S6 — Presets (UX.md, TASKS.md T11). Region 1: named-preset list, default
  // starred + sorted first (eye first). Region 2: inline editor that opens on
  // row select, matching UX.md's "EDITOR (inline, on select)" layout — no
  // separate route/modal.
  import { presetsStore } from "../stores/presets.svelte";
  import type { Preset } from "../types";

  let selectedId = $state<number | null>(null);
  let creating = $state(false);

  let name = $state("");
  let isDefault = $state(false);
  let formatExpr = $state("");
  let outputTemplate = $state("");
  let proxy = $state("");
  let extraArgs = $state("");

  let saveError = $state<string | null>(null);
  let saveErrorStderr = $state<string | null>(null);
  let saving = $state(false);
  let deleteError = $state<string | null>(null);

  const selected = $derived(presetsStore.presets.find((p) => p.id === selectedId) ?? null);

  function selectPreset(preset: Preset) {
    creating = false;
    selectedId = preset.id;
    name = preset.name;
    isDefault = preset.is_default;
    formatExpr = preset.format_expr;
    outputTemplate = preset.output_template;
    proxy = preset.proxy ?? "";
    extraArgs = preset.extra_args ?? "";
    saveError = null;
    saveErrorStderr = null;
    deleteError = null;
  }

  function startCreate() {
    creating = true;
    selectedId = null;
    name = "";
    isDefault = false;
    formatExpr = "";
    outputTemplate = "%(title)s.%(ext)s";
    proxy = "";
    extraArgs = "";
    saveError = null;
    saveErrorStderr = null;
    deleteError = null;
  }

  function closeEditor() {
    creating = false;
    selectedId = null;
  }

  async function save() {
    saveError = null;
    saveErrorStderr = null;
    saving = true;
    try {
      if (creating) {
        const preset = await presetsStore.create({
          name,
          format_expr: formatExpr,
          output_template: outputTemplate,
          proxy: proxy.trim() || null,
          extra_args: extraArgs.trim() || null,
          is_default: isDefault,
        });
        if (preset) {
          selectPreset(preset);
        } else if (presetsStore.error) {
          saveError = presetsStore.error.message;
          saveErrorStderr = presetsStore.error.stderr ?? null;
        }
      } else if (selectedId != null) {
        const preset = await presetsStore.update({
          id: selectedId,
          name,
          format_expr: formatExpr,
          output_template: outputTemplate,
          proxy: proxy.trim() || null,
          extra_args: extraArgs.trim() || null,
        });
        if (preset) {
          if (isDefault && !preset.is_default) {
            await presetsStore.setDefault(preset.id);
          }
          const refreshed = presetsStore.presets.find((p) => p.id === preset.id);
          if (refreshed) selectPreset(refreshed);
        } else if (presetsStore.error) {
          saveError = presetsStore.error.message;
          saveErrorStderr = presetsStore.error.stderr ?? null;
        }
      }
    } finally {
      saving = false;
    }
  }

  async function deleteSelected() {
    if (selectedId == null) return;
    deleteError = null;
    // ponytail: native confirm() stands in for a proper confirm dialog
    // component — UX.md says "Delete confirms"; this satisfies that with the
    // simplest primitive available. Upgrade path: a shared ConfirmDialog
    // once a second destructive action needs one (T14 remove/cancel).
    if (!confirm(`Delete preset "${name}"?`)) return;
    const ok = await presetsStore.remove(selectedId);
    if (ok) {
      closeEditor();
    } else if (presetsStore.error) {
      deleteError = presetsStore.error.message;
    }
  }
</script>

<main class="presets">
  <div class="header-row">
    <h1>Presets</h1>
    <button type="button" class="new-btn" onclick={startCreate}>+ New preset</button>
  </div>

  {#if presetsStore.error && !saveError && !deleteError}
    <p class="error">{presetsStore.error.message}</p>
  {/if}

  <ul class="preset-list">
    {#each presetsStore.presets as preset (preset.id)}
      <li>
        <button
          type="button"
          class="preset-row"
          class:selected={preset.id === selectedId}
          onclick={() => selectPreset(preset)}
        >
          <span class="star" aria-hidden="true">{preset.is_default ? "★" : ""}</span>
          <span class="name">{preset.name}</span>
          {#if preset.is_default}<span class="default-tag">default</span>{/if}
          <span class="expr">{preset.format_expr}</span>
        </button>
      </li>
    {:else}
      <li class="empty">No presets yet.</li>
    {/each}
  </ul>

  {#if creating || selected}
    <div class="editor">
      <label>
        <span>Name</span>
        <input type="text" bind:value={name} placeholder="4K" />
      </label>
      <label class="checkbox-field">
        <input type="checkbox" bind:checked={isDefault} />
        <span>default</span>
      </label>
      <label>
        <span>Format expr</span>
        <input type="text" class="mono" bind:value={formatExpr} placeholder="bv*[height<=2160]+ba/b" />
      </label>
      <label>
        <span>Output tmpl</span>
        <input type="text" class="mono" bind:value={outputTemplate} placeholder="%(title)s.%(ext)s" />
      </label>
      <label>
        <span>Proxy</span>
        <input type="text" bind:value={proxy} placeholder="(inherit global)" />
      </label>
      <label>
        <span>Extra args</span>
        <input type="text" class="mono" bind:value={extraArgs} placeholder="--embed-thumbnail" />
      </label>

      {#if saveError}
        <div class="save-error">
          <p class="error">{saveError}</p>
          {#if saveErrorStderr}
            <pre class="stderr">{saveErrorStderr}</pre>
          {/if}
        </div>
      {/if}
      {#if deleteError}
        <p class="error">{deleteError}</p>
      {/if}

      <footer>
        {#if !creating}
          <button type="button" class="delete-btn" onclick={deleteSelected}>Delete</button>
        {/if}
        <button type="button" onclick={closeEditor}>Cancel</button>
        <button
          type="button"
          class="save-btn"
          disabled={!name.trim() || !formatExpr.trim() || !outputTemplate.trim() || saving}
          onclick={save}
        >
          {saving ? "Saving…" : "Save"}
        </button>
      </footer>
    </div>
  {/if}
</main>

<style>
  .presets {
    max-width: 48rem;
    margin: 2rem auto;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .header-row h1 {
    margin: 0;
  }
  .new-btn {
    background: var(--primary);
    color: var(--primary-foreground);
    border-color: var(--primary);
    font-weight: 700;
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
  .preset-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .preset-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    background: var(--card);
    color: var(--card-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.5rem 0.75rem;
    text-align: left;
  }
  .preset-row.selected {
    border-color: var(--primary);
    font-weight: 700;
  }
  .star {
    color: var(--primary);
    width: 1em;
  }
  .name {
    font-weight: 700;
  }
  .default-tag {
    font-size: 0.75em;
    color: var(--muted-foreground);
  }
  .expr {
    margin-inline-start: auto;
    font-family: var(--font-mono);
    font-size: 0.85em;
    color: var(--muted-foreground);
  }
  .empty {
    color: var(--muted-foreground);
  }
  .editor {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    background: var(--card);
    color: var(--card-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 1rem;
  }
  .editor label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85em;
    color: var(--muted-foreground);
  }
  .checkbox-field {
    flex-direction: row !important;
    align-items: center;
    gap: 0.4rem !important;
  }
  .save-error {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
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
  .error {
    color: var(--error-token);
    margin: 0;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
  .delete-btn {
    margin-inline-end: auto;
    color: var(--error-token);
  }
  .save-btn {
    background: var(--primary);
    color: var(--primary-foreground);
    border-color: var(--primary);
    font-weight: 700;
  }
</style>
