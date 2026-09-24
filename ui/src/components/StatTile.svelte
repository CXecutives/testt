<!-- A number with a label; counts up once when first visible; optionally clickable. -->
<script lang="ts">
  import { reveal } from '$lib/actions/reveal';
  import { formatNumber } from '$lib/i18n/format';
  import { countUp } from '$lib/motion/transitions';
  import type { IconName } from './Icon.svelte';
  import IconTile, { type TileTone } from './IconTile.svelte';

  interface Props {
    label: string;
    value: number;
    tone?: TileTone;
    icon?: IconName | null;
    hint?: string | null;
    onclick?: (() => void) | null;
    testid?: string | null;
  }

  let {
    label,
    value,
    tone = 'coral',
    icon = null,
    hint = null,
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
    {#if icon}<IconTile {tone} {icon} size="sm" />{/if}
    <span class="label">{label}</span>
  </span>
  <span class="value">{formatNumber(Math.round(number.current))}</span>
  {#if hint}<span class="hint">{hint}</span>{/if}
{/snippet}

{#if onclick}
  <button
    type="button"
    class="tile clickable"
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
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    min-width: var(--stat-min);
    padding: var(--space-20);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-card);
    background-color: var(--surface);
    box-shadow: var(--sh-sm);
    text-align: left;
    isolation: isolate;
  }

  .clickable {
    transition:
      transform var(--dur-base) var(--ease-out),
      border-color var(--dur-base) var(--ease-standard);
  }

  .clickable::after {
    position: absolute;
    z-index: var(--z-below);
    inset: calc(-1 * var(--border-width));
    border-radius: inherit;
    box-shadow: var(--sh-card);
    content: '';
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-base) var(--ease-standard);
  }

  .clickable:hover {
    border-color: var(--border-accent);
    transform: translateY(var(--lift-card));
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

  .head {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }

  .label {
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .value {
    color: var(--text-heading);
    font: var(--type-2xl);
    font-variant-numeric: var(--numeric);
    letter-spacing: var(--tracking-tight);
  }

  .hint {
    color: var(--text-subtle);
    font: var(--type-xs);
  }
</style>
