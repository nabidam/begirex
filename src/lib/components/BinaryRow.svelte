<script lang="ts">
  // BinaryRow (UX.md S1 Region 1 + S7 Region 1, TASKS.md T17, migrated to
  // shadcn/lucide at T28) — one row per binary with a live status token,
  // shared between the full onboarding wizard and Settings' "Engine &
  // health" section. `onDownload` is only passed by S1 (S7 never
  // re-triggers an in-app fetch — its own missing-binary case is handled by
  // GlobalBanner's Fix → reopens S1 per UX.md S7 states); when absent this
  // renders the compact S7 form (path/version + Change…) instead of S1's
  // found/not-found + Download-for-me/Set-path choice. Built on shadcn
  // `card` (T28 AC1) — same "list row styled like a card" precedent as
  // FactsGrid.svelte (T26).
  import type { BinaryStatus } from "../types";
  import * as Card from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { Progress } from "$lib/components/ui/progress";
  import Loader2 from "lucide-svelte/icons/loader-circle";
  import CircleCheck from "lucide-svelte/icons/circle-check";
  import Download from "lucide-svelte/icons/download";
  import TriangleAlert from "lucide-svelte/icons/triangle-alert";

  let {
    label,
    status,
    onSetPath,
    onDownload,
    downloadState,
  }: {
    label: string;
    status: BinaryStatus | null | undefined;
    onSetPath: () => void;
    onDownload?: () => void;
    downloadState?: { active: boolean; percent: number; error: string | null; stderr: string | null };
  } = $props();

  let showStderr = $state(false);
</script>

<Card.Root size="sm" class="gap-2">
  <Card.Content class="flex flex-col gap-2">
    <div class="flex flex-wrap items-center gap-2.5">
      <span class="w-[4.5rem] font-bold">{label}</span>
      {#if !status}
        <span class="inline-flex items-center gap-1.5 font-mono text-[0.85em] text-muted-foreground" role="status">
          <Loader2 aria-hidden="true" class="size-3.5 animate-spin" />
          Checking…
        </span>
      {:else if status.found}
        <span class="inline-flex items-center gap-1.5 font-mono text-[0.85em] text-primary">
          <CircleCheck aria-hidden="true" class="size-3.5" />
          found — {status.path}{status.version ? ` (${status.version})` : ""}
        </span>
        <Button type="button" variant="ghost" size="sm" onclick={onSetPath}>Change…</Button>
      {:else if downloadState?.active}
        <span class="inline-flex items-center gap-1.5 font-mono text-[0.85em] text-primary">
          <Download aria-hidden="true" class="size-3.5" />
          downloading…
        </span>
      {:else}
        <span class="inline-flex items-center gap-1.5 font-mono text-[0.85em] text-[var(--warning)]">
          <TriangleAlert aria-hidden="true" class="size-3.5" />
          not found
        </span>
      {/if}
    </div>

    {#if status && !status.found}
      {#if downloadState?.active}
        <div class="ms-[5.1rem] flex items-center gap-2">
          <Progress value={Math.min(100, Math.max(0, downloadState.percent))} class="h-2 flex-1" />
          <span class="shrink-0 font-mono text-[0.78em] text-muted-foreground">{downloadState.percent.toFixed(0)}%</span>
        </div>
      {:else}
        <div class="ms-[5.1rem] flex gap-2">
          {#if onDownload}
            <Button type="button" size="sm" onclick={onDownload}>Download for me</Button>
          {/if}
          <Button type="button" variant="outline" size="sm" onclick={onSetPath}>Set path…</Button>
        </div>
      {/if}

      {#if downloadState?.error}
        <div class="ms-[5.1rem] flex flex-col gap-1">
          <p class="m-0 text-[0.85em] text-[var(--error-token)]">
            Couldn't download {label}: {downloadState.error}.
            <Button
              type="button"
              variant="link"
              class="h-auto p-0 text-[0.85em] text-[var(--error-token)]"
              onclick={onDownload}
            >
              Retry
            </Button>, or set a path.
          </p>
          {#if downloadState.stderr}
            <Button
              type="button"
              variant="link"
              class="h-auto w-fit p-0 text-[0.85em]"
              onclick={() => (showStderr = !showStderr)}
            >
              {showStderr ? "Hide" : "Show"} details
            </Button>
            {#if showStderr}
              <pre
                class="m-0 max-h-24 overflow-y-auto rounded-lg border border-border bg-muted p-2 font-mono text-[0.8em] whitespace-pre-wrap text-[var(--error-token)]"
              >{downloadState.stderr}</pre>
            {/if}
          {/if}
        </div>
      {/if}
    {/if}
  </Card.Content>
</Card.Root>
