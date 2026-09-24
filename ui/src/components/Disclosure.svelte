<!--
  A heading that opens a section. One chevron turns half a turn when open (180 ms,
  emphasized; the angle stays under reduced motion). The content fades in (100 ms) when the
  user opens it and is gone at once when closed: no height animation, which would lay out
  the page in every frame. On hover the head's text darkens and its chevron turns navy (no
  background); like a row it runs edge to edge in its container and pads its label by
  --row-inset.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fade } from '$lib/motion/transitions';
  import Icon from './Icon.svelte';

  interface Props {
    label: string;
    open?: boolean;
    testid?: string | null;
    children: Snippet;
  }

  let { label, open = $bindable(false), testid = null, children }: Props = $props();
  const id = $props.id();
</script>

<div class="disclosure" class:open data-testid={testid ?? undefined}>
  <button
    type="button"
    class="head"
    aria-expanded={open}
    aria-controls="{id}-panel"
    onclick={() => (open = !open)}
  >
    <span class="label">{label}</span>
    <span class="chevron"><Icon name="chevron-down" size="sm" /></span>
  </button>
  <div class="panel" id="{id}-panel" role="region">
    {#if open}
      <div class="inner" in:fade>{@render children()}</div>
    {/if}
  </div>
</div>

<style>
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-12);
    width: calc(100% + 2 * var(--row-inset));
    min-height: var(--control-sm);
    margin-inline: calc(-1 * var(--row-inset));
    padding-inline: var(--row-inset);
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
    transition: color var(--dur-base) var(--ease-standard);
  }

  .head:hover {
    color: var(--text);
    transition-duration: var(--dur-hover);
  }

  .head:focus-visible {
    box-shadow: var(--focus-ring-inset);
  }

  .chevron {
    display: inline-flex;
    color: var(--text-muted);
    transition:
      transform var(--dur-slow) var(--ease-emphasized),
      color var(--dur-base) var(--ease-standard);
  }

  .head:hover .chevron {
    color: var(--icon-accent);
  }

  .open .chevron {
    transform: rotate(var(--turn-half));
  }

  .inner {
    padding-top: var(--space-8);
  }
</style>
