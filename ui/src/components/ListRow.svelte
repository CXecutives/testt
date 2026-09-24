<!--
  A list row: leading, content, trailing, top-aligned, all rows of one fixed height (mail
  style: three lines of content). The one inner padding of the columns on the sides. Hover
  tints the row (100 ms); the selected row keeps the selection wash and a coral bar on the
  left. No other decoration.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    selected?: boolean;
    /** Greyed out (excluded jobs behind the divider). */
    muted?: boolean;
    onclick?: (() => void) | null;
    testid?: string | null;
    leading?: Snippet | null;
    trailing?: Snippet | null;
    children: Snippet;
  }

  let {
    selected = false,
    muted = false,
    onclick = null,
    testid = null,
    leading = null,
    trailing = null,
    children,
  }: Props = $props();
</script>

<button
  type="button"
  class="row"
  class:selected
  class:muted
  aria-current={selected ? 'true' : undefined}
  data-testid={testid ?? undefined}
  onclick={() => onclick?.()}
>
  {#if leading}<span class="leading">{@render leading()}</span>{/if}
  <span class="content">{@render children()}</span>
  {#if trailing}<span class="trailing">{@render trailing()}</span>{/if}
</button>

<style>
  .row {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: var(--space-12);
    width: 100%;
    height: var(--row-height);
    overflow: hidden;
    padding: var(--space-12) var(--pane-padding) calc(var(--space-12) - var(--border-width));
    border-bottom: var(--border-width) solid var(--border);
    background-color: transparent;
    text-align: left;
    transition: background-color var(--dur-fast) var(--ease-standard);
  }

  .row:hover {
    background-color: var(--surface-hover);
  }

  .selected,
  .selected:hover {
    background-color: var(--surface-selected);

    /* The ring's track would vanish on the selection wash. */
    --ring-track: var(--border-strong);
  }

  /* The selection bar on the left edge. */
  .selected::before {
    position: absolute;
    top: var(--space-12);
    bottom: var(--space-12);
    left: 0;
    width: var(--row-bar);
    border-radius: var(--radius-full);
    background-color: var(--accent);
    content: '';
  }

  .row:focus-visible {
    box-shadow: var(--focus-ring-inset);
  }

  .muted {
    opacity: var(--opacity-muted);
  }

  .leading,
  .trailing {
    display: flex;
    flex: none;
  }

  .trailing {
    flex-direction: column;
    align-items: flex-end;
    gap: var(--space-4);
  }

  .content {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
</style>
