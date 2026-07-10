<script lang="ts">
  // S6 — Presets (UX.md, TASKS.md T11, migrated to shadcn/lucide at T27).
  // Region 1: named-preset list, default starred + sorted first (eye first).
  // Region 2: inline editor that opens on row select, matching UX.md's
  // "EDITOR (inline, on select)" layout — no separate route/modal.
  import { presetsStore } from "../stores/presets.svelte";
  import type { Preset } from "../types";
  import * as Table from "$lib/components/ui/table";
  import { Input } from "$lib/components/ui/input";
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { Badge } from "$lib/components/ui/badge";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import { cn } from "$lib/utils";
  import Plus from "lucide-svelte/icons/plus";
  import Star from "lucide-svelte/icons/star";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import Loader2 from "lucide-svelte/icons/loader-circle";

  let selectedId = $state<number | null>(null);
  let creating = $state(false);

  let name = $state("");
  let isDefault = $state(false);
  let formatExpr = $state("");
  let outputTemplate = $state("");
  let proxy = $state("");
  let extraArgs = $state("");

  let saveError = $state<string | null>(null);
  let saveErrorStderr = $state<string | null>(null);
  let saving = $state(false);
  let deleteError = $state<string | null>(null);
  // shadcn alert-dialog, replacing the hand-rolled confirm() — set by
  // requestDelete, cleared once the user picks an option; the dialog itself
  // never calls the store directly.
  let deleteConfirmOpen = $state(false);

  const selected = $derived(presetsStore.presets.find((p) => p.id === selectedId) ?? null);

  function selectPreset(preset: Preset) {
    creating = false;
    selectedId = preset.id;
    name = preset.name;
    isDefault = preset.is_default;
    formatExpr = preset.format_expr;
    outputTemplate = preset.output_template;
    proxy = preset.proxy ?? "";
    extraArgs = preset.extra_args ?? "";
    saveError = null;
    saveErrorStderr = null;
    deleteError = null;
  }

  function selectRowKeydown(e: KeyboardEvent, preset: Preset) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      selectPreset(preset);
    }
  }

  function startCreate() {
    creating = true;
    selectedId = null;
    name = "";
    isDefault = false;
    formatExpr = "";
    outputTemplate = "%(title)s.%(ext)s";
    proxy = "";
    extraArgs = "";
    saveError = null;
    saveErrorStderr = null;
    deleteError = null;
  }

  function closeEditor() {
    creating = false;
    selectedId = null;
  }

  async function save() {
    saveError = null;
    saveErrorStderr = null;
    saving = true;
    try {
      if (creating) {
        const preset = await presetsStore.create({
          name,
          format_expr: formatExpr,
          output_template: outputTemplate,
          proxy: proxy.trim() || null,
          extra_args: extraArgs.trim() || null,
          is_default: isDefault,
        });
        if (preset) {
          selectPreset(preset);
        } else if (presetsStore.error) {
          saveError = presetsStore.error.message;
          saveErrorStderr = presetsStore.error.stderr ?? null;
        }
      } else if (selectedId != null) {
        const preset = await presetsStore.update({
          id: selectedId,
          name,
          format_expr: formatExpr,
          output_template: outputTemplate,
          proxy: proxy.trim() || null,
          extra_args: extraArgs.trim() || null,
        });
        if (preset) {
          if (isDefault && !preset.is_default) {
            await presetsStore.setDefault(preset.id);
          }
          const refreshed = presetsStore.presets.find((p) => p.id === preset.id);
          if (refreshed) selectPreset(refreshed);
        } else if (presetsStore.error) {
          saveError = presetsStore.error.message;
          saveErrorStderr = presetsStore.error.stderr ?? null;
        }
      }
    } finally {
      saving = false;
    }
  }

  function requestDelete() {
    if (selectedId == null) return;
    deleteError = null;
    deleteConfirmOpen = true;
  }

  // Runs only once the alert-dialog's destructive action is confirmed.
  async function confirmDelete() {
    deleteConfirmOpen = false;
    if (selectedId == null) return;
    const ok = await presetsStore.remove(selectedId);
    if (ok) {
      closeEditor();
    } else if (presetsStore.error) {
      deleteError = presetsStore.error.message;
    }
  }
</script>

