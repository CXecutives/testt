<!--
  One reason of a match: met | partial | open | violation | check, weighted must | nice |
  hard | info. Quote and profile evidence appear in the tooltip; hovering can highlight
  the passage (onhover), a click can scroll to it (onselect). A reason that jumps washes
  on hover and shows a small arrow down, gives a little when pressed, and takes the navy
  wash while its passage is pinned (active).
-->
<script lang="ts" module>
  import type { ReasonKind, ReasonWeight } from '$lib/ipc/types';
  import type { BadgeTone } from './Badge.svelte';
  import type { IconName } from './Icon.svelte';

  export const REASON_KINDS: readonly ReasonKind[] = [
    'met',
    'partial',
    'open',
    'violation',
    'check',
  ];
  export const REASON_WEIGHTS: readonly ReasonWeight[] = ['must', 'nice', 'hard'];

  const ICON: Record<ReasonKind, IconName> = {
    met: 'check',
    partial: 'circle-half',
    open: 'circle-dashed',
    violation: 'ban',
    check: 'info',
  };

  // Muss and Kann are plain facts, never alarms: both neutral. Only a decided exclusion is red.
  const WEIGHT_TONE: Record<ReasonWeight, BadgeTone> = {
    must: 'neutral',
    nice: 'neutral',
    hard: 'danger',
    info: 'neutral',
  };
</script>

<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { de } from '$lib/i18n/de';
  import Badge from './Badge.svelte';
  import Icon from './Icon.svelte';

  interface Props {
    kind: ReasonKind;
    label: string;
    weight?: ReasonWeight | null;
    /** Tooltip: the ad's words and the profile evidence (texts.ts reasonHint). */
    hint?: string | null;
    /** One line without weight badge (list rows). */
    compact?: boolean;
    active?: boolean;
    onhover?: ((on: boolean) => void) | null;
    onselect?: (() => void) | null;
  }

  let {
    kind,
    label,
    weight = null,
    hint = null,
    compact = false,
    active = false,
    onhover = null,
    onselect = null,
  }: Props = $props();
</script>

{#snippet body()}
  <span class="icon" role="img" aria-label={de.reason.kind[kind]}
    ><Icon name={ICON[kind]} size="sm" /></span
  >
  <span class="label">{label}</span>
  {#if weight && !compact}<Badge label={de.reason.weight[weight]} tone={WEIGHT_TONE[weight]} />{/if}
{/snippet}

{#if onselect}
  <button
    type="button"
    class="reason {kind}"
    class:active
    use:tooltip={hint}
    onpointerenter={() => onhover?.(true)}
    onpointerleave={() => onhover?.(false)}
    onclick={() => onselect?.()}
  >
    <span class="face">{@render body()}</span>
    <span class="jump" aria-hidden="true"><Icon name="arrow-down" size="xs" /></span>
  </button>
{:else}
  <span class="reason {kind}" class:compact use:tooltip={hint}>{@render body()}</span>
{/if}

<style>
  .reason {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
    padding: var(--space-6) var(--space-8);
    border-radius: var(--radius-sm);
    color: var(--text);
    font: var(--type-md);
    text-align: left;
    transition: background-color var(--dur-base) var(--ease-standard);
  }

  button.reason {
    width: 100%;
  }

  button.reason:hover {
    background-color: var(--surface-hover);
    transition-duration: var(--dur-hover);
  }

  button.reason:active {
    background-color: var(--surface-press);
    transition-duration: var(--dur-instant);
  }

  /* The pinned passage: the navy wash of a chosen filter. */
  .active,
  .active:hover {
    background-color: var(--active-surface);
  }

  button.reason:focus-visible {
    box-shadow: var(--focus-ring-inset);
  }

  /* The content of a reason that jumps: it gives a little under the pointer (60 ms). */
  .face {
    display: flex;
    flex: 1;
    align-items: inherit;
    gap: var(--space-8);
    min-width: 0;
    transition: transform var(--dur-base) var(--ease-emphasized);
  }

  button.reason:active .face {
    transform: scale(var(--scale-press));
    transition-duration: var(--dur-instant);
  }

  /* The way to the passage: a small arrow that drops in on hover (100 ms). */
  .jump {
    display: inline-flex;
    flex: none;
    align-self: center;
    color: var(--text-subtle);
    opacity: 0;
    transform: translateY(calc(-1 * var(--move-sm)));
    transition:
      opacity var(--dur-fast) var(--ease-standard),
      transform var(--dur-fast) var(--ease-out);
  }

  button.reason:hover .jump,
  button.reason:focus-visible .jump {
    opacity: 1;
    transform: none;
  }

  .compact {
    gap: var(--space-4);
    padding: 0;
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .icon {
    display: inline-flex;
    flex: none;
    color: var(--reason-color);
  }

  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Outside list rows a reason wraps instead of losing its end. */
  .reason:not(.compact) {
    align-items: flex-start;
  }

  /* The weight badge follows the words instead of standing at the far end. */
  .reason:not(.compact) .label {
    flex: 0 1 auto;
    white-space: normal;
    overflow-wrap: break-word;
  }

  .reason:not(.compact) .icon {
    margin-top: var(--space-2);
  }

  .met {
    --reason-color: var(--success-strong);
  }

  .partial {
    --reason-color: var(--warning-strong);
  }

  .open {
    --reason-color: var(--text-subtle);
  }

  .violation {
    --reason-color: var(--danger-strong);
  }

  .check {
    --reason-color: var(--info);
  }
</style>
