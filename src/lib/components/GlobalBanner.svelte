<script lang="ts">
  // GlobalBanner (UX.md S7 "binary went missing mid-session" state, T16) — a
  // persistent, app-wide banner: "<binary> is no longer at its path —
  // downloads are paused" + a Fix button that reopens S1 (K1-AC7).
  import { binaryHealthStore } from "../stores/binaryHealth.svelte";

  let { onFix }: { onFix: () => void } = $props();

  const LABEL: Record<string, string> = {
    ytdlp: "yt-dlp",
    ffmpeg: "ffmpeg",
  };
</script>

{#if binaryHealthStore.missing}
  <div class="banner" role="alert">
    <span class="glyph" aria-hidden="true">⚠</span>
    <span class="message">
      {LABEL[binaryHealthStore.missing.which] ?? binaryHealthStore.missing.which} is no longer at its
      path — downloads are paused.
    </span>
    <button type="button" onclick={onFix}>Fix</button>
  </div>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 1rem;
    background: var(--warning);
    color: var(--warning-foreground);
    font-size: 0.9em;
  }
  .glyph {
    font-size: 1.1em;
  }
  .message {
    flex: 1;
  }
  button {
    background: var(--warning-foreground);
    color: var(--warning);
    border: none;
    border-radius: var(--radius);
    padding: 0.3rem 0.75rem;
    font-family: var(--font-sans);
    font-weight: 700;
    cursor: pointer;
  }
  button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>
