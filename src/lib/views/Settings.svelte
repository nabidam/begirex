<script lang="ts">
  // S7 — Settings (UX.md, TASKS.md T17, migrated to shadcn/lucide at T28).
  // Region 1 Engine & health (→ eye first): BinaryRow per binary (no
  // onDownload — S7 never re-triggers an in-app fetch, missing-mid-session
  // is GlobalBanner's job) + Re-check + Re-run onboarding. Region 2
  // Downloads: N/output dir/template/default preset. Region 3 Network:
  // global proxy. Region 4 About: build flavor + versions. Each region is
  // its own shadcn `card` (T28 AC1), one per the pre-existing `.card`
  // section this task's boxes already visually were.
  import { settingsStore } from "../stores/settings.svelte";
  import { presetsStore } from "../stores/presets.svelte";
  import { pickBinaryPath, pickDirectory } from "../ipc";
  import BinaryRow from "../components/BinaryRow.svelte";
  import * as Card from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as Select from "$lib/components/ui/select";
  import * as Alert from "$lib/components/ui/alert";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import FolderOpen from "lucide-svelte/icons/folder-open";
  import Check from "lucide-svelte/icons/check";

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

<main class="mx-auto flex w-full max-w-3xl flex-col gap-5 p-6">
  <h1 class="m-0 text-xl font-semibold">Settings</h1>

  {#if settingsStore.error}
    <Alert.Root class="border-[var(--error-token)]">
      <Alert.Description class="text-[var(--error-token)]">{settingsStore.error}</Alert.Description>
    </Alert.Root>
  {/if}

  <Card.Root size="sm">
    <Card.Header>
      <Card.Title class="text-[0.9em] tracking-wide text-muted-foreground uppercase">Engine &amp; health</Card.Title>
    </Card.Header>
    <Card.Content class="flex flex-col gap-3">
      <div class="flex flex-col gap-2.5">
        {#each BINARIES as [which, label] (which)}
          <BinaryRow label={label} status={settingsStore.binaries?.[which]} onSetPath={() => changePath(which)} />
        {/each}
      </div>
      <div class="flex gap-2">
        <Button type="button" variant="outline" size="sm" onclick={recheck} disabled={recheckState === "checking"}>
          <RefreshCw aria-hidden="true" class={recheckState === "checking" ? "size-3.5 animate-spin" : "size-3.5"} />
          {recheckState === "checking" ? "Re-checking…" : "Re-check"}
        </Button>
        <Button type="button" variant="outline" size="sm" onclick={onReRunOnboarding}>Re-run onboarding</Button>
      </div>
    </Card.Content>
  </Card.Root>

  <Card.Root size="sm">
    <Card.Header>
      <Card.Title class="text-[0.9em] tracking-wide text-muted-foreground uppercase">Downloads</Card.Title>
    </Card.Header>
    <Card.Content class="flex flex-col gap-3">
      <label class="flex max-w-[26rem] flex-col gap-1">
        <span class="text-[0.85em] text-muted-foreground">Parallel downloads (N)</span>
        <Input
          type="number"
          min="1"
          bind:value={concurrency}
          onchange={saveConcurrency}
          aria-label="Default parallel downloads"
        />
        {#if savedField === "concurrency"}
          <span class="inline-flex items-center gap-1 text-[0.85em] text-primary"><Check aria-hidden="true" class="size-3.5" />Saved</span>
        {/if}
      </label>
      <label class="flex max-w-[26rem] flex-col gap-1">
        <span class="text-[0.85em] text-muted-foreground">Default output dir</span>
        <div class="flex gap-1.5">
          <Input type="text" class="flex-1" bind:value={outputDir} readonly />
          <Button type="button" variant="outline" size="icon" onclick={pickOutputDir} aria-label="Choose directory">
            <FolderOpen aria-hidden="true" />
          </Button>
        </div>
        {#if savedField === "output_dir"}
          <span class="inline-flex items-center gap-1 text-[0.85em] text-primary"><Check aria-hidden="true" class="size-3.5" />Saved</span>
        {/if}
      </label>
      <label class="flex max-w-[26rem] flex-col gap-1">
        <span class="text-[0.85em] text-muted-foreground">Default filename tmpl</span>
        <Input
          type="text"
          class="font-mono"
          bind:value={outputTemplate}
          onchange={saveOutputTemplate}
          placeholder="%(title)s.%(ext)s"
        />
        {#if savedField === "output_template"}
          <span class="inline-flex items-center gap-1 text-[0.85em] text-primary"><Check aria-hidden="true" class="size-3.5" />Saved</span>
        {/if}
      </label>
      <label class="flex max-w-[26rem] flex-col gap-1">
        <span class="text-[0.85em] text-muted-foreground">Default preset</span>
        <Select.Root type="single" value={String(defaultPresetId)} onValueChange={handleDefaultPresetChange}>
          <Select.Trigger class="w-full">
            {defaultPresetLabel}
          </Select.Trigger>
          <Select.Content>
            {#each presetsStore.presets as preset (preset.id)}
              <Select.Item value={String(preset.id)} label={preset.name}>
                {preset.name}
              </Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
        {#if savedField === "default_preset"}
          <span class="inline-flex items-center gap-1 text-[0.85em] text-primary"><Check aria-hidden="true" class="size-3.5" />Saved</span>
        {/if}
      </label>
    </Card.Content>
  </Card.Root>

  <Card.Root size="sm">
    <Card.Header>
      <Card.Title class="text-[0.9em] tracking-wide text-muted-foreground uppercase">Network</Card.Title>
    </Card.Header>
    <Card.Content class="flex flex-col gap-3">
      <label class="flex max-w-[26rem] flex-col gap-1">
        <span class="text-[0.85em] text-muted-foreground">Global proxy</span>
        <Input type="text" bind:value={proxy} onchange={saveProxy} placeholder="socks5://user:pass@host:port" />
        {#if savedField === "proxy"}
          <span class="inline-flex items-center gap-1 text-[0.85em] text-primary"><Check aria-hidden="true" class="size-3.5" />Saved</span>
        {/if}
      </label>
    </Card.Content>
  </Card.Root>

  <Card.Root size="sm">
    <Card.Header>
      <Card.Title class="text-[0.9em] tracking-wide text-muted-foreground uppercase">About</Card.Title>
    </Card.Header>
    <Card.Content>
      <p class="m-0 font-mono text-foreground">
        Build: {settingsStore.settings?.build_flavor ?? "—"} · BegireX 0.1.0
        {#if settingsStore.settings?.ytdlp_version}· yt-dlp {settingsStore.settings.ytdlp_version}{/if}
        {#if settingsStore.settings?.ffmpeg_version}· ffmpeg {settingsStore.settings.ffmpeg_version}{/if}
      </p>
    </Card.Content>
  </Card.Root>
</main>
