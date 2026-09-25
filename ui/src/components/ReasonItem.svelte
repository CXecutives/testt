<!--
  One reason of a match: met | partial | open | violation | check, weighted must | nice |
  hard | info. Quote and profile evidence appear in the tooltip, or as a quiet line under
  the words (`detail`), inside the reason so its wash and its click cover it too; hovering
  can highlight the passage (onhover), a click can scroll to it (onselect). A reason that
  jumps washes on hover and shows a small arrow down, darkens while pressed, and takes the
  navy wash while its passage is pinned (active).
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
    // To check: the question mark of the criteria chip, in info navy everywhere.
    check: 'circle-help',
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
  import { t } from '$lib/i18n/t';
  import Badge from './Badge.svelte';
  import Icon from './Icon.svelte';

  interface Props {
    kind: ReasonKind;
    label: string;
    weight?: ReasonWeight | null;
    /** Tooltip: the ad's words and the profile evidence (texts.ts reasonHint). */
    hint?: string | null;
    /** A quiet line under the words (the evidence), part of the reason (not in compact). */
    detail?: string | null;
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
    detail = null,
    compact = false,
    active = false,
    onhover = null,
    onselect = null,
  }: Props = $props();
</script>

{#snippet words()}
  <span class="label">{label}</span>
  {#if weight && !compact}<Badge label={t.reason.weight[weight]} tone={WEIGHT_TONE[weight]} />{/if}
{/snippet}

{#snippet body()}
  <span class="icon" role="img" aria-label={t.reason.kind[kind]}
    ><Icon name={ICON[kind]} size="sm" /></span
  >
  {#if detail && !compact}
    <span class="words">
      <span class="head">{@render words()}</span>
      <span class="detail" data-testid="evidence">{detail}</span>
    </span>
  {:else}
    {@render words()}
  {/if}
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
    color: var(--text);
    font: var(--type-md);
    text-align: left;
    transition: background-color var(--dur-base) var(--ease-standard);
  }

  /* A reason that jumps is a row: edge to edge in its list, content padded by the list's
     --row-inset (without one, a small inset of its own). */
  button.reason {
    width: calc(100% + 2 * var(--row-inset));
    margin-inline: calc(-1 * var(--row-inset));
    padding-inline: max(var(--row-inset), var(--space-8));
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

  .face {
    display: flex;
    flex: 1;
    align-items: inherit;
    gap: var(--space-8);
    min-width: 0;
  }

  /* The way to the passage: a small arrow that appears on hover (100 ms). */
  .jump {
    display: inline-flex;
    flex: none;
    align-self: center;
    color: var(--text-subtle);
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }

  button.reason:hover .jump,
  button.reason:focus-visible .jump {
    opacity: 1;
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

  /* With a detail: the words and, under them, the evidence (on the axis of the words). */
  .words {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  .head {
    display: flex;
    align-items: flex-start;
    gap: var(--space-8);
    min-width: 0;
  }

  .detail {
    color: var(--text-subtle);
    font: var(--type-sm);
    overflow-wrap: break-word;
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