<main class="mx-auto flex w-full max-w-3xl flex-col gap-4 p-6">
  <div class="flex items-center justify-between">
    <h1 class="m-0 text-xl font-semibold">Presets</h1>
    <Button type="button" onclick={startCreate}>
      <Plus aria-hidden="true" class="size-4" />
      New preset
    </Button>
  </div>

  {#if presetsStore.error && !saveError && !deleteError}
    <p class="m-0 text-[var(--error-token)]">{presetsStore.error.message}</p>
  {/if}

  <Table.Root>
    <Table.Header>
      <Table.Row>
        <Table.Head class="w-8"><span class="sr-only">Default</span></Table.Head>
        <Table.Head>Name</Table.Head>
        <Table.Head>Expression</Table.Head>
      </Table.Row>
    </Table.Header>
    <Table.Body>
      {#each presetsStore.presets as preset (preset.id)}
        <Table.Row
          tabindex={0}
          aria-selected={preset.id === selectedId}
          data-state={preset.id === selectedId ? "selected" : undefined}
          class="cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--ring)]"
          onclick={() => selectPreset(preset)}
          onkeydown={(e) => selectRowKeydown(e, preset)}
        >
          <Table.Cell>
            {#if preset.is_default}
              <Star aria-hidden="true" class="size-4 fill-primary text-primary" />
            {/if}
          </Table.Cell>
          <Table.Cell class={cn("font-medium", preset.id === selectedId && "font-semibold")}>
            <span class="flex items-center gap-2">
              {preset.name}
              {#if preset.is_default}<Badge variant="secondary">Default</Badge>{/if}
            </span>
          </Table.Cell>
          <Table.Cell class="font-mono text-[0.85em] text-muted-foreground">{preset.format_expr}</Table.Cell>
        </Table.Row>
      {:else}
        <Table.Row>
          <Table.Cell colspan={3} class="text-center text-muted-foreground">No presets yet.</Table.Cell>
        </Table.Row>
      {/each}
    </Table.Body>
  </Table.Root>

  {#if creating || selected}
    <div class="ring-foreground/10 bg-card text-card-foreground flex flex-col gap-3 rounded-xl p-4 ring-1">
      <label class="flex flex-col gap-1.5 text-[0.85em] text-muted-foreground">
        <span>Name</span>
        <Input type="text" bind:value={name} placeholder="4K" />
      </label>
      <label class="flex flex-row items-center gap-2">
        <Checkbox bind:checked={isDefault} />
        <span class="text-[0.85em] text-muted-foreground">Default</span>
      </label>
      <label class="flex flex-col gap-1.5 text-[0.85em] text-muted-foreground">
        <span>Format expr</span>
        <Input type="text" class="font-mono" bind:value={formatExpr} placeholder="bv*[height<=2160]+ba/b" />
      </label>
      <label class="flex flex-col gap-1.5 text-[0.85em] text-muted-foreground">
        <span>Output tmpl</span>
        <Input type="text" class="font-mono" bind:value={outputTemplate} placeholder="%(title)s.%(ext)s" />
      </label>
      <label class="flex flex-col gap-1.5 text-[0.85em] text-muted-foreground">
        <span>Proxy</span>
        <Input type="text" bind:value={proxy} placeholder="(inherit global)" />
      </label>
      <label class="flex flex-col gap-1.5 text-[0.85em] text-muted-foreground">
        <span>Extra args</span>
        <Input type="text" class="font-mono" bind:value={extraArgs} placeholder="--embed-thumbnail" />
      </label>

      {#if saveError}
        <div class="flex flex-col gap-2">
          <p class="m-0 text-[var(--error-token)]">{saveError}</p>
          {#if saveErrorStderr}
            <pre class="m-0 max-h-32 overflow-y-auto rounded-lg border border-border bg-muted p-2.5 font-mono text-[0.85em] whitespace-pre-wrap text-[var(--error-token)]">{saveErrorStderr}</pre>
          {/if}
        </div>
      {/if}
      {#if deleteError}
        <p class="m-0 text-[var(--error-token)]">{deleteError}</p>
      {/if}

      <footer class="flex justify-end gap-2">
        {#if !creating}
          <Button
            type="button"
            variant="ghost"
            class="me-auto text-[var(--error-token)] hover:text-[var(--error-token)]"
            onclick={requestDelete}
          >
            <Trash2 aria-hidden="true" class="size-4" />
            Delete
          </Button>
        {/if}
        <Button type="button" variant="outline" onclick={closeEditor}>Cancel</Button>
        <Button
          type="button"
          disabled={!name.trim() || !formatExpr.trim() || !outputTemplate.trim() || saving}
          onclick={save}
        >
          {#if saving}<Loader2 aria-hidden="true" class="size-4 animate-spin" />{/if}
          {saving ? "Saving…" : "Save"}
        </Button>
      </footer>
    </div>
  {/if}
</main>

<AlertDialog.Root open={deleteConfirmOpen} onOpenChange={(open) => (deleteConfirmOpen = open)}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete preset "{name}"?</AlertDialog.Title>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Keep</AlertDialog.Cancel>
      <AlertDialog.Action variant="destructive" onclick={confirmDelete}>Delete</AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
