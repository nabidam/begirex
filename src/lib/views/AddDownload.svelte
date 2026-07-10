<script lang="ts">
  // S3 — Add Download overlay (UX.md, TASKS.md T9, migrated to shadcn/lucide
  // at T25). Progressive disclosure: URL input is always visible; the format
  // region unfolds after a probe attempt (success or failure); Advanced
  // (output template/proxy/extra args) stays collapsed by default. Mounted
  // once by Queue.svelte, opened via its "+ Add" button or Ctrl/Cmd+N,
  // closed via Esc or Cancel/Add.
  import { onMount } from "svelte";
  import { queueStore } from "../stores/queue.svelte";
  import { presetsStore } from "../stores/presets.svelte";
  import { probeFormats } from "../ipc";
  import type { AppError, ProbeFormatsResponse } from "../types";
  import FormatQuickPicks from "../components/FormatQuickPicks.svelte";
  import FormatPicker from "../components/FormatPicker.svelte";
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as Select from "$lib/components/ui/select";
  import * as Collapsible from "$lib/components/ui/collapsible";
  import { cn } from "$lib/utils";
  import X from "lucide-svelte/icons/x";
  import ChevronRight from "lucide-svelte/icons/chevron-right";

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

  let selectedPresetLabel = $derived.by((): string => {
    const preset = presetsStore.presets.find((p) => p.id === selectedPresetId);
    if (!preset) return "Select a preset";
    return `${preset.name}${preset.is_default ? " (default)" : ""}`;
  });

  // bits-ui's Dialog.Content ships its own focus-trap auto-focus (defaults
  // to the content panel itself) that would otherwise race the URL input's
  // focus below. `onOpenAutoFocus` (wired on Dialog.Content) intercepts it
  // and focuses urlInputEl deterministically instead, matching the original
  // `$effect(() => { if (open) urlInputEl?.focus(); })` timing exactly (runs
  // once per open transition, nothing else changes it).
  function onOpenAutoFocus(e: Event) {
    e.preventDefault();
    urlInputEl?.focus();
  }

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

  // ponytail: DetailDrawer.svelte (S5, T15) enforces "S5-before-S3" Esc
  // priority via a `window`-level keydown listener + stopImmediatePropagation
  // (mounted ahead of this component in Shell.svelte). bits-ui's Dialog
  // registers its own Escape handling on `document` — which the DOM bubble
  // phase always reaches *before* `window` — and doesn't coordinate with our
  // hand-rolled priority scheme. Left at its default "close" behavior, it
  // would auto-close this dialog on every Escape independent of whether S5
  // is topmost, double-closing S3+S5 together instead of S5 alone. So the
  // Dialog.Content below sets `escapeKeydownBehavior="ignore"` (disables
  // bits' own auto-close; it still no-ops via `preventDefault` only, it does
  // not `stopPropagation`) and Escape continues to be owned exclusively by
  // the existing window-level `handleKeydown` above, unchanged — preserving
  // T15 AC4 byte-for-byte. Backdrop-click-to-close and the header Close
  // button still go through bits' normal `onOpenChange`, which mirrors the
  // old scrim `onclick`/icon-button `close()` exactly (see below).
  function onOpenChange(next: boolean) {
    open = next;
    if (!next) reset();
  }
</script>

