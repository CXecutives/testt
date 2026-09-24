<!--
  A number with a label; counts up once when first visible; optionally clickable (a filter).
  Calm: a white tile with a hairline, the icon small in the label line, coloured only when
  the tone means something. A clickable tile is a filter and looks like one: hover lifts it
  1 px with a soft shadow, the active tile (its filter is on) takes an ink edge on a muted
  surface (coral stays for the few accents).
-->
<script lang="ts">
  import { reveal } from '$lib/actions/reveal';
  import { formatNumber } from '$lib/i18n/format';
  import { countUp } from '$lib/motion/transitions';
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

  // Short count (--dur-slow); only the score ring takes the longer reveal.
  const number = countUp(0, 'slow');
  let revealed = $state(false);

  $effect(() => {
    if (revealed) number.target = value;
  });
</script>

{#snippet body()}
  <span class="head">
    {#if icon}<span class="icon {tone}"><Icon name={icon} size="sm" /></span>{/if}
    <span class="label">{label}</span>
  </span>
  <span class="value">{formatNumber(Math.round(number.current))}</span>
  {#if hint}<span class="hint">{hint}</span>{/if}
{/snippet}

{#if onclick}
  <button
    type="button"
    class="tile clickable"
    class:active
    aria-pressed={active}
    data-testid={testid ?? undefined}
    use:reveal={() => (revealed = true)}
    onclick={() => onclick?.()}
  >
    {@render body()}
  </button>
{:else}
  <div class="tile" data-testid={testid ?? undefined} use:reveal={() => (revealed = true)}>
    {@render body()}
  </div>
{/if}

<style>
  .tile {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
    position: relative;
    isolation: isolate;
    padding: var(--space-12) var(--space-16);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-card);
    background-color: var(--surface);
    text-align: left;
  }

  .clickable {
    transition:
      transform var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      background-color var(--dur-fast) var(--ease-standard);
  }

  /* The hover shadow fades in on ::after (box-shadow itself never animates). */
  .clickable::after {
    position: absolute;
    z-index: var(--z-below);
    inset: calc(-1 * var(--border-width));
    border-radius: inherit;
    box-shadow: var(--sh-sm);
    content: '';
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }

  .clickable:hover {
    border-color: var(--border-strong);
    transform: translateY(var(--lift));
  }

  .clickable:hover::after {
    opacity: 1;
  }

  .clickable:active {
    transform: translateY(0) scale(var(--scale-press));
    transition-duration: var(--dur-instant);
  }

  .clickable:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .active,
  .active:hover {
    border-color: var(--text);
    background-color: var(--surface-muted);
  }

  .head {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    min-width: 0;
  }

  .icon {
    display: inline-flex;
    color: var(--text-subtle);
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

  .value {
    color: var(--text-heading);
    font: var(--type-xl);
    font-variant-numeric: var(--numeric);
    letter-spacing: var(--tracking-tight);
  }

  .hint {
    color: var(--text-muted);
    font: var(--type-xs);
  }
</style>
