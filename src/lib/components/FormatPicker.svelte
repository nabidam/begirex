<script lang="ts">
  // S4 — Format Picker (UX.md, TASKS.md T10, migrated to shadcn/lucide at
  // T25). Modal fallback over S3's Region 2: the full probed-format table
  // (virtualized via VirtualList, untouched by T25), with filter chips +
  // text filter narrowing it, and row selection composing the raw
  // expression mirrored back into S3's field. "Use format" writes the
  // composed expression back, deselects S3's quick-pick group (Flow B step
  // 3), and closes.
  import type { Format } from "../types";
  import VirtualList from "./VirtualList.svelte";
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Toggle } from "$lib/components/ui/toggle";
  import { cn } from "$lib/utils";
  import Search from "lucide-svelte/icons/search";
  import Check from "lucide-svelte/icons/check";
  import X from "lucide-svelte/icons/x";

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

  function onOpenChange(next: boolean) {
    open = next;
  }

  // ponytail: same bits-ui-Escape-vs-hand-rolled-priority reconciliation as
  // AddDownload.svelte (see its handleKeydown comment), plus one more layer
  // here: S4 nests *inside* S3's Dialog.Content in the component tree, so a
  // document-level Escape listener (bits' default) would keep bubbling past
  // this dialog to S3's/S5's own handlers after closing S4 — the original
  // hand-rolled scrim avoided that by stopping propagation locally, before
  // it ever left this overlay. Reproduced here: `escapeKeydownBehavior`
  // stays "ignore" (bits never auto-closes) and this local `onkeydown`
  // (attached to Dialog.Content itself, firing during the bubble phase
  // *before* the event would reach `document`/`window`) is the sole
  // Escape-closes-S4 mechanism, stopped from propagating further — so a
  // single Escape only ever closes the topmost overlay, matching T15 AC4.
  function handleContentKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      close();
    }
  }
</script>

<Dialog.Root {open} {onOpenChange}>
  <Dialog.Content
    escapeKeydownBehavior="ignore"
    showCloseButton={false}
    onkeydown={handleContentKeydown}
    class="flex max-h-[calc(100vh-4rem)] w-full max-w-[calc(100vw-2rem)] flex-col gap-3 sm:max-w-[42rem]"
  >
    <div class="flex items-center justify-between">
      <Dialog.Title class="text-[1.05em]">Formats for &ldquo;{title}&rdquo;</Dialog.Title>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class="text-muted-foreground"
        onclick={close}
        aria-label="Close"
      >
        <X aria-hidden="true" />
      </Button>
    </div>

    <div class="flex flex-wrap items-center gap-3">
      <div class="relative min-w-32 flex-1">
        <Search aria-hidden="true" class="pointer-events-none absolute inset-y-0 start-2.5 my-auto size-3.5 text-muted-foreground" />
        <Input
          type="text"
          bind:value={filterText}
          placeholder="filter"
          aria-label="Filter formats"
          class="ps-8"
        />
      </div>
      <Toggle variant="outline" size="sm" bind:pressed={showVideoOnly}>video only</Toggle>
      <Toggle variant="outline" size="sm" bind:pressed={showAudioOnly}>audio only</Toggle>
      <Toggle variant="outline" size="sm" bind:pressed={showFreeMerge}>free-merge</Toggle>
    </div>

    {#if formats.length === 0}
      <p class="px-2 py-6 text-center text-[0.9em] text-muted-foreground">
        No formats returned — the site may require auth or the URL is not a media page.
      </p>
    {:else if filtered.length === 0}
      <p class="px-2 py-6 text-center text-[0.9em] text-muted-foreground">
        No formats returned — the site may require auth or the URL is not a media page.
      </p>
    {:else}
      <div
        class="grid grid-cols-[4rem_5rem_3.5rem_3rem_5rem_6rem_1fr] items-center gap-2 px-1.5 text-[0.75em] tracking-wide text-muted-foreground uppercase"
        role="row"
      >
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
          {@const best = f.id === bestVideoId || f.id === bestAudioId}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class={cn(
              "grid grid-cols-[4rem_5rem_3.5rem_3rem_5rem_6rem_1fr] items-center gap-2 rounded-lg px-1.5 text-[0.85em] hover:bg-accent focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring",
              selected && "bg-secondary text-secondary-foreground hover:bg-secondary",
            )}
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
            <span class="font-mono">{f.id}</span>
            <span>{f.resolution ?? "—"}</span>
            <span>{f.ext}</span>
            <span>{f.fps ?? "—"}</span>
            <span class="font-mono">{sizeLabel(f.filesize)}</span>
            <span class="font-mono">{f.codec ?? "—"}</span>
            <span class="flex items-center gap-1.5 overflow-hidden text-ellipsis whitespace-nowrap">
              {f.note ?? ""}
              {#if best}
                <span class={cn("flex shrink-0 items-center gap-0.5 font-mono", !selected && "text-primary")}>
                  <Check aria-hidden="true" class="size-3.5" /> pick
                </span>
              {/if}
            </span>
          </div>
        {/snippet}
      </VirtualList>
    {/if}

    <label class="flex flex-col gap-1 text-[0.85em] text-muted-foreground">
      <span>Expression</span>
      <Input
        type="text"
        class="font-mono"
        bind:value={expression}
        oninput={onExpressionInput}
        placeholder="137+140"
      />
    </label>

    <div class="flex justify-end gap-2">
      <Button type="button" variant="outline" onclick={close}>Cancel</Button>
      <Button type="button" disabled={!expression.trim()} onclick={useFormat}>Use format</Button>
    </div>
  </Dialog.Content>
</Dialog.Root>
