<!--
  The ad text with the passages of the match marked; the passage of a hovered reason is
  tinted (80 ms in, 150 ms out), the one just jumped to flashes navy once. Built from text nodes and <mark>
  elements only (no HTML from the page ever reaches the DOM). Offsets are UTF-16, as the
  browser counts; overlapping passages keep the first one. The text selects and copies like
  a document (`data-copy`).
-->
<script lang="ts" module>
  import type { Highlight } from '$lib/ipc/types';

  export interface Segment {
    text: string;
    mark: Highlight | null;
  }

  export function segments(text: string, highlights: readonly Highlight[]): Segment[] {
    const sorted = [...highlights]
      .filter((h) => h.start < h.end && h.start >= 0 && h.end <= text.length)
      .sort((a, b) => a.start - b.start || b.end - a.end);
    const out: Segment[] = [];
    let at = 0;
    for (const mark of sorted) {
      if (mark.start < at) continue;
      if (mark.start > at) out.push({ text: text.slice(at, mark.start), mark: null });
      out.push({ text: text.slice(mark.start, mark.end), mark });
      at = mark.end;
    }
    if (at < text.length) out.push({ text: text.slice(at), mark: null });
    return out;
  }
</script>

<script lang="ts">
  interface Props {
    text: string;
    highlights: readonly Highlight[];
    /** Reason whose passages are lit (hover in the reasons). */
    active: string | null;
    /** Reason whose passages flash once (after a jump to them). */
    flash?: string | null;
    element?: HTMLElement | null;
  }
  let { text, highlights, active, flash = null, element = $bindable(null) }: Props = $props();

  const parts = $derived(segments(text, highlights));
</script>

<div class="text" bind:this={element} data-testid="ad-text" data-copy>
  {#each parts as part, index (index)}{#if part.mark}<mark
        class="mark {part.mark.kind}"
        class:active={active === part.mark.reason}
        class:flash={flash === part.mark.reason}
        data-reason={part.mark.reason}>{part.text}</mark
      >{:else}{part.text}{/if}{/each}
</div>

<style>
  .text {
    color: var(--text);
    font: var(--type-body);
    white-space: pre-line;
    overflow-wrap: anywhere;
  }

  .mark {
    border-radius: var(--radius-xs);
    background-color: transparent;
    color: inherit;
    text-decoration-line: underline;
    text-decoration-color: var(--border-strong);
    text-decoration-thickness: var(--focus-width);
    text-underline-offset: var(--space-4);
    transition: background-color var(--dur-base) var(--ease-standard);
  }

  .mark.active,
  .mark.flash {
    transition-duration: var(--dur-hover);
  }

  .met {
    text-decoration-color: var(--score-high-ring);
  }

  /* Met in part: amber, like its half circle; a point to check: navy, like its question
     mark (RD-06). */
  .partial {
    text-decoration-color: var(--warning);
  }

  .violation {
    text-decoration-color: var(--danger);
  }

  .check {
    text-decoration-color: var(--info);
  }

  .met.active {
    background-color: var(--score-high-surface);
  }

  .partial.active {
    background-color: var(--warning-soft);
  }

  .open.active {
    background-color: var(--surface-muted);
  }

  .violation.active {
    background-color: var(--danger-soft);
  }

  .check.active {
    background-color: var(--info-soft);
  }

  /* After a jump: the passage lights up in navy ("you are here") and settles. */
  .mark.flash {
    background-color: var(--border-navy);
  }
</style>
