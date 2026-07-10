<script lang="ts" generics="T">
  // Gap list #1 (DESIGN.md §4): no shadcn virtualizer, so a small hand-rolled
  // fixed-row-height windower — used by S4's format table (T10) and S2's
  // queue rows (T14). Renders only the rows in the scrolled viewport (+ a
  // buffer above/below) inside a full-height spacer so native scrollbar
  // sizing/position stays correct without measuring every row.
  import type { Snippet } from "svelte";

  let {
    items,
    itemHeight,
    height,
    buffer = 6,
    row,
  }: {
    items: T[];
    itemHeight: number;
    height: number;
    buffer?: number;
    row: Snippet<[T, number]>;
  } = $props();

  let scrollTop = $state(0);

  let totalHeight = $derived(items.length * itemHeight);
  let visibleCount = $derived(Math.ceil(height / itemHeight));
  let startIndex = $derived(Math.max(0, Math.floor(scrollTop / itemHeight) - buffer));
  let endIndex = $derived(Math.min(items.length, startIndex + visibleCount + buffer * 2));
  let visibleItems = $derived(items.slice(startIndex, endIndex));
  let offsetY = $derived(startIndex * itemHeight);

  function onScroll(e: Event) {
    scrollTop = (e.currentTarget as HTMLDivElement).scrollTop;
  }
</script>

<div class="relative overflow-y-auto" style:height="{height}px" onscroll={onScroll}>
  <div class="relative w-full" style:height="{totalHeight}px">
    <div class="absolute start-0 top-0 w-full" style:transform="translateY({offsetY}px)">
      {#each visibleItems as item, i (startIndex + i)}
        <div class="box-border w-full" style:height="{itemHeight}px">
          {@render row(item, startIndex + i)}
        </div>
      {/each}
    </div>
  </div>
</div>
