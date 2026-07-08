<script lang="ts">
  // FactsGrid (UX.md S5 region 2) — the "why/where/how" of one item: address
  // (copyable), resolved output path (open-dir), status, size/downloaded,
  // speed/ETA, resume capability (SPEC-critical), effective proxy
  // (K5-AC1/AC2), and format+preset. `label-mono` for sizes/format/flags.
  import StageToken from "./StageToken.svelte";
  import type { Item } from "../types";

  let {
    item,
    globalProxy,
    presetName,
    onOpenFolder,
  }: {
    item: Item;
    globalProxy: string | null;
    presetName: string | null;
    onOpenFolder: (dir: string) => void;
  } = $props();

  let copied = $state(false);

  // ponytail: duplicated from QueueRow.svelte rather than extracted to a
  // shared util — QueueRow.svelte isn't in this task's touched-file list
  // (TASKS.md T15), and these are three one-line formatters. Upgrade path:
  // factor into a shared helper if a third caller needs them.
  function formatBytes(bytes: number | null): string {
    if (bytes == null) return "—";
    const mb = bytes / (1024 * 1024);
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb.toFixed(0)} MB`;
  }

  function formatSpeed(bps: number | null): string {
    if (bps == null) return "—";
    return `${(bps / (1024 * 1024)).toFixed(2)} MB/s`;
  }

  function formatEta(seconds: number | null): string {
    if (seconds == null) return "—";
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  async function copyAddress() {
    await navigator.clipboard.writeText(item.url);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  const savingTo = $derived(item.output_path ?? `${item.output_dir}/${item.output_template}`);
  const effectiveProxy = $derived(item.proxy ?? globalProxy);
  const proxyIsOverride = $derived(item.proxy != null);
</script>

<dl class="facts">
  <div class="row">
    <dt>Address</dt>
    <dd>
      <span class="value mono truncate" title={item.url}>{item.url}</span>
      <button type="button" class="link-btn" onclick={copyAddress}>{copied ? "Copied" : "Copy"}</button>
    </dd>
  </div>

  <div class="row">
    <dt>Saving to</dt>
    <dd>
      <span class="value mono truncate" title={savingTo}>{savingTo}</span>
      <button type="button" class="link-btn" onclick={() => onOpenFolder(item.output_dir)}>Open dir</button>
    </dd>
  </div>

  <div class="row">
    <dt>Status</dt>
    <dd><StageToken stage={item.stage} /></dd>
  </div>

  <div class="row">
    <dt>Size</dt>
    <dd class="mono">{formatBytes(item.total_bytes)}</dd>
    <dt>Downloaded</dt>
    <dd class="mono">{formatBytes(item.downloaded_bytes)}</dd>
  </div>

  <div class="row">
    <dt>Speed</dt>
    <dd class="mono">{formatSpeed(item.speed_bps)}</dd>
    <dt>ETA</dt>
    <dd class="mono">{formatEta(item.eta_seconds)}</dd>
  </div>

  <div class="row">
    <dt>Resume</dt>
    <dd>{item.resume_capable ? "Yes (partial on disk)" : "No"}</dd>
  </div>

  <div class="row">
    <dt>Proxy</dt>
    <dd class="mono">{effectiveProxy ?? "—"} <span class="hint">({proxyIsOverride ? "override" : "global"})</span></dd>
  </div>

  <div class="row">
    <dt>Format</dt>
    <dd class="mono">{item.format_expr} · Preset: {presetName ?? "None"}</dd>
  </div>
</dl>

{#if item.stage === "error" && item.error_message}
  <pre class="stderr">{item.error_message}</pre>
{/if}

<style>
  .facts {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin: 0;
  }
  .row {
    display: grid;
    grid-template-columns: 6rem 1fr;
    align-items: baseline;
    gap: 0.5rem;
    font-size: 0.85em;
  }
  .row:has(dt:nth-of-type(2)) {
    grid-template-columns: 6rem 1fr 4rem 1fr;
  }
  dt {
    color: var(--muted-foreground);
  }
  dd {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }
  .value {
    min-width: 0;
  }
  .truncate {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .hint {
    color: var(--muted-foreground);
    font-family: var(--font-sans);
  }
  .link-btn {
    flex-shrink: 0;
    background: none;
    border: none;
    color: var(--primary);
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
    font-size: 0.9em;
  }
  .link-btn:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .stderr {
    margin: 0;
    color: var(--error-token);
    background: var(--muted);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.6rem;
    font-family: var(--font-mono);
    font-size: 0.8em;
    white-space: pre-wrap;
    max-height: 6rem;
    overflow-y: auto;
  }
</style>
