<script lang="ts">
  // StageToken (UX.md S2, DESIGN.md §4 gap #2) — icon + label-mono text chip,
  // one per stage. Never color alone (NFR-4/hard rule #4): every stage pairs
  // a distinct lucide glyph with its literal stage name. Colors per
  // DESIGN.md gap #2: active states `primary`, paused/queued/cancelled
  // `on-surface-variant` (muted-foreground), done `secondary`, error
  // `--error-token`.
  import { cn } from "$lib/utils";
  import Download from "lucide-svelte/icons/download";
  import Merge from "lucide-svelte/icons/merge";
  import Clock from "lucide-svelte/icons/clock";
  import CirclePause from "lucide-svelte/icons/circle-pause";
  import CircleCheck from "lucide-svelte/icons/circle-check";
  import CircleAlert from "lucide-svelte/icons/circle-alert";
  import CircleX from "lucide-svelte/icons/circle-x";
  import Circle from "lucide-svelte/icons/circle";

  let { stage }: { stage: string } = $props();

  const ICON: Record<string, typeof Download> = {
    downloading: Download,
    merging: Merge,
    queued: Clock,
    paused: CirclePause,
    completed: CircleCheck,
    error: CircleAlert,
    cancelled: CircleX,
  };

  const COLOR_CLASS: Record<string, string> = {
    downloading: "text-primary",
    merging: "text-primary",
    queued: "text-muted-foreground",
    paused: "text-muted-foreground",
    completed: "text-secondary",
    error: "text-[var(--error-token)]",
    cancelled: "text-muted-foreground",
  };

  const Icon = $derived(ICON[stage] ?? Circle);
</script>

<span
  class={cn(
    "inline-flex items-center gap-1 whitespace-nowrap font-mono text-[0.85em]",
    COLOR_CLASS[stage] ?? "text-muted-foreground",
  )}
>
  <Icon aria-hidden="true" class="size-3.5 shrink-0" />
  <span>{stage}</span>
</span>
