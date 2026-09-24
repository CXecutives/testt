<!--
  The navigation of the sidebar: icon and label per view, an optional count (unread jobs).
  Active item in accent-soft with ink text, hover surface-hover, the count in muted text.
  Collapsed (icon rail) the labels move into tooltips and a dot on the icon stands for the count.
-->
<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  export interface SideNavItem<Id extends string = string> {
    id: Id;
    label: string;
    icon: IconName;
    count?: number | null;
    testid?: string;
  }
</script>

<script lang="ts" generics="Id extends string">
  import { tooltip } from '$lib/actions/tooltip';
  import { formatNumber } from '$lib/i18n/format';
  import { fade } from '$lib/motion/transitions';
  import Icon from './Icon.svelte';

  interface Props {
    items: readonly SideNavItem<Id>[];
    active: Id;
    label: string;
    collapsed?: boolean;
    onselect: (id: Id) => void;
  }
  let { items, active, label, collapsed = false, onselect }: Props = $props();
</script>

<nav class="nav" class:collapsed aria-label={label}>
  {#each items as item (item.id)}
    <button
      type="button"
      class="item"
      aria-current={item.id === active ? 'page' : undefined}
      aria-label={collapsed ? item.label : undefined}
      data-testid={item.testid}
      use:tooltip={collapsed ? item.label : null}
      onclick={() => onselect(item.id)}
    >
      <span class="glyph">
        <Icon name={item.icon} size="md" />
        {#if collapsed && item.count}<span class="dot" aria-hidden="true"></span>{/if}
      </span>
      {#if !collapsed}
        <span class="label" in:fade>{item.label}</span>
        {#if item.count}<span class="count" in:fade>{formatNumber(item.count)}</span>{/if}
      {/if}
    </button>
  {/each}
</nav>

<style>
  .nav {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .item {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    height: var(--control-md);
    padding: 0 var(--space-12);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font: var(--type-tab);
    transition:
      background-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }

  .item:hover {
    background-color: var(--surface-hover);
    color: var(--text);
  }

  .item[aria-current='page'] {
    background-color: var(--surface-selected);
    color: var(--text-heading);
    font-weight: var(--weight-medium);
  }

  .item:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .collapsed .item {
    justify-content: center;
    width: var(--control-lg);
    height: var(--control-lg);
    padding: 0;
  }

  .glyph {
    position: relative;
    display: inline-flex;
  }

  .dot {
    position: absolute;
    top: calc(-1 * var(--space-2));
    right: calc(-1 * var(--space-2));
    width: var(--dot);
    height: var(--dot);
    border: var(--border-width) solid var(--bg);
    border-radius: var(--radius-full);
    background-color: var(--accent);
  }

  .label {
    flex: 1;
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The unread count: muted text, no pill (coral stays for the accents). */
  .count {
    color: var(--text-muted);
    font: var(--type-xs);
    font-weight: var(--weight-medium);
    font-variant-numeric: var(--numeric);
  }
</style>