<Dialog.Root {open} {onOpenChange}>
  <Dialog.Content
    escapeKeydownBehavior="ignore"
    showCloseButton={false}
    {onOpenAutoFocus}
    class="flex max-h-[calc(100vh-4rem)] w-full max-w-[calc(100vw-2rem)] flex-col gap-3.5 overflow-y-auto sm:max-w-[32rem]"
  >
    <div class="flex items-center justify-between">
      <Dialog.Title>Add Download</Dialog.Title>
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

    <label class="flex flex-col gap-1.5">
      <span class="text-[0.85em] text-muted-foreground">URL(s)</span>
      <div class="flex gap-2">
        <Input
          type="text"
          bind:value={url}
          bind:ref={urlInputEl}
          placeholder="https://…"
          class="flex-1"
        />
        <Button
          type="button"
          disabled={!url.trim() || probeState === "loading"}
          onclick={probe}
        >
          {probeState === "loading" ? "Probing…" : "Probe"}
          {#if probeState !== "loading"}<ChevronRight aria-hidden="true" class="size-3.5" />{/if}
        </Button>
      </div>
      <p class="m-0 text-[0.8em] text-muted-foreground">Paste a playlist and it expands to N items on Add.</p>
    </label>

    <label class="flex flex-col gap-1.5">
      <span class="text-[0.85em] text-muted-foreground">Preset</span>
      <Select.Root type="single" value={String(selectedPresetId ?? "")} onValueChange={(v) => v && applyPreset(Number(v))}>
        <Select.Trigger class="w-full">
          {selectedPresetLabel}
        </Select.Trigger>
        <Select.Content>
          {#each presetsStore.presets as preset (preset.id)}
            <Select.Item value={String(preset.id)} label={`${preset.name}${preset.is_default ? " (default)" : ""}`}>
              {preset.name}{preset.is_default ? " (default)" : ""}
            </Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
    </label>

    {#if probeState !== "idle"}
      <div class="flex flex-col gap-2">
        <div class="flex items-center justify-between">
          <span class="text-[0.85em] text-muted-foreground">Format</span>
          {#if probeState === "success" && probeResult}
            <Button
              type="button"
              variant="link"
              class="h-auto p-0 text-[0.85em]"
              onclick={() => (formatPickerOpen = true)}
            >
              Format Picker
            </Button>
          {/if}
        </div>
        {#if probeState === "loading"}
          <div class="flex flex-col gap-2" aria-hidden="true">
            <div class="h-[1.6rem] animate-pulse rounded-lg bg-accent"></div>
            <div class="h-[1.6rem] animate-pulse rounded-lg bg-accent"></div>
          </div>
        {:else if probeState === "error"}
          <div class="flex flex-col gap-2">
            <pre class="m-0 max-h-32 overflow-y-auto rounded-lg border border-border bg-muted p-2.5 font-mono text-[0.85em] whitespace-pre-wrap text-[var(--error-token)]">{probeError?.stderr || probeError?.message}</pre>
            <Button type="button" variant="outline" size="sm" class="w-fit" onclick={probe}>Retry</Button>
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

    <Collapsible.Root bind:open={advancedOpen} class="flex flex-col gap-2">
      <Collapsible.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            type="button"
            variant="ghost"
            size="sm"
            class="w-fit gap-1 self-start px-0 text-muted-foreground hover:bg-transparent"
            aria-expanded={advancedOpen}
          >
            <ChevronRight aria-hidden="true" class={cn("size-3.5 transition-transform", advancedOpen && "rotate-90")} />
            Advanced
          </Button>
        {/snippet}
      </Collapsible.Trigger>
      <Collapsible.Content class="flex flex-col gap-2">
        <label class="flex flex-col gap-1.5">
          <span class="text-[0.85em] text-muted-foreground">Output template</span>
          <Input type="text" class="font-mono" bind:value={outputTemplate} placeholder="%(title)s.%(ext)s" />
        </label>
        <label class="flex flex-col gap-1.5">
          <span class="text-[0.85em] text-muted-foreground">Proxy override</span>
          <Input type="text" bind:value={proxyOverride} placeholder="http://…" />
        </label>
        <label class="flex flex-col gap-1.5">
          <span class="text-[0.85em] text-muted-foreground">Extra CLI args</span>
          <Input type="text" class="font-mono" bind:value={extraArgs} placeholder="--no-mtime" />
        </label>
      </Collapsible.Content>
    </Collapsible.Root>

    <div class="flex justify-end gap-2">
      <Button type="button" variant="outline" onclick={close}>Cancel</Button>
      <Button type="button" disabled={!url.trim() || adding} onclick={addClick}>Add to queue</Button>
    </div>
  </Dialog.Content>
</Dialog.Root>
