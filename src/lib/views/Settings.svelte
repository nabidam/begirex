<script lang="ts">
  // S7 — Settings (UX.md, TASKS.md T17, migrated to shadcn/lucide at T28).
  // Engine health is operational and kept distinct. The remaining defaults
  // live in one linear form so routine configuration is easy to scan.
  import { settingsStore } from "../stores/settings.svelte";
  import { presetsStore } from "../stores/presets.svelte";
  import { pickBinaryPath, pickDirectory } from "../ipc";
  import BinaryRow from "../components/BinaryRow.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as Select from "$lib/components/ui/select";
  import * as Alert from "$lib/components/ui/alert";
  import Check from "lucide-svelte/icons/check";
  import CircleCheck from "lucide-svelte/icons/circle-check";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import FolderOpen from "lucide-svelte/icons/folder-open";
  import Loader2 from "lucide-svelte/icons/loader-circle";
  import TriangleAlert from "lucide-svelte/icons/triangle-alert";

  let { onReRunOnboarding }: { onReRunOnboarding: () => void } = $props();

  const BINARIES: Array<["ytdlp" | "ffmpeg", string]> = [
    ["ytdlp", "yt-dlp"],
    ["ffmpeg", "ffmpeg"],
  ];

  let concurrency = $state(settingsStore.settings?.default_concurrency ?? 2);
  let outputDir = $state(settingsStore.settings?.default_output_dir ?? "");
  let outputTemplate = $state(settingsStore.settings?.default_output_template ?? "");
  let proxy = $state(settingsStore.settings?.global_proxy ?? "");
  // T17 ambiguity note (internal, no user-visible behavior change): S7's
  // "Default preset" field is wired to presetsStore.setDefault (the
  // `is_default` flag S3/S6 actually read — AddDownload.svelte picks
  // presetsStore.defaultPreset, not settings.default_preset_id) rather than
  // settings.update_settings{default_preset_id}, so there is exactly one
  // source of truth for "the default preset" instead of two disconnected
  // ones with the same name.
  let defaultPresetId = $state(presetsStore.defaultPreset?.id ?? "");

  let savedField = $state<string | null>(null);
  let recheckState = $state<"idle" | "checking">("idle");

  const defaultPresetLabel = $derived.by((): string => {
    const preset = presetsStore.presets.find((p) => p.id === defaultPresetId);
    return preset?.name ?? "Select a preset";
  });

  const savedMessage = $derived.by((): string | null => {
    const labels: Record<string, string> = {
      concurrency: "Parallel downloads",
      output_dir: "Output directory",
      output_template: "Filename template",
      default_preset: "Default preset",
      proxy: "Global proxy",
    };
    return savedField ? `${labels[savedField]} saved.` : null;
  });

  const engineStatus = $derived.by((): "ready" | "attention" | "checking" => {
    const statuses = BINARIES.map(([which]) => settingsStore.binaries?.[which]);
    if (statuses.some((status) => !status)) return "checking";
    return statuses.every((status) => status?.found) ? "ready" : "attention";
  });

  function flashSaved(field: string) {
    savedField = field;
    setTimeout(() => {
      if (savedField === field) savedField = null;
    }, 1500);
  }

  async function changePath(which: "ytdlp" | "ffmpeg") {
    const path = await pickBinaryPath();
    if (path) await settingsStore.resolveBinaryPath(which, path);
  }

  async function recheck() {
    recheckState = "checking";
    try {
      await settingsStore.recheck();
    } finally {
      recheckState = "idle";
    }
  }

  async function saveConcurrency() {
    if (Number.isInteger(concurrency) && concurrency >= 1) {
      const ok = await settingsStore.update({ default_concurrency: concurrency });
      if (ok) flashSaved("concurrency");
    }
  }

  async function pickOutputDir() {
    const dir = await pickDirectory();
    if (dir) {
      outputDir = dir;
      const ok = await settingsStore.update({ default_output_dir: dir });
      if (ok) flashSaved("output_dir");
    }
  }

  async function saveOutputTemplate() {
    if (outputTemplate.trim()) {
      const ok = await settingsStore.update({ default_output_template: outputTemplate.trim() });
      if (ok) flashSaved("output_template");
    }
  }

  async function saveProxy() {
    const ok = await settingsStore.update({ global_proxy: proxy.trim() || null });
    if (ok) flashSaved("proxy");
  }

  async function handleDefaultPresetChange(value: string) {
    if (!value) return;
    const id = Number(value);
    defaultPresetId = id;
    const ok = await presetsStore.setDefault(id);
    if (ok) flashSaved("default_preset");
  }
</script>

