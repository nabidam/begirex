<script lang="ts">
  // S1 — First-run Onboarding, full wizard (UX.md S1, TASKS.md T17,
  // replacing T4's minimal blocking version; migrated to shadcn/lucide at
  // T28). Region 1: engine check, one BinaryRow per binary with
  // found/downloading/error states. Region 2: optional global proxy.
  // Continue is gated on both binaries resolved; "I'll set it later" is the
  // degraded-mode escape hatch (AC2) — App.svelte lands on S2 without
  // requiring bothFound. The whole wizard is one shadcn `card` (T28 AC1),
  // same single-box layout T17 shipped, now composed of real Card
  // sub-parts instead of one hand-rolled bordered div.
  import { settingsStore } from "../stores/settings.svelte";
  import { pickBinaryPath } from "../ipc";
  import BinaryRow from "../components/BinaryRow.svelte";
  import * as Card from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as Alert from "$lib/components/ui/alert";
  import CircleCheck from "lucide-svelte/icons/circle-check";

  let { onContinue, onSkip }: { onContinue: () => void; onSkip: () => void } = $props();

  let proxy = $state(settingsStore.settings?.global_proxy ?? "");
  let continuing = $state(false);

  const BINARIES: Array<["ytdlp" | "ffmpeg", string]> = [
    ["ytdlp", "yt-dlp"],
    ["ffmpeg", "ffmpeg"],
  ];

  const bothFound = $derived(
    settingsStore.settings?.build_flavor === "bundled" ||
      (settingsStore.binaries?.ytdlp.found === true && settingsStore.binaries?.ffmpeg.found === true),
  );

  async function setPath(which: "ytdlp" | "ffmpeg") {
    const path = await pickBinaryPath();
    if (path) {
      await settingsStore.resolveBinaryPath(which, path);
    }
  }

  async function continueClick() {
    continuing = true;
    try {
      await settingsStore.saveProxy(proxy);
      if (!settingsStore.error) onContinue();
    } finally {
      continuing = false;
    }
  }
</script>

<main class="mx-auto my-12 w-full max-w-[34rem] p-4">
  <Card.Root>
    <Card.Header>
      <Card.Title class="text-[1.1em]">BegireX · First-time setup</Card.Title>
    </Card.Header>

    <Card.Content class="flex flex-col gap-5">
      {#if settingsStore.error}
        <Alert.Root class="border-[var(--error-token)]">
          <Alert.Description class="text-[var(--error-token)]">{settingsStore.error}</Alert.Description>
        </Alert.Root>
      {/if}

      <section class="flex flex-col gap-2.5">
        <h2 class="m-0 text-[0.9em] tracking-wide text-muted-foreground uppercase">Engine check</h2>
        {#if settingsStore.settings?.build_flavor === "bundled"}
          <!-- UX.md S1 density note: the bundled build skips detection entirely
               (ARCHITECTURE §9 seeds ytdlp_path/ffmpeg_path to its shipped
               binaries) — this screen only fully appears for the light build. -->
          <p class="m-0 flex items-center gap-1.5 font-mono text-primary">
            <CircleCheck aria-hidden="true" class="size-4" />
            Engine bundled
          </p>
        {:else}
          <div class="flex flex-col gap-2.5">
            {#each BINARIES as [which, label] (which)}
              <BinaryRow
                {label}
                status={settingsStore.binaries?.[which]}
                onSetPath={() => setPath(which)}
                onDownload={() => settingsStore.downloadBinary(which)}
                downloadState={settingsStore.downloads[which]}
              />
            {/each}
          </div>
        {/if}
      </section>

      <section class="flex flex-col gap-2.5">
        <h2 class="m-0 text-[0.9em] tracking-wide text-muted-foreground uppercase">
          Network <span class="font-normal tracking-normal normal-case">(optional)</span>
        </h2>
        <label class="flex flex-col gap-1">
          <span class="text-[0.85em] text-muted-foreground">Proxy</span>
          <Input type="text" bind:value={proxy} placeholder="socks5://user:pass@host:port" />
        </label>
      </section>
    </Card.Content>

    <Card.Footer class="justify-end gap-3">
      <Button type="button" variant="ghost" onclick={onSkip}>I'll set it later</Button>
      <Button type="button" disabled={!bothFound || continuing} onclick={continueClick}>
        {continuing ? "Continuing…" : "Continue"}
      </Button>
    </Card.Footer>
  </Card.Root>
</main>
