<script lang="ts">
  // S5 — Download Detail drawer (UX.md, TASKS.md T15, migrated to
  // shadcn/lucide at T26). Docked beside the live queue (never a
  // scrim/modal — ARCHITECTURE/UX §2 "S3 and S5 are overlays over S2...
  // sits beside S5"), opened via queueStore.openDetail (T14's row
  // click/Enter) and closed via close button or Esc. Mounted once,
  // unconditionally, from Shell.svelte so its Esc listener registers before
  // AddDownload's (S3) — UX §2 "Esc closes the topmost overlay (S5 → S3)".
  //
  // ponytail: AC1 asks for the outer shell to be shadcn `Sheet`, but this
  // stays the hand-rolled fixed `<aside>` (only the *inner* primitives —
  // FactsGrid's Card, LogDisclosure's Collapsible, Button, Progress,
  // lucide icons — are swapped), per the task's own documented fallback
  // ("non-scrim wins over using the Sheet shell"). Reason, traced against
  // bits-ui 2.18.1 (node_modules/bits-ui/dist/bits/dialog/*,
  // .../utilities/focus-scope/focus-scope.svelte.js): shadcn's generated
  // `ui/sheet/sheet-content.svelte` unconditionally renders
  // `<SheetOverlay />` inside itself with no prop to omit/style it, so the
  // *official* `Sheet.Content` wrapper cannot go non-scrim without either
  // hand-editing the generated ui/ file (against CONVENTIONS' "don't
  // hand-edit beyond theming") or re-composing the content shell directly
  // from raw bits-ui `Dialog` primitives outside the generated wrapper —
  // and even then, bits-ui's `FocusScope` always steals focus into the
  // panel on open (`#handleOpenAutoFocus` fires unconditionally, independent
  // of `trapFocus`), which the old drawer never did, so replicating exact
  // prior behavior would require suppressing that too. Given T15's AC4
  // (Esc priority) and the non-scrim/interactive-queue-behind requirement
  // are both load-bearing, the lower-risk path is keeping the proven
  // hand-rolled shell and only migrating what the task can safely swap.
  // Upgrade path: once `ui/sheet` grows an overlay-opt-out prop upstream,
  // swap the `<aside>` for real `Sheet.Root`/`Sheet.Content` with
  // `interactOutsideBehavior="ignore"`, `trapFocus={false}`,
  // `preventScroll={false}`, and a no-op `onOpenAutoFocus`.
  import { onMount } from "svelte";
  import { queueStore } from "../stores/queue.svelte";
  import { presetsStore } from "../stores/presets.svelte";
  import { settingsStore } from "../stores/settings.svelte";
  import { openPath } from "../ipc";
  import StageToken from "../components/StageToken.svelte";
  import FactsGrid from "../components/FactsGrid.svelte";
  import LogDisclosure from "../components/LogDisclosure.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Progress } from "$lib/components/ui/progress";
  import { cn } from "$lib/utils";
  import X from "lucide-svelte/icons/x";
  import Pause from "lucide-svelte/icons/pause";
  import Play from "lucide-svelte/icons/play";
  import RotateCcw from "lucide-svelte/icons/rotate-ccw";
  import Ban from "lucide-svelte/icons/ban";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import FileText from "lucide-svelte/icons/file-text";
  import FolderOpen from "lucide-svelte/icons/folder-open";

  const ACTIVE_STAGES = new Set(["downloading", "merging"]);
  const TERMINAL_STAGES = new Set(["completed", "cancelled"]);
  const TOAST_WINDOW_MS = 5000;

  const item = $derived(
    queueStore.activeDetailId != null
      ? queueStore.items.find((i) => i.id === queueStore.activeDetailId) ?? null
      : null,
  );
  const presetName = $derived(
    item?.preset_id != null
      ? presetsStore.presets.find((p) => p.id === item.preset_id)?.name ?? null
      : null,
  );

  let toast = $state<{ message: string; undo: () => void } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  let removeTimer: ReturnType<typeof setTimeout> | undefined;

  // Item removed (e.g. a row-level Remove elsewhere) while its drawer is
  // open — nothing left to show, so close.
  $effect(() => {
    if (queueStore.activeDetailId != null && item === null) {
      queueStore.closeDetail();
    }
  });

  function close() {
    queueStore.closeDetail();
  }

  function showUndoToast(message: string, undo: () => void) {
    clearTimeout(toastTimer);
    toast = {
      message,
      undo: () => {
        undo();
        toast = null;
      },
    };
    toastTimer = setTimeout(() => {
      toast = null;
    }, TOAST_WINDOW_MS);
  }

  // Cancel is real immediately (the process must actually stop); Undo
  // restores via retry_item (stage -> queued) — same ponytail precedent as
  // Queue.svelte's row/bulk Cancel, since retry_item is the only backend
  // verb that reverses a cancel.
  async function requestCancel(id: number) {
    if (!confirm("Cancel this download?")) return;
    await queueStore.cancel(id);
    showUndoToast("Cancelled.", () => queueStore.retry(id));
  }

  // Remove defers the real bulk(remove) call for the toast window, same as
  // Queue.svelte — Undo just clears the timer, nothing ever reached the
  // backend. The row itself stays visible in the queue list during the
  // window (this component has no access to Queue.svelte's local
  // hiddenIds); the drawer closing is this action's immediate feedback.
  function requestRemove(id: number) {
    if (!confirm("Remove this download?")) return;
    close();
    removeTimer = setTimeout(() => {
      queueStore.bulk("remove", [id]);
    }, TOAST_WINDOW_MS);
    showUndoToast("Removed.", () => clearTimeout(removeTimer));
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && queueStore.activeDetailId != null) {
      e.preventDefault();
      e.stopImmediatePropagation();
      close();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });
