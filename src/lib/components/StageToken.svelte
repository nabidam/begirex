<script lang="ts">
  // StageToken (UX.md S2, DESIGN.md §4 gap #2) — icon + label-mono text chip,
  // one per stage. Never color alone (NFR-4/hard rule #4): every stage pairs
  // a distinct glyph with its literal stage name. Colors per DESIGN.md gap
  // #2: active states `primary`, paused/queued/cancelled `on-surface-variant`
  // (--muted-foreground), done `secondary`, error `--error-token`.
  // ponytail: unicode glyphs stand in for lucide-svelte icons, same
  // no-new-dependency precedent as Sidebar.svelte (T13).
  let { stage }: { stage: string } = $props();

  const ICON: Record<string, string> = {
    downloading: "↓",
    merging: "⇄",
    queued: "‖",
    paused: "⏸",
    completed: "✓",
    error: "⚠",
    cancelled: "✕",
  };

  const COLOR_VAR: Record<string, string> = {
    downloading: "var(--primary)",
    merging: "var(--primary)",
    queued: "var(--muted-foreground)",
    paused: "var(--muted-foreground)",
    completed: "var(--secondary)",
    error: "var(--error-token)",
    cancelled: "var(--muted-foreground)",
  };
</script>

<span class="stage-token" style:color={COLOR_VAR[stage] ?? "var(--muted-foreground)"}>
  <span class="glyph" aria-hidden="true">{ICON[stage] ?? "•"}</span>
  <span class="label">{stage}</span>
</span>

<style>
  .stage-token {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-family: var(--font-mono);
    font-size: 0.85em;
    white-space: nowrap;
  }
  .glyph {
    width: 1em;
    text-align: center;
  }
</style>
