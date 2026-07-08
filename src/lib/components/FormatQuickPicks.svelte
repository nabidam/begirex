<script lang="ts">
  // S3 region 2 (UX.md): named quick picks derived from a probe result, plus
  // the always-visible raw expression field — the two are one selectable
  // group (picking a quick pick fills the expression; editing the expression
  // deselects it). The full format table (all resolutions/codecs, sortable,
  // virtualized) is S4 (T10) — this only surfaces a handful of common picks.
  import type { Format } from "../types";

  let {
    formats,
    expression = $bindable(""),
    selectedQuickPickId = $bindable(null),
  }: {
    formats: Format[];
    expression: string;
    selectedQuickPickId: string | null;
  } = $props();

  type QuickPick = { id: string; label: string; expression: string };

  // yt-dlp's own `resolution` field is `WIDTHxHEIGHT` (confirmed against a
  // real probe — see engine_supervisor.rs's `map_format`), with a bare
  // `HEIGHTp` fallback for the rare format missing width.
  function heightOf(f: Format): number | null {
    const wxh = f.resolution?.match(/^\d+x(\d+)$/);
    if (wxh) return Number(wxh[1]);
    const pOnly = f.resolution?.match(/^(\d+)p$/);
    return pOnly ? Number(pOnly[1]) : null;
  }

  function sizeLabel(bytes: number | null): string {
    if (bytes == null) return "";
    const mb = bytes / (1024 * 1024);
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb.toFixed(0)} MB`;
  }

  // Top 2 distinct video resolutions (by height, descending) + one
  // audio-only pick if present — matches the UX.md S3 mock's "1080p / 720p /
  // audio…" trio without needing the full S4 table.
  let quickPicks = $derived.by((): QuickPick[] => {
    const picks: QuickPick[] = [];

    const heights = [...new Set(formats.map(heightOf).filter((h): h is number => h != null))]
      .sort((a, b) => b - a)
      .slice(0, 2);
    for (const h of heights) {
      const match = formats.find((f) => heightOf(f) === h);
      const parts = [`${h}p`, match?.ext, sizeLabel(match?.filesize ?? null)].filter(Boolean);
      picks.push({
        id: `height-${h}`,
        label: parts.join(" · "),
        expression: `bv*[height<=${h}]+ba/b`,
      });
    }

    const audioOnly = formats.find((f) => f.resolution === "audio only");
    if (audioOnly) {
      const parts = ["audio", sizeLabel(audioOnly.filesize)].filter(Boolean);
      picks.push({ id: "audio", label: parts.join(" · "), expression: "ba/b" });
    }

    return picks;
  });

  function pick(qp: QuickPick) {
    expression = qp.expression;
    selectedQuickPickId = qp.id;
  }

  function onExpressionInput() {
    selectedQuickPickId = null;
  }
</script>

<div class="format-region">
  {#if quickPicks.length > 0}
    <div class="quick-picks" role="radiogroup" aria-label="Quick format picks">
      {#each quickPicks as qp (qp.id)}
        <button
          type="button"
          class="quick-pick"
          class:selected={selectedQuickPickId === qp.id}
          role="radio"
          aria-checked={selectedQuickPickId === qp.id}
          onclick={() => pick(qp)}
        >
          {qp.label}
        </button>
      {/each}
    </div>
  {/if}
  <label class="expression-field">
    <span>Expression</span>
    <input
      type="text"
      class="expression-input"
      bind:value={expression}
      oninput={onExpressionInput}
      placeholder="bv*+ba/b"
    />
  </label>
</div>

<style>
  .format-region {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .quick-picks {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .quick-pick {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.3rem 0.6rem;
    font-family: var(--font-sans);
    font-size: 0.85em;
    cursor: pointer;
  }
  .quick-pick:hover {
    background: var(--accent);
  }
  .quick-pick:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .quick-pick.selected {
    background: var(--secondary);
    color: var(--secondary-foreground);
    border-color: var(--ring);
    font-weight: 700;
  }
  .expression-field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    color: var(--muted-foreground);
    font-size: 0.85em;
  }
  .expression-input {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4rem 0.6rem;
    font-family: var(--font-mono);
  }
  .expression-input:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>
