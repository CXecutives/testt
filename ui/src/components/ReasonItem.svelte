<!--
  One reason of a match: met | partial | open | violation | check, weighted must | nice |
  hard | info. The evidence from the profile appears in the tooltip; hovering can highlight
  the passage (onhover), a click can scroll to it (onselect).
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
    partial: 'circle-dashed',
    open: 'circle-dashed',
    violation: 'ban',
    check: 'info',
  };

  const WEIGHT_TONE: Record<ReasonWeight, BadgeTone> = {
    must: 'slate',
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
    evidence?: string | null;
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
    evidence = null,
    compact = false,
    active = false,
    onhover = null,
    onselect = null,
  }: Props = $props();

  const hint = $derived(evidence ? `${de.reason.evidence}: ${evidence}` : null);
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
    {@render body()}
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
    transition: background-color var(--dur-fast) var(--ease-standard);
  }

  button.reason {
    width: 100%;
  }

  button.reason:hover,
  .active {
    background-color: var(--surface-hover);
  }

  button.reason:focus-visible {
    box-shadow: var(--focus-ring-inset);
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
