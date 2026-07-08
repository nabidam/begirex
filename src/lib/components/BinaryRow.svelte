<script lang="ts">
  // BinaryRow (UX.md S1 Region 1 + S7 Region 1, TASKS.md T17) — one row per
  // binary with a live status token, shared between the full onboarding
  // wizard and Settings' "Engine & health" section. `onDownload` is only
  // passed by S1 (S7 never re-triggers an in-app fetch — its own missing-
  // binary case is handled by GlobalBanner's Fix → reopens S1 per UX.md S7
  // states); when absent this renders the compact S7 form (path/version +
  // Change…) instead of S1's found/not-found + Download-for-me/Set-path
  // choice.
  import type { BinaryStatus } from "../types";

  let {
    label,
    status,
    onSetPath,
    onDownload,
    downloadState,
  }: {
    label: string;
    status: BinaryStatus | null | undefined;
    onSetPath: () => void;
    onDownload?: () => void;
    downloadState?: { active: boolean; percent: number; error: string | null; stderr: string | null };
  } = $props();

  let showStderr = $state(false);
</script>

<div class="binary-row">
  <div class="head">
    <span class="label">{label}</span>
    {#if !status}
      <span class="status-token loading" role="status">
        <span class="spinner" aria-hidden="true"></span>
        Checking…
      </span>
    {:else if status.found}
      <span class="status-token ok">
        <span class="glyph" aria-hidden="true">✓</span>
        found — {status.path}{status.version ? ` (${status.version})` : ""}
      </span>
      <button type="button" class="ghost" onclick={onSetPath}>Change…</button>
    {:else if downloadState?.active}
      <span class="status-token downloading">
        <span class="glyph" aria-hidden="true">↓</span>
        downloading…
      </span>
    {:else}
      <span class="status-token missing">
        <span class="glyph" aria-hidden="true">✗</span>
        not found
      </span>
    {/if}
  </div>

  {#if status && !status.found}
    {#if downloadState?.active}
      <div class="progress-row">
        <div class="bar-track">
          <div class="bar-fill" style:width="{Math.min(100, Math.max(0, downloadState.percent))}%"></div>
        </div>
        <span class="figures mono">{downloadState.percent.toFixed(0)}%</span>
      </div>
    {:else}
      <div class="resolve-actions">
        {#if onDownload}
          <button type="button" onclick={onDownload}>Download for me</button>
        {/if}
        <button type="button" onclick={onSetPath}>Set path…</button>
      </div>
    {/if}

    {#if downloadState?.error}
      <div class="download-error">
        <p class="error">
          Couldn't download {label}: {downloadState.error}. <button type="button" class="link-btn" onclick={onDownload}>Retry</button>, or set a path.
        </p>
        {#if downloadState.stderr}
          <button type="button" class="link-btn" onclick={() => (showStderr = !showStderr)}>
            {showStderr ? "Hide" : "Show"} details
          </button>
          {#if showStderr}
            <pre class="stderr">{downloadState.stderr}</pre>
          {/if}
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .binary-row {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.6rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--card);
    color: var(--card-foreground);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }
  .label {
    width: 4.5rem;
    font-weight: 700;
  }
  .status-token {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-family: var(--font-mono);
    font-size: 0.85em;
  }
  .status-token.ok {
    color: var(--primary);
  }
  .status-token.missing {
    color: var(--warning);
  }
  .status-token.downloading {
    color: var(--primary);
  }
  .status-token.loading {
    color: var(--muted-foreground);
  }
  .glyph {
    width: 1em;
    text-align: center;
  }
  .spinner {
    width: 0.8em;
    height: 0.8em;
    border-radius: 50%;
    border: 2px solid var(--muted-foreground);
    border-top-color: transparent;
    animation: spin 800ms linear infinite;
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .resolve-actions {
    display: flex;
    gap: 0.5rem;
    margin-inline-start: 5.1rem;
  }
  .progress-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-inline-start: 5.1rem;
  }
  .bar-track {
    flex: 1;
    height: 8px;
    border-radius: 999px;
    background: var(--muted);
    overflow: hidden;
  }
  .bar-fill {
    height: 100%;
    background: var(--primary);
    border-radius: 999px;
    transition: width 200ms linear;
  }
  .figures {
    font-size: 0.78em;
    color: var(--muted-foreground);
    flex-shrink: 0;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .download-error {
    margin-inline-start: 5.1rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .error {
    color: var(--error-token);
    margin: 0;
    font-size: 0.85em;
  }
  .stderr {
    margin: 0;
    color: var(--error-token);
    background: var(--muted);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.5rem;
    font-family: var(--font-mono);
    font-size: 0.8em;
    white-space: pre-wrap;
    max-height: 6rem;
    overflow-y: auto;
  }
  button {
    background: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.35rem 0.6rem;
    font-family: var(--font-sans);
    font-size: 0.85em;
    cursor: pointer;
  }
  button.ghost {
    background: transparent;
  }
  button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .link-btn {
    background: none;
    border: none;
    color: var(--primary);
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
    font: inherit;
    font-size: 0.85em;
  }
</style>
