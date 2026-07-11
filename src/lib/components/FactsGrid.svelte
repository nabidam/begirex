<script lang="ts">
  // FactsGrid (UX.md S5 region 2, migrated to shadcn/lucide at T26) — the
  // "why/where/how" of one item: address (copyable), resolved output path
  // (open-dir), status, size/downloaded, speed/ETA, resume capability
  // (SPEC-critical), effective proxy (K5-AC1/AC2), and format+preset.
  // `label-mono` for sizes/format/flags. Hosted on a shadcn `Card` per T26
  // AC1.
  import StageToken from "./StageToken.svelte";
  import type { Item } from "../types";
  import * as Card from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { cn } from "$lib/utils";
  import Copy from "lucide-svelte/icons/copy";
  import Check from "lucide-svelte/icons/check";
  import FolderOpen from "lucide-svelte/icons/folder-open";

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
  let copyFailed = $state(false);
  let folderError = $state(false);

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
    try {
      await navigator.clipboard.writeText(item.url);
      copied = true;
      copyFailed = false;
      setTimeout(() => (copied = false), 1500);
    } catch {
      copyFailed = true;
      setTimeout(() => (copyFailed = false), 2500);
    }
  }

  async function openFolder() {
    try {
      await onOpenFolder(item.output_dir);
      folderError = false;
    } catch {
      folderError = true;
      setTimeout(() => (folderError = false), 2500);
    }
  }

  const savingTo = $derived(item.output_path ?? `${item.output_dir}/${item.output_template}`);
  const effectiveProxy = $derived(item.proxy ?? globalProxy);
  const proxyIsOverride = $derived(item.proxy != null);
</script>

<Card.Root size="sm" class="gap-0">
  <Card.Content>
    <dl class="m-0 flex flex-col gap-2">
      <div class="grid grid-cols-[var(--facts-label-inline-size)_minmax(0,1fr)] items-baseline gap-2 text-xs">
        <dt class="text-muted-foreground">Address</dt>
        <dd class="m-0 flex min-w-0 items-center gap-2">
          <span class="min-w-0 flex-1 truncate font-mono" title={item.url}>{item.url}</span>
          <Button type="button" variant="link" size="sm" class="h-auto shrink-0 gap-1 p-0" onclick={copyAddress}>
            {#if copyFailed}
              <Copy aria-hidden="true" class="size-3.5" />
              Copy failed
            {:else if copied}
              <Check aria-hidden="true" class="size-3.5" />
              Copied
            {:else}
              <Copy aria-hidden="true" class="size-3.5" />
              Copy
            {/if}
          </Button>
        </dd>
      </div>

      <div class="grid grid-cols-[var(--facts-label-inline-size)_minmax(0,1fr)] items-baseline gap-2 text-xs">
        <dt class="text-muted-foreground">Saving to</dt>
        <dd class="m-0 flex min-w-0 items-center gap-2">
          <span class="min-w-0 flex-1 truncate font-mono" title={savingTo}>{savingTo}</span>
          <Button
            type="button"
            variant="link"
            size="sm"
            class="h-auto shrink-0 gap-1 p-0"
            onclick={openFolder}
          >
            <FolderOpen aria-hidden="true" class="size-3.5" />
            {folderError ? "Open failed" : "Open dir"}
          </Button>
        </dd>
      </div>

      <div class="grid grid-cols-[var(--facts-label-inline-size)_minmax(0,1fr)] items-baseline gap-2 text-xs">
        <dt class="text-muted-foreground">Status</dt>
        <dd class="m-0"><StageToken stage={item.stage} /></dd>
      </div>

      <div class="grid grid-cols-[var(--facts-label-inline-size)_minmax(0,1fr)_var(--facts-compact-label-inline-size)_minmax(0,1fr)] items-baseline gap-2 text-xs">
        <dt class="text-muted-foreground">Size</dt>
        <dd class="m-0 min-w-0 truncate font-mono" title={formatBytes(item.total_bytes)}>{formatBytes(item.total_bytes)}</dd>
        <dt class="text-muted-foreground">Downloaded</dt>
        <dd class="m-0 min-w-0 truncate font-mono" title={formatBytes(item.downloaded_bytes)}>{formatBytes(item.downloaded_bytes)}</dd>
      </div>

      <div class="grid grid-cols-[var(--facts-label-inline-size)_minmax(0,1fr)_var(--facts-compact-label-inline-size)_minmax(0,1fr)] items-baseline gap-2 text-xs">
        <dt class="text-muted-foreground">Speed</dt>
        <dd class="m-0 min-w-0 truncate font-mono" title={formatSpeed(item.speed_bps)}>{formatSpeed(item.speed_bps)}</dd>
        <dt class="text-muted-foreground">ETA</dt>
        <dd class="m-0 min-w-0 truncate font-mono" title={formatEta(item.eta_seconds)}>{formatEta(item.eta_seconds)}</dd>
      </div>

      <div class="grid grid-cols-[var(--facts-label-inline-size)_minmax(0,1fr)] items-baseline gap-2 text-xs">
        <dt class="text-muted-foreground">Resume</dt>
        <dd class="m-0">{item.resume_capable ? "Yes (partial on disk)" : "No"}</dd>
      </div>

      <div class="grid grid-cols-[var(--facts-label-inline-size)_minmax(0,1fr)] items-baseline gap-2 text-xs">
        <dt class="text-muted-foreground">Proxy</dt>
        <dd class="m-0 font-mono">
          {#if effectiveProxy}
            {effectiveProxy}
            <span class="font-sans text-muted-foreground">({proxyIsOverride ? "override" : "global"})</span>
          {:else}
            <span class="font-sans text-muted-foreground">Not configured</span>
          {/if}
        </dd>
      </div>

      <div class="grid grid-cols-[var(--facts-label-inline-size)_minmax(0,1fr)] items-baseline gap-2 text-xs">
        <dt class="text-muted-foreground">Format</dt>
        <dd class="m-0 min-w-0 break-words font-mono">{item.format_expr} · Preset: {presetName ?? "None"}</dd>
      </div>
    </dl>
  </Card.Content>
</Card.Root>

{#if item.stage === "error" && item.error_message}
  <pre
    class={cn(
      "m-0 max-h-24 overflow-y-auto rounded-lg border border-border bg-muted p-2.5 font-mono text-xs whitespace-pre-wrap",
      "text-[var(--error-token)]",
    )}
  >{item.error_message}</pre>
{/if}
