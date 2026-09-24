<!--
  The navigation of the sidebar: icon and label per view, an optional count (unread jobs).
  The active entry sits on one white pill that slides to it (180 ms, emphasized; the
  sibling of the segmented thumb), its label deep navy and its icon navy. An idle entry
  washes on hover and its icon turns navy. The count is the deep navy pill and rolls when
  it changes. Collapsed (icon rail) the labels move into tooltips and a coral dot on the
  icon stands for the count. While the window is inactive the active label turns ink.
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
  import { cssVars } from '$lib/actions/cssVars';
  import { settled } from '$lib/motion/settled.svelte';
  import { fade, pop } from '$lib/motion/transitions';
  import Count from './Count.svelte';
  import Icon from './Icon.svelte';

  interface Props {
    items: readonly SideNavItem<Id>[];
    active: Id;
    label: string;
    collapsed?: boolean;
    onselect: (id: Id) => void;
  }
  let { items, active, label, collapsed = false, onselect }: Props = $props();

  const index = $derived(items.findIndex((item) => item.id === active));
  const motion = settled();
</script>

<nav
  class="nav"
  class:collapsed
  class:ready={motion.ready}
  aria-label={label}
  use:cssVars={{ index: Math.max(0, index) }}
>
  <!-- Re-created when the rail flips, so crossing 1100 px places it without sliding. -->
  {#key collapsed}
    <span class="indicator" class:none={index < 0} aria-hidden="true"></span>
  {/key}
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
        {#if collapsed && item.count}<span class="dot" aria-hidden="true" in:pop></span>{/if}
      </span>
      {#if !collapsed}
        <span class="label" in:fade>{item.label}</span>
        {#if item.count}<span class="count" in:fade><Count value={item.count} tone="strong" /></span
          >{/if}
      {/if}
    </button>
  {/each}
</nav>

<style>
  .nav {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    isolation: isolate;
  }

  /* The one white pill behind the active entry. */
  .indicator {
    position: absolute;
    z-index: var(--z-below);
    top: 0;
    right: 0;
    left: 0;
    height: var(--control-md);
    border: var(--border-width) solid var(--nav-active-border);
    border-radius: var(--radius-md);
    background-color: var(--nav-active-bg);
    box-shadow: var(--sh-xs);
    transform: translateY(calc(var(--index) * var(--nav-step)));
    will-change: transform;
  }

  /* It slides only once the nav has been drawn (never when it mounts). */
  .ready .indicator {
    transition: transform var(--dur-slow) var(--ease-emphasized);
  }

  .indicator.none {
    opacity: 0;
  }

  .collapsed .indicator {
    right: auto;
    width: var(--control-lg);
    height: var(--control-lg);
    transform: translateY(calc(var(--index) * var(--nav-step-rail)));
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
      background-color var(--dur-base) var(--ease-standard),
      color var(--dur-base) var(--ease-standard);
  }

  .item:not([aria-current='page']):hover {
    background-color: var(--surface-hover);
    color: var(--text);
    transition-duration: var(--dur-hover);
    --nav-glyph: var(--nav-active-icon);
  }

  .item:not([aria-current='page']):active {
    background-color: var(--surface-press);
    transition-duration: var(--dur-instant);
  }

  .item[aria-current='page'] {
    color: var(--nav-active-fg);
    --nav-glyph: var(--nav-active-icon);
  }

  /* Like Mail and Explorer: the selection greys out while the window is in the back. */
  :global(:root[data-window='inactive']) .item[aria-current='page'] {
    color: var(--text);
    --nav-glyph: var(--text);
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
    color: var(--nav-glyph, currentcolor);
    transition: color var(--dur-base) var(--ease-standard);
  }

  .dot {
    position: absolute;
    top: calc(-1 * var(--space-2));
    right: calc(-1 * var(--space-2));
    width: var(--dot);
    height: var(--dot);
    border: var(--border-width) solid var(--bg);
    border-radius: var(--radius-full);
    background-color: var(--unread);
  }

  .label {
    flex: 1;
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    display: inline-flex;
  }
</style>
