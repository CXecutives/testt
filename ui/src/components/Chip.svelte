<!--
  A quiet pill for a fact with a state (the reader's criteria strip): an icon for the state,
  then the words, usually the ad's own value ("1.100 €/Tag", "ab sofort").
  - met: a green icon (only with the ad as evidence), unknown: the navy question mark of a
    point to check (info, the same colour as its reason and its passage), violated: red on
    the danger wash, unset: muted with a dash (the ad does not say), plain: a neutral fact.
  - With onselect it is a button: it washes on hover (80 ms in, 150 ms out), lights its
    passage while hovered (onhover) and jumps to it on a click; pressed it darkens and never
    moves. Without it the pill is plain text with its tooltip.
-->
<script lang="ts" module>
  export type ChipState = 'met' | 'violated' | 'unknown' | 'unset' | 'plain';
  export const CHIP_STATES: readonly ChipState[] = ['met', 'unknown', 'violated', 'unset', 'plain'];
</script>

<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import Icon, { type IconName } from './Icon.svelte';

  interface Props {
    label: string;
    state?: ChipState;
    icon: IconName;
    /** Tooltip (what the state means). */
    hint?: string | null;
    /** Its passage is marked in the text. */
    active?: boolean;
    onhover?: ((on: boolean) => void) | null;
    onselect?: (() => void) | null;
    testid?: string | null;
  }

  let {
    label,
    state = 'plain',
    icon,
    hint = null,
    active = false,
    onhover = null,
    onselect = null,
    testid = null,
  }: Props = $props();
</script>

{#snippet body()}
  <span class="chip-icon"><Icon name={icon} size="xs" /></span>
  <span class="chip-label">{label}</span>
{/snippet}

{#if onselect}
  <button
    type="button"
    class="chip {state}"
    class:active
    data-state={state}
    data-testid={testid ?? undefined}
    use:tooltip={hint}
    onpointerenter={() => onhover?.(true)}
    onpointerleave={() => onhover?.(false)}
    onclick={() => onselect?.()}
  >
    {@render body()}
  </button>
{:else}
  <span
    class="chip {state}"
    data-state={state}
    data-testid={testid ?? undefined}
    use:tooltip={hint}
  >
    {@render body()}
  </span>
{/if}

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    height: var(--badge-height);
    padding: 0 var(--space-8) 0 var(--space-6);
    border-radius: var(--radius-full);
    background-color: var(--surface-muted);
    color: var(--text-muted);
    font: var(--type-xs);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    transition: background-color var(--dur-base) var(--ease-standard);
  }

  button.chip:hover {
    background-color: var(--border);
    transition-duration: var(--dur-hover);
  }

  button.chip:active {
    background-color: var(--border-strong);
    transition-duration: var(--dur-instant);
  }

  button.chip:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .chip.active {
    background-color: var(--active-surface);
    color: var(--active-text);
  }

  .chip-icon {
    display: inline-flex;
    color: var(--chip-icon, var(--text-subtle));
  }

  .met {
    --chip-icon: var(--success-strong);
  }

  /* To check is info everywhere: chip, reason icon and the passage's underline. */
  .unknown {
    --chip-icon: var(--info);
  }

  .violated,
  button.violated:hover {
    --chip-icon: var(--danger-strong);

    background-color: var(--danger-soft);
    color: var(--danger-strong);
  }

  .unset {
    color: var(--text-subtle);
  }
</style>
