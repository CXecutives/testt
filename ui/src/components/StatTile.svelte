<!--
  A number with a label; optionally clickable (a filter). Calm: a white tile with a
  hairline, the icon small in the label line, coloured only when the tone means something.
  The number is simply there (no count-up each time the view comes back). A clickable tile
  darkens its hairline on hover; the active tile (its filter is on) takes an ink edge on a
  muted surface (coral stays for the few accents).
-->
<script lang="ts">
  import { formatNumber } from '$lib/i18n/format';
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
</script>

{#snippet body()}
  <span class="head">
    {#if icon}<span class="icon {tone}"><Icon name={icon} size="sm" /></span>{/if}
    <span class="label">{label}</span>
  </span>
  <span class="value">{formatNumber(value)}</span>
  {#if hint}<span class="hint">{hint}</span>{/if}
{/snippet}

{#if onclick}
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
      border-color var(--dur-fast) var(--ease-standard),
      background-color var(--dur-fast) var(--ease-standard);
  }

  .clickable:hover {
    border-color: var(--border-strong);
  }

  .clickable:active {
    background-color: var(--surface-muted);
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
    font-weight: var(--weight-medium);
    font-variant-numeric: var(--numeric);
    letter-spacing: var(--tracking-tight);
  }

  .hint {
    color: var(--text-muted);
    font: var(--type-xs);
  }
</style>