<main class="mx-auto flex w-full max-w-2xl flex-col gap-6 p-6">
  <header class="flex flex-col gap-1">
    <h1 class="m-0 text-xl font-semibold">Settings</h1>
    <p class="m-0 text-sm text-muted-foreground">Download defaults and engine configuration.</p>
  </header>

  {#if settingsStore.error}
    <Alert.Root class="border-[var(--error-token)]">
      <Alert.Description class="text-[var(--error-token)]">{settingsStore.error}</Alert.Description>
    </Alert.Root>
  {/if}

  <section class="flex flex-col gap-3" aria-labelledby="engine-heading">
    <div class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
      <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
        <h2 id="engine-heading" class="m-0 text-base font-semibold">Engine health</h2>
        {#if engineStatus === "ready"}
          <span class="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2 py-0.5 font-mono text-xs text-primary">
            <CircleCheck aria-hidden="true" class="size-3" />
            Tools ready
          </span>
        {:else if engineStatus === "attention"}
          <span class="inline-flex items-center gap-1 rounded-full bg-[var(--warning)]/10 px-2 py-0.5 font-mono text-xs text-[var(--warning)]">
            <TriangleAlert aria-hidden="true" class="size-3" />
            Setup needed
          </span>
        {:else}
          <span class="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 font-mono text-xs text-muted-foreground">
            <Loader2 aria-hidden="true" class="size-3 animate-spin" />
            Checking tools
          </span>
        {/if}
        <p class="m-0 text-sm text-muted-foreground">Verify the tools BegireX uses to process downloads.</p>
      </div>
      <Button type="button" variant="ghost" size="sm" onclick={onReRunOnboarding}>Re-run onboarding</Button>
    </div>
    <div class="flex flex-col gap-2.5">
      {#each BINARIES as [which, label] (which)}
        <BinaryRow label={label} status={settingsStore.binaries?.[which]} onSetPath={() => changePath(which)} />
      {/each}
    </div>
    <div>
      <Button type="button" variant="outline" size="sm" onclick={recheck} disabled={recheckState === "checking"}>
        <RefreshCw aria-hidden="true" class={recheckState === "checking" ? "size-3.5 animate-spin" : "size-3.5"} />
        {recheckState === "checking" ? "Re-checking…" : "Re-check"}
      </Button>
    </div>
  </section>

  <section class="flex flex-col gap-3" aria-labelledby="defaults-heading">
    <div>
      <h2 id="defaults-heading" class="m-0 text-base font-semibold">Download defaults</h2>
      <p class="m-0 text-sm text-muted-foreground">Applied to new downloads unless a preset overrides them.</p>
    </div>
    {#if savedMessage}
      <p class="inline-flex w-fit items-center gap-1.5 rounded-md bg-primary/10 px-2 py-1 text-sm text-primary" role="status">
        <Check aria-hidden="true" class="size-3.5" />
        {savedMessage}
      </p>
    {/if}
    <div class="divide-y divide-border border-y border-border">
      <label class="grid gap-1.5 py-3 sm:grid-cols-[11rem_minmax(0,1fr)] sm:items-center sm:gap-4">
        <span class="text-sm">Parallel downloads</span>
        <Input
          type="number"
          min="1"
          class="sm:max-w-28"
          bind:value={concurrency}
          onchange={saveConcurrency}
          aria-label="Default parallel downloads"
        />
      </label>
      <label class="grid gap-1.5 py-3 sm:grid-cols-[11rem_minmax(0,1fr)] sm:items-center sm:gap-4">
        <span class="text-sm">Output directory</span>
        <div class="flex min-w-0 gap-1.5">
          <Input type="text" class="min-w-0 flex-1" bind:value={outputDir} readonly />
          <Button type="button" variant="outline" size="icon" onclick={pickOutputDir} aria-label="Choose directory">
            <FolderOpen aria-hidden="true" />
          </Button>
        </div>
      </label>
      <label class="grid gap-1.5 py-3 sm:grid-cols-[11rem_minmax(0,1fr)] sm:items-center sm:gap-4">
        <span class="text-sm">Filename template</span>
        <Input
          type="text"
          class="font-mono"
          bind:value={outputTemplate}
          onchange={saveOutputTemplate}
          placeholder="%(title)s.%(ext)s"
        />
      </label>
      <label class="grid gap-1.5 py-3 sm:grid-cols-[11rem_minmax(0,1fr)] sm:items-center sm:gap-4">
        <span class="text-sm">Default preset</span>
        <Select.Root type="single" value={String(defaultPresetId)} onValueChange={handleDefaultPresetChange}>
          <Select.Trigger class="w-full">{defaultPresetLabel}</Select.Trigger>
          <Select.Content>
            {#each presetsStore.presets as preset (preset.id)}
              <Select.Item value={String(preset.id)} label={preset.name}>{preset.name}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      </label>
      <label class="grid gap-1.5 py-3 sm:grid-cols-[11rem_minmax(0,1fr)] sm:items-center sm:gap-4">
        <span class="text-sm">Global proxy</span>
        <Input type="text" bind:value={proxy} onchange={saveProxy} placeholder="socks5://user:pass@host:port" />
      </label>
    </div>
  </section>

  <footer class="border-t border-border pt-3 font-mono text-xs text-muted-foreground">
    Build {settingsStore.settings?.build_flavor ?? "—"} · BegireX 0.1.0
    {#if settingsStore.settings?.ytdlp_version}· yt-dlp {settingsStore.settings.ytdlp_version}{/if}
    {#if settingsStore.settings?.ffmpeg_version}· ffmpeg {settingsStore.settings.ffmpeg_version}{/if}
  </footer>
</main>
