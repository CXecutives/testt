<!--
  A heading that opens a section. The chevron turns 180° (150 ms) and the content is there
  at once: no height animation, which would lay out the page in every frame.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
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
  <div class="panel" id="{id}-panel" role="region" inert={!open}>
    <div class="inner">{@render children()}</div>
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
    transition: background-color var(--dur-fast) var(--ease-standard);
  }

  .head:hover {
    background-color: var(--surface-hover);
    color: var(--text);
  }

  .head:focus-visible {
    box-shadow: var(--focus-ring-inset);
  }

  .chevron {
    display: inline-flex;
    color: var(--text-muted);
    transition: transform var(--dur-base) var(--ease-out);
  }

  .open .chevron {
    transform: rotate(180deg);
  }

  .panel {
    display: none;
  }

  .open .panel {
    display: block;
  }

  .open .inner {
    padding-top: var(--space-8);
  }
</style>