</script>

{#if item}
  <aside
    class="fixed inset-y-0 end-0 z-40 flex w-96 max-w-[calc(100vw-4rem)] flex-col border-s border-border bg-[var(--surface-lowest)] shadow-[-4px_0_12px_color-mix(in_srgb,var(--surface-lowest)_60%,transparent)]"
    aria-label="Download detail"
  >
    <header class="flex flex-col gap-[0.6rem] border-b border-border px-[1.1rem] pt-4 pb-3">
      <div class="flex items-center justify-between gap-2">
        <h2 class="m-0 min-w-0 truncate text-[1em]" title={item.title ?? item.url}>{item.title ?? item.url}</h2>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          class="shrink-0 text-muted-foreground"
          onclick={close}
          aria-label="Close"
        >
          <X aria-hidden="true" />
        </Button>
      </div>
      <div class="flex items-center gap-2">
        <StageToken stage={item.stage} />
        <Progress
          value={Math.min(100, Math.max(0, item.percent))}
          class={cn("flex-1", ACTIVE_STAGES.has(item.stage) ? "h-2" : "h-1")}
        />
        <span class="shrink-0 font-mono text-[0.78em] text-muted-foreground">{item.percent.toFixed(0)}%</span>
      </div>
    </header>

    <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-[1.1rem] py-4">
      <FactsGrid
        {item}
        globalProxy={settingsStore.settings?.global_proxy ?? null}
        {presetName}
        onOpenFolder={(dir) => openPath(dir)}
      />

      <LogDisclosure itemId={item.id} stage={item.stage} />
    </div>

    <footer class="flex flex-wrap justify-end gap-2 border-t border-border px-[1.1rem] py-3">
      {#if item.stage === "completed"}
        <Button type="button" variant="outline" onclick={() => item.output_path && openPath(item.output_path)}>
          <FileText aria-hidden="true" />
          Open file
        </Button>
        <Button type="button" variant="outline" onclick={() => item.output_path && openPath(item.output_path, true)}>
          <FolderOpen aria-hidden="true" />
          Open folder
        </Button>
      {:else}
        {#if ACTIVE_STAGES.has(item.stage)}
          <Button type="button" variant="outline" onclick={() => queueStore.pause(item.id)}>
            <Pause aria-hidden="true" />
            Pause
          </Button>
        {:else if item.stage === "paused"}
          <Button type="button" variant="outline" onclick={() => queueStore.resume(item.id)}>
            <Play aria-hidden="true" />
            Resume
          </Button>
        {/if}
        {#if item.stage === "error" || item.stage === "cancelled"}
          <Button type="button" variant="default" onclick={() => queueStore.retry(item.id)}>
            <RotateCcw aria-hidden="true" />
            Retry
          </Button>
        {/if}
        {#if !TERMINAL_STAGES.has(item.stage)}
          <Button type="button" variant="outline" onclick={() => requestCancel(item.id)}>
            <Ban aria-hidden="true" />
            Cancel
          </Button>
        {/if}
      {/if}
      <Button type="button" variant="outline" onclick={() => requestRemove(item.id)}>
        <Trash2 aria-hidden="true" />
        Remove
      </Button>
    </footer>
  </aside>
{/if}

{#if toast}
  <div
    class="fixed start-[50%] bottom-6 z-[45] flex -translate-x-1/2 items-center gap-3 rounded-lg border border-border bg-[var(--surface-high)] px-4 py-2.5 text-foreground"
    role="status"
  >
    <span>{toast.message}</span>
    <Button type="button" variant="link" size="sm" class="h-auto p-0 font-bold" onclick={() => toast?.undo()}>
      Undo
    </Button>
  </div>
{/if}
