<script lang="ts">
  // Sidebar (UX.md S2, DESIGN.md gap #5) — + Add CTA, the status filter tree
  // (live count badges), Presets/Settings pinned bottom. Collapses to a
  // ~56px icon rail below ~1100px window width or by manual toggle
  // (DESIGN.md §6). Collapsed-rail labels are shadcn `tooltip` (T23,
  // replacing the T13 `title`-attr ponytail); each row's `Tooltip.Root` is
  // simply `disabled` while expanded so the same markup serves both states.
  import { filtersStore, STATUS_FILTERS, type StatusFilter } from "../stores/filters.svelte";
  import type { Item } from "../types";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { cn } from "$lib/utils";
  import ChevronsLeft from "lucide-svelte/icons/chevrons-left";
  import ChevronsRight from "lucide-svelte/icons/chevrons-right";
  import Plus from "lucide-svelte/icons/plus";
  import List from "lucide-svelte/icons/list";
  import Download from "lucide-svelte/icons/download";
  import Clock from "lucide-svelte/icons/clock";
  import CirclePause from "lucide-svelte/icons/circle-pause";
  import CircleCheck from "lucide-svelte/icons/circle-check";
  import CircleAlert from "lucide-svelte/icons/circle-alert";
  import Package from "lucide-svelte/icons/package";
  import SettingsIcon from "lucide-svelte/icons/settings";

  let { items, onAdd, onOpenPresets, onOpenSettings, collapsed, addDisabled = false }: {
    items: Item[];
    onAdd: () => void;
    onOpenPresets: () => void;
    onOpenSettings: () => void;
    collapsed: boolean;
    addDisabled?: boolean;
  } = $props();

  const ICON: Record<StatusFilter, typeof List> = {
    all: List,
    downloading: Download,
    queued: Clock,
    paused: CirclePause,
    completed: CircleCheck,
    failed: CircleAlert,
  };

  const LABEL: Record<StatusFilter, string> = {
    all: "All",
    downloading: "Downloading",
    queued: "Queued",
    paused: "Paused",
    completed: "Completed",
    failed: "Failed",
  };
</script>

<Tooltip.Provider>
  <nav
    class={cn(
      "flex flex-col gap-3 overflow-y-auto border-e border-border bg-card p-3 text-card-foreground",
      collapsed ? "w-14 items-center px-1.5" : "w-60",
    )}
    aria-label="Queue navigation"
  >
    <Tooltip.Root disabled={!collapsed}>
      <Tooltip.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            type="button"
            variant="ghost"
            class="w-full justify-center text-muted-foreground"
            onclick={() => filtersStore.toggleCollapsed()}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            {#if collapsed}
              <ChevronsRight aria-hidden="true" />
            {:else}
              <ChevronsLeft aria-hidden="true" />
            {/if}
          </Button>
        {/snippet}
      </Tooltip.Trigger>
      <Tooltip.Content side="right">{collapsed ? "Expand sidebar" : "Collapse sidebar"}</Tooltip.Content>
    </Tooltip.Root>

    <Tooltip.Root disabled={!collapsed}>
      <Tooltip.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            type="button"
            variant="default"
            class="w-full justify-center gap-2 font-bold"
            onclick={onAdd}
            disabled={addDisabled}
          >
            <Plus aria-hidden="true" />
            {#if !collapsed}<span>Add</span>{/if}
          </Button>
        {/snippet}
      </Tooltip.Trigger>
      <Tooltip.Content side="right">
        {addDisabled ? "Set up yt-dlp/ffmpeg in Settings to enable downloads." : "Add"}
      </Tooltip.Content>
    </Tooltip.Root>

    <ul class="m-0 flex w-full list-none flex-col gap-0.5 p-0">
      {#each STATUS_FILTERS as filter (filter)}
        {@const count = filtersStore.countFor(filter, items)}
        {@const active = filtersStore.status === filter}
        {@const Icon = ICON[filter]}
        <li>
          <Tooltip.Root disabled={!collapsed}>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  type="button"
                  variant="ghost"
                  class={cn(
                    "w-full border-s-2",
                    active ? "border-s-primary bg-accent font-bold" : "border-s-transparent",
                    collapsed ? "justify-center px-0" : "justify-start gap-2",
                  )}
                  onclick={() => filtersStore.setStatus(filter)}
                  aria-current={active ? "true" : undefined}
                >
                  <Icon aria-hidden="true" />
                  {#if !collapsed}<span class="flex-1 text-start">{LABEL[filter]}</span>{/if}
                  {#if !collapsed}
                    <Badge variant="secondary" class="font-mono">{count}</Badge>
                  {/if}
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content side="right">{LABEL[filter]}</Tooltip.Content>
          </Tooltip.Root>
        </li>
      {/each}
    </ul>

    <div class="mt-auto flex w-full flex-col gap-0.5">
      <Tooltip.Root disabled={!collapsed}>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              type="button"
              variant="ghost"
              class={cn("w-full", collapsed ? "justify-center" : "justify-start gap-2")}
              onclick={onOpenPresets}
            >
              <Package aria-hidden="true" />
              {#if !collapsed}<span>Presets</span>{/if}
            </Button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content side="right">Presets</Tooltip.Content>
      </Tooltip.Root>

      <!-- Settings' hover hint always names the shortcut, in both rail
           states — matches the pre-migration `title` behavior verbatim. -->
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              type="button"
              variant="ghost"
              class={cn("w-full", collapsed ? "justify-center" : "justify-start gap-2")}
              onclick={onOpenSettings}
            >
              <SettingsIcon aria-hidden="true" />
              {#if !collapsed}<span>Settings</span>{/if}
            </Button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content side="right">Settings (Ctrl/Cmd+,)</Tooltip.Content>
      </Tooltip.Root>
    </div>
  </nav>
</Tooltip.Provider>
