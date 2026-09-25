<!--
  A number with a label; optionally clickable (a filter). Calm at rest: a white tile with a
  hairline, the icon small in the label line (navy for a neutral tile, the tone's colour
  otherwise), the value in ink. A clickable tile answers on hover with a navy hairline and a
  soft shadow that fades in (painted once on ::after, never animated as a shadow, no lift),
  and darkens a step while pressed (it never moves). The active tile (its filter is on)
  takes the navy trio of a chosen filter. A value of 0 is quiet (subtle) and not a filter:
  the tile is static then. The number rolls when it changes on screen, not when the view
  comes back.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { formatNumber } from '$lib/i18n/format';
  import { settled } from '$lib/motion/settled.svelte';
  import { roll } from '$lib/motion/transitions';
  import Icon, { type IconName } from './Icon.svelte';
  import type { TileTone } from './IconTile.svelte';

  interface Props {
    label: string;
    value: number;
    tone?: TileTone;
    icon?: IconName | null;
    hint?: string | null;
    /** The filter of this tile is on. */
    active?: boolean;
    onclick?: (() => void) | null;
    testid?: string | null;
  }

  let {
    label,
    value,
    tone = 'neutral',
    icon = null,
    hint = null,
    active = false,
    onclick = null,
    testid = null,
  }: Props = $props();

  // An empty tile filters nothing (an active one stays a button, so it can be cleared).
  const clickable = $derived(onclick !== null && (value !== 0 || active));

  // A number that arrives while the view is still being built (a load right after it
  // mounts) is simply there; only a change on a drawn screen rolls.
  const motion = settled();
  let previous = untrack(() => value);
  let up = $state(true);

  $effect.pre(() => {
    const next = value;
    untrack(() => {
      up = next >= previous;
      previous = next;
    });
  });
</script>

{#snippet body()}
  <span class="head">
    {#if icon}<span class="icon {tone}"><Icon name={icon} size="sm" /></span>{/if}
    <span class="label">{label}</span>
  </span>
  <span class="value" class:zero={value === 0}>
    {#key value}<span class="digits" in:roll={{ up, on: motion.ready }}>{formatNumber(value)}</span
      >{/key}
  </span>
  {#if hint}<span class="hint">{hint}</span>{/if}
{/snippet}

{#if clickable}
  <button
    type="button"
    class="tile clickable"
    class:active
    aria-pressed={active}
    data-testid={testid ?? undefined}
    onclick={() => onclick?.()}
  >
    {@render body()}
  </button>
{:else}
  <div class="tile" data-testid={testid ?? undefined}>
    {@render body()}
  </div>
{/if}

<style>
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
    padding: var(--space-12);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-card);
    background-color: var(--surface);
    text-align: left;
  }

  .clickable {
    transition:
      border-color var(--dur-base) var(--ease-standard),
      background-color var(--dur-base) var(--ease-standard),
      color var(--dur-base) var(--ease-standard);
  }

  /* The hover shadow, painted once and shown by opacity. */
  .clickable::after {
    position: absolute;
    inset: calc(-1 * var(--border-width));
    border-radius: inherit;
    box-shadow: var(--sh-hover);
    content: '';
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-base) var(--ease-standard);
  }

  .clickable:hover {
    border-color: var(--border-navy);
    transition-duration: var(--dur-hover);
  }

  .clickable:hover::after {
    opacity: 1;
    transition-duration: var(--dur-hover);
  }

  .clickable:active:hover {
    background-color: var(--surface-muted);
    transition-duration: var(--dur-instant);
  }

  .clickable:active:hover::after {
    opacity: 0;
    transition-duration: var(--dur-instant);
  }

  .clickable:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .active,
  .active:hover,
  .active:active:hover {
    border-color: var(--active-edge);
    background-color: var(--active-surface);
  }

  .head {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    min-width: 0;
  }

  .icon {
    display: inline-flex;
    color: var(--icon-accent);
  }

  .icon.success {
    color: var(--success-strong);
  }

  .icon.warning {
    color: var(--warning-strong);
  }

  .icon.danger {
    color: var(--danger-strong);
  }

  .icon.coral {
    color: var(--accent-text);
  }

  .label {
    overflow: hidden;
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .active .label,
  .active .value {
    color: var(--active-text);
  }

  .value {
    overflow: hidden;
    color: var(--text-heading);
    font: var(--type-xl);
    font-weight: var(--weight-medium);
    font-variant-numeric: var(--numeric);
    letter-spacing: var(--tracking-tight);
  }

  .value.zero {
    color: var(--text-subtle);
  }

  .digits {
    display: inline-block;
  }

  .hint {
    color: var(--text-muted);
    font: var(--type-xs);
  }
</style>
