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
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import { cn } from "$lib/utils";
  import { toast } from "svelte-sonner";
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

  let removeTimers = new Map<number, ReturnType<typeof setTimeout>>();
  let pendingAction = $state<"pause" | "resume" | "retry" | "cancel" | "remove" | "open" | null>(null);
  let confirmState = $state<{ kind: "cancel" | "remove"; id: number; title: string } | null>(null);

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

  function requestCancel(id: number) {
    if (!item || pendingAction) return;
    confirmState = { kind: "cancel", id, title: item.title ?? item.url };
  }

  function requestRemove(id: number) {
    if (!item || pendingAction) return;
    confirmState = { kind: "remove", id, title: item.title ?? item.url };
  }

  async function confirmProceed() {
    if (!confirmState) return;
    const { kind, id } = confirmState;
    confirmState = null;
    pendingAction = kind;

    if (kind === "cancel") {
      await queueStore.cancel(id);
      if (queueStore.error) {
        toast.error(`Couldn’t cancel download. ${queueStore.error}`);
      } else {
        toast("Canceled download. Undo queues it again.", {
          duration: TOAST_WINDOW_MS,
          action: { label: "Undo", onClick: () => queueStore.retry(id) },
        });
      }
      pendingAction = null;
      return;
    }

    close();
    const timer = setTimeout(async () => {
      await queueStore.bulk("remove", [id]);
      if (queueStore.error) toast.error(`Couldn’t remove download. ${queueStore.error}`);
      removeTimers.delete(id);
    }, TOAST_WINDOW_MS);
    removeTimers.set(id, timer);
    toast("Removed from the queue. The output file is kept.", {
      duration: TOAST_WINDOW_MS,
      action: {
        label: "Undo",
        onClick: () => {
          const pendingTimer = removeTimers.get(id);
          if (pendingTimer) clearTimeout(pendingTimer);
          removeTimers.delete(id);
        },
      },
    });
    pendingAction = null;
  }

  async function runItemAction(action: "pause" | "resume" | "retry", id: number) {
    if (pendingAction) return;
    pendingAction = action;
    await queueStore[action](id);
    if (queueStore.error) toast.error(`Couldn’t ${action} download. ${queueStore.error}`);
    pendingAction = null;
  }

  async function openOutput(path: string, reveal = false) {
    if (pendingAction) return;
    pendingAction = "open";
    try {
      await openPath(path, reveal);
    } catch {
      toast.error(reveal ? "Couldn’t open the output folder." : "Couldn’t open the output file.");
    } finally {
      pendingAction = null;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && confirmState === null && queueStore.activeDetailId != null) {
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
    class="fixed inset-y-0 end-0 z-[var(--z-drawer)] flex w-[var(--drawer-inline-size)] max-w-[var(--drawer-max-inline-size)] flex-col border-s border-border bg-[var(--surface-lowest)]"
    aria-labelledby="detail-drawer-title"
  >
    <header class="flex flex-col gap-2 border-b border-border px-4 py-4">
      <div class="flex items-center justify-between gap-2">
        <h2 id="detail-drawer-title" class="m-0 min-w-0 truncate text-sm" title={item.title ?? item.url}>{item.title ?? item.url}</h2>
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
        <span class="shrink-0 font-mono text-xs text-muted-foreground">{item.percent.toFixed(0)}%</span>
      </div>
    </header>

    <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-4 py-4">
      <FactsGrid
        {item}
        globalProxy={settingsStore.settings?.global_proxy ?? null}
        {presetName}
        onOpenFolder={(dir) => openPath(dir)}
      />

      <LogDisclosure itemId={item.id} stage={item.stage} />
    </div>

    <footer class="flex flex-wrap items-center justify-between gap-2 border-t border-border px-4 py-3">
      <div class="flex flex-wrap gap-2">
      {#if item.stage === "completed"}
        <Button type="button" variant="outline" disabled={pendingAction !== null || !item.output_path} onclick={() => item.output_path && openOutput(item.output_path)}>
          <FileText aria-hidden="true" />
          Open file
        </Button>
        <Button type="button" variant="outline" disabled={pendingAction !== null || !item.output_path} onclick={() => item.output_path && openOutput(item.output_path, true)}>
          <FolderOpen aria-hidden="true" />
          Open folder
        </Button>
      {:else}
        {#if ACTIVE_STAGES.has(item.stage)}
          <Button type="button" variant="outline" disabled={pendingAction !== null} onclick={() => runItemAction("pause", item.id)}>
            <Pause aria-hidden="true" />
            Pause
          </Button>
        {:else if item.stage === "paused"}
          <Button type="button" variant="outline" disabled={pendingAction !== null} onclick={() => runItemAction("resume", item.id)}>
            <Play aria-hidden="true" />
            Resume
          </Button>
        {/if}
        {#if item.stage === "error" || item.stage === "cancelled"}
          <Button type="button" variant="default" disabled={pendingAction !== null} onclick={() => runItemAction("retry", item.id)}>
            <RotateCcw aria-hidden="true" />
            Retry
          </Button>
        {/if}
        {#if !TERMINAL_STAGES.has(item.stage)}
          <Button type="button" variant="destructive" disabled={pendingAction !== null} onclick={() => requestCancel(item.id)}>
            <Ban aria-hidden="true" />
            Cancel
          </Button>
        {/if}
      {/if}
      </div>
      <Button type="button" variant="destructive" disabled={pendingAction !== null} onclick={() => requestRemove(item.id)}>
        <Trash2 aria-hidden="true" />
        Remove
      </Button>
    </footer>
  </aside>
{/if}

<AlertDialog.Root
  open={confirmState !== null}
  onOpenChange={(open) => {
    if (!open) confirmState = null;
  }}
>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>{confirmState?.kind === "cancel" ? "Cancel download?" : "Remove download from queue?"}</AlertDialog.Title>
      <AlertDialog.Description>
        {#if confirmState?.kind === "cancel"}
          Stops “{confirmState.title}”. Undo queues it again.
        {:else}
          Removes “{confirmState?.title}” from this queue after five seconds. Undo keeps it in the queue; the output file is not deleted.
        {/if}
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Keep download</AlertDialog.Cancel>
      <AlertDialog.Action variant="destructive" disabled={pendingAction !== null} onclick={confirmProceed}>
        {confirmState?.kind === "cancel" ? "Cancel download" : "Remove from queue"}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
