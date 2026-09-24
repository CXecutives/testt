<!--
  A 76 px list row: leading, content, trailing. Hover tints the row and grows a gradient
  bar on the left (scaleY); the selected row keeps both.
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
    align-items: center;
    gap: var(--space-12);
    width: 100%;
    height: var(--row-height);
    padding: 0 var(--space-16);
    border-bottom: var(--border-width) solid var(--border);
    background-color: transparent;
    text-align: left;
    transition: background-color var(--dur-fast) var(--ease-standard);
  }

  .row::before {
    position: absolute;
    top: var(--space-12);
    bottom: var(--space-12);
    left: 0;
    width: var(--row-bar);
    border-radius: var(--radius-full);
    background: var(--grad-bar);
    content: '';
    transform: scaleY(0);
    transition: transform var(--dur-base) var(--ease-out);
  }

  .row:hover {
    background-color: var(--surface-hover);
  }

  .row:hover::before,
  .selected::before {
    transform: scaleY(1);
  }

  .selected,
  .selected:hover {
    background-color: var(--surface-selected);
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
    align-items: center;
  }

  .trailing {
    flex-direction: column;
    align-items: flex-end;
    align-self: stretch;
    justify-content: center;
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
