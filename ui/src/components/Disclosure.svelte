<!--
  A heading that opens a section. The chevron turns 180°; the content grows via
  grid-template-rows 0fr -> 1fr (the one documented exception to "animate transform and
  opacity only", see stylelint.config.js).
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
    width: 100%;
    min-height: var(--control-md);
    padding: 0 var(--space-12);
    border-radius: var(--radius-control);
    color: var(--text);
    font: var(--type-md);
    font-weight: var(--weight-semibold);
    transition: background-color var(--dur-fast) var(--ease-standard);
  }

  .head:hover {
    background-color: var(--surface-hover);
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
    display: grid;
    grid-template-rows: 0fr;
    opacity: 0;
    transition:
      grid-template-rows var(--dur-base) var(--ease-out),
      opacity var(--dur-base) var(--ease-standard);
  }

  .open .panel {
    grid-template-rows: 1fr;
    opacity: 1;
  }

  .inner {
    min-height: 0;
    overflow: hidden;
    padding: 0 var(--space-12);
  }

  .open .inner {
    padding-bottom: var(--space-12);
  }
</style>
