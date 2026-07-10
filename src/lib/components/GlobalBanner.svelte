<script lang="ts">
  // GlobalBanner (UX.md S7 "binary went missing mid-session" state, T16,
  // migrated to shadcn/lucide at T28) — a persistent, app-wide banner:
  // "<binary> is no longer at its path — downloads are paused" + a Fix
  // button that reopens S1 (K1-AC7). Built on shadcn `alert`, themed to the
  // --warning/--warning-foreground tokens (DESIGN.md §2 "tertiary") via
  // arbitrary utilities rather than alert's own default/destructive
  // variants — neither maps to "warning" (T28 AC1).
  import { binaryHealthStore } from "../stores/binaryHealth.svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import TriangleAlert from "lucide-svelte/icons/triangle-alert";

  let { onFix }: { onFix: () => void } = $props();

  const LABEL: Record<string, string> = {
    ytdlp: "yt-dlp",
    ffmpeg: "ffmpeg",
  };
</script>

{#if binaryHealthStore.missing}
  <Alert.Root
    class="flex w-full items-center gap-2.5 rounded-none border-0 bg-[var(--warning)] px-4 py-2 text-[var(--warning-foreground)]"
  >
    <TriangleAlert aria-hidden="true" class="size-4 shrink-0 text-[var(--warning-foreground)]" />
    <Alert.Description class="flex-1 text-[0.9em] text-[var(--warning-foreground)]">
      {LABEL[binaryHealthStore.missing.which] ?? binaryHealthStore.missing.which} is no longer at its
      path — downloads are paused.
    </Alert.Description>
    <Button
      type="button"
      onclick={onFix}
      class="h-auto shrink-0 bg-[var(--warning-foreground)] px-3 py-1 text-[var(--warning)] hover:bg-[var(--warning-foreground)]/90"
    >
      Fix
    </Button>
  </Alert.Root>
{/if}
