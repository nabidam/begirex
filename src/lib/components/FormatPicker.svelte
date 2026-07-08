<script lang="ts">
  // S4 — Format Picker (UX.md, TASKS.md T10). Modal fallback over S3's Region
  // 2: the full probed-format table (virtualized via VirtualList), with
  // filter chips + text filter narrowing it, and row selection composing the
  // raw expression mirrored back into S3's field. "Use format" writes the
  // composed expression back, deselects S3's quick-pick group (Flow B step
  // 3), and closes.
  import type { Format } from "../types";
  import VirtualList from "./VirtualList.svelte";

  let {
    open = $bindable(false),
    formats,
    title,
    expression = $bindable(""),
    selectedQuickPickId = $bindable<string | null>(null),
  }: {
    open?: boolean;
    formats: Format[];
    title: string;
    expression?: string;
    selectedQuickPickId?: string | null;
  } = $props();

  const ROW_HEIGHT = 32;
  const TABLE_HEIGHT = 320;

  let filterText = $state("");
  let showVideoOnly = $state(false);
  let showAudioOnly = $state(false);
  let showFreeMerge = $state(false);

  let selectedVideoId = $state<string | null>(null);
  let selectedAudioId = $state<string | null>(null);

  function isAudioOnly(f: Format): boolean {
    return f.resolution === "audio only";
  }
  function isVideo(f: Format): boolean {
    return !!f.resolution && !isAudioOnly(f);
  }
  function isFreeMerge(f: Format): boolean {
    return isVideo(f) && f.has_audio;
  }

  function heightOf(f: Format): number {
    const wxh = f.resolution?.match(/^\d+x(\d+)$/);
    if (wxh) return Number(wxh[1]);
    const pOnly = f.resolution?.match(/^(\d+)p$/);
    return pOnly ? Number(pOnly[1]) : 0;
  }

  function sizeLabel(bytes: number | null): string {
    if (bytes == null) return "—";
    const mb = bytes / (1024 * 1024);
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb.toFixed(0)} MB`;
  }

  // "Best" row eye-first per UX.md: prefer the highest-resolution muxed
  // (free-merge) format so no pairing is needed; otherwise the highest
  // resolution video paired with the largest audio-only track.
  let bestVideoId = $derived.by((): string | null => {
    const videos = formats.filter(isVideo).sort((a, b) => heightOf(b) - heightOf(a));
    const muxed = videos.find(isFreeMerge);
    return (muxed ?? videos[0])?.id ?? null;
  });
  let bestAudioId = $derived.by((): string | null => {
    const bestVideo = formats.find((f) => f.id === bestVideoId);
    if (bestVideo?.has_audio) return null;
    const audios = formats.filter(isAudioOnly).sort((a, b) => (b.filesize ?? 0) - (a.filesize ?? 0));
    return audios[0]?.id ?? null;
  });

  $effect(() => {
    if (open) {
      selectedVideoId = bestVideoId;
      selectedAudioId = bestAudioId;
    }
  });

  let filtered = $derived.by((): Format[] => {
    const anyChipActive = showVideoOnly || showAudioOnly || showFreeMerge;
    const text = filterText.trim().toLowerCase();
    return formats.filter((f) => {
      if (anyChipActive) {
        const matchesChip =
          (showVideoOnly && isVideo(f)) ||
          (showAudioOnly && isAudioOnly(f)) ||
          (showFreeMerge && isFreeMerge(f));
        if (!matchesChip) return false;
      }
      if (text) {
        const haystack = [f.id, f.resolution, f.ext, f.codec, f.note].filter(Boolean).join(" ").toLowerCase();
        if (!haystack.includes(text)) return false;
      }
      return true;
    });
  });

  function toggleRow(f: Format) {
    if (isAudioOnly(f)) {
      selectedAudioId = selectedAudioId === f.id ? null : f.id;
    } else {
      selectedVideoId = selectedVideoId === f.id ? null : f.id;
      if (f.has_audio) selectedAudioId = null;
    }
  }

  let composedExpression = $derived.by((): string => {
    if (selectedVideoId && selectedAudioId) return `${selectedVideoId}+${selectedAudioId}`;
    if (selectedVideoId) return selectedVideoId;
    if (selectedAudioId) return selectedAudioId;
    return "";
  });

  $effect(() => {
    if (composedExpression) expression = composedExpression;
  });

  function onExpressionInput() {
    selectedVideoId = null;
    selectedAudioId = null;
  }

  function close() {
    open = false;
  }

  function useFormat() {
    selectedQuickPickId = null;
    close();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      close();
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scrim" onclick={close} onkeydown={handleKeydown}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="overlay"
      role="dialog"
      aria-modal="true"
      aria-label="Formats for {title}"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
    >
      <header>
        <h2>Formats for &ldquo;{title}&rdquo;</h2>
        <button type="button" class="icon-btn" onclick={close} aria-label="Close">✕</button>
      </header>

      <div class="filters">
        <input
          type="text"
          class="filter-text"
          bind:value={filterText}
          placeholder="⌕ filter"
          aria-label="Filter formats"
        />
        <label class="chip">
          <input type="checkbox" bind:checked={showVideoOnly} />
          video only
        </label>
        <label class="chip">
          <input type="checkbox" bind:checked={showAudioOnly} />
          audio only
        </label>
        <label class="chip">
          <input type="checkbox" bind:checked={showFreeMerge} />
          free-merge
        </label>
      </div>

      {#if formats.length === 0}
        <p class="empty">No formats returned — the site may require auth or the URL is not a media page.</p>
      {:else if filtered.length === 0}
        <p class="empty">No formats returned — the site may require auth or the URL is not a media page.</p>
      {:else}
        <div class="table-header" role="row">
          <span>ID</span>
          <span>RES</span>
          <span>EXT</span>
          <span>FPS</span>
          <span>SIZE</span>
          <span>CODEC</span>
          <span>NOTE</span>
        </div>
        <VirtualList items={filtered} itemHeight={ROW_HEIGHT} height={TABLE_HEIGHT}>
          {#snippet row(f: Format)}
            {@const selected = f.id === selectedVideoId || f.id === selectedAudioId}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              class="table-row"
              class:selected
              class:best={f.id === bestVideoId || f.id === bestAudioId}
              role="row"
              tabindex="0"
              onclick={() => toggleRow(f)}
              onkeydown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  toggleRow(f);
                }
              }}
            >
              <span class="mono">{f.id}</span>
              <span>{f.resolution ?? "—"}</span>
              <span>{f.ext}</span>
              <span>{f.fps ?? "—"}</span>
              <span class="mono">{sizeLabel(f.filesize)}</span>
              <span class="mono">{f.codec ?? "—"}</span>
              <span class="note">
                {f.note ?? ""}
                {#if f.id === bestVideoId || f.id === bestAudioId}<span class="best-badge">✓ pick</span>{/if}
              </span>
            </div>
          {/snippet}
        </VirtualList>
      {/if}

      <label class="expression-field">
        <span>Expression</span>
        <input
          type="text"
          class="mono"
          bind:value={expression}
          oninput={onExpressionInput}
          placeholder="137+140"
        />
      </label>

      <footer>
        <button type="button" onclick={close}>Cancel</button>
        <button type="button" disabled={!expression.trim()} onclick={useFormat}>Use format</button>
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
    z-index: 60;
  }
  .overlay {
    background: var(--card);
    color: var(--card-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 1.25rem;
    width: 42rem;
    max-width: calc(100vw - 2rem);
    max-height: calc(100vh - 4rem);
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  header h2 {
    margin: 0;
    font-size: 1.05em;
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
  .filters {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  .filter-text {
    flex: 1;
    min-width: 8rem;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85em;
    color: var(--muted-foreground);
    cursor: pointer;
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
  input.mono,
  .mono {
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
  .empty {
    color: var(--muted-foreground);
    text-align: center;
    padding: 1.5rem 0.5rem;
    font-size: 0.9em;
  }
  .table-header,
  .table-row {
    display: grid;
    grid-template-columns: 4rem 5rem 3.5rem 3rem 5rem 6rem 1fr;
    gap: 0.5rem;
    align-items: center;
    padding-inline: 0.4rem;
  }
  .table-header {
    color: var(--muted-foreground);
    font-size: 0.75em;
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }
  .table-row {
    font-size: 0.85em;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .table-row:hover {
    background: var(--accent);
  }
  .table-row:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }
  .table-row.selected {
    background: var(--secondary);
    color: var(--secondary-foreground);
  }
  .table-row.best .note {
    color: var(--primary);
  }
  .note {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .best-badge {
    font-family: var(--font-mono);
    font-size: 0.85em;
    color: var(--primary);
    flex-shrink: 0;
  }
  .expression-field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    color: var(--muted-foreground);
    font-size: 0.85em;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
