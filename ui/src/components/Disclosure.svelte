<!--
  A heading that opens a section. One chevron turns half a turn when open (180 ms,
  emphasized; the angle stays under reduced motion). The content fades in (100 ms) when the
  user opens it and is gone at once when closed: no height animation, which would lay out
  the page in every frame. The head washes on hover and its chevron turns navy.
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
    /* The label lines up with the text around it; the hover wash reaches a little out. */
    width: calc(100% + 2 * var(--space-8));
    min-height: var(--control-sm);
    margin: 0 calc(-1 * var(--space-8));
    padding: 0 var(--space-8);
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
    transition:
      background-color var(--dur-base) var(--ease-standard),
      color var(--dur-base) var(--ease-standard);
  }

  .head:hover {
    background-color: var(--surface-hover);
    color: var(--text);
    transition-duration: var(--dur-hover);
  }

  .head:active {
    background-color: var(--surface-press);
    transition-duration: var(--dur-instant);
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
    color: var(--nav-active-icon);
  }

  .open .chevron {
    transform: rotate(var(--turn-half));
  }

  .inner {
    padding-top: var(--space-8);
  }
</style>
