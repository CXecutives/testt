<!--
  Two or three equal-width options with an optional counter each. A white thumb slides
  under the chosen option (translateX by whole option widths, 180 ms, emphasized); the
  chosen label is deep navy and its count a soft navy pill, the others stay muted with a
  plain count (same box, so nothing moves). An unchosen option washes on hover; a press
  scales its label a little (not the box, so the thumb never jitters). Counts roll when
  they change.
-->
<script lang="ts" module>
  export interface SegmentedOption<Id extends string = string> {
    id: Id;
    label: string;
    count?: number | null;
  }
</script>

<script lang="ts" generics="Id extends string">
  import { cssVars } from '$lib/actions/cssVars';
  import { settled } from '$lib/motion/settled.svelte';
  import Count from './Count.svelte';

  interface Props {
    options: readonly SegmentedOption<Id>[];
    value: Id;
    label: string;
    size?: 'sm' | 'md';
    testid?: string | null;
    onchange: (id: Id) => void;
  }

  let { options, value, label, size = 'md', testid = null, onchange }: Props = $props();

  const motion = settled();

  const index = $derived(
    Math.max(
      0,
      options.findIndex((option) => option.id === value),
    ),
  );
</script>

<div
  class="segmented {size}"
  class:ready={motion.ready}
  role="radiogroup"
  aria-label={label}
  data-testid={testid ?? undefined}
  use:cssVars={{ count: options.length, index }}
>
  <span class="thumb" aria-hidden="true"></span>
  {#each options as option (option.id)}
    {@const chosen = option.id === value}
    <button
      type="button"
      role="radio"
      class="option"
      aria-checked={chosen}
      onclick={() => onchange(option.id)}
    >
      <span class="label">{option.label}</span>
      {#if option.count !== undefined && option.count !== null}
        <Count value={option.count} tone={chosen ? 'soft' : 'plain'} />
      {/if}
    </button>
  {/each}
</div>

<style>
  .segmented {
    position: relative;
    display: inline-grid;
    grid-auto-columns: 1fr;
    grid-auto-flow: column;
    height: var(--seg-height);
    padding: var(--space-2);
    border-radius: var(--radius-control);
    background-color: var(--surface-track);
    isolation: isolate;
  }

  .thumb {
    position: absolute;
    z-index: var(--z-below);
    top: var(--space-2);
    bottom: var(--space-2);
    left: var(--space-2);
    width: calc((100% - 2 * var(--space-2)) / var(--count));
    border-radius: var(--radius-sm);
    background-color: var(--surface);
    box-shadow: var(--sh-thumb);
    transform: translateX(calc(var(--index) * 100%));
    will-change: transform;
  }

  /* It slides only once the control has been drawn (never when it mounts). */
  .ready .thumb {
    transition: transform var(--dur-slow) var(--ease-emphasized);
  }

  .option {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
    padding: 0 var(--space-12);
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    font: var(--seg-type);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    transition: color var(--dur-base) var(--ease-standard);
  }

  /* The hover wash of an unchosen option, inside its own box (the thumb stays below). */
  .option::before {
    position: absolute;
    z-index: var(--z-below);
    inset: 0;
    border-radius: inherit;
    background-color: var(--surface-hover);
    content: '';
    opacity: 0;
    transition: opacity var(--dur-base) var(--ease-standard);
  }

  .option[aria-checked='false']:hover {
    color: var(--text);
    transition-duration: var(--dur-hover);
  }

  .option[aria-checked='false']:hover::before {
    opacity: 1;
    transition-duration: var(--dur-hover);
  }

  .option[aria-checked='true'] {
    color: var(--nav-active-fg);
  }

  :global(:root[data-window='inactive']) .option[aria-checked='true'] {
    color: var(--text);
  }

  .label {
    display: inline-block;
    transition: transform var(--dur-base) var(--ease-emphasized);
  }

  .option:active .label {
    transform: scale(var(--scale-press));
    transition-duration: var(--dur-instant);
  }

  .option:focus-visible {
    box-shadow: var(--focus-ring-inset);
  }

  .sm {
    --seg-height: var(--control-sm);
    --seg-type: var(--type-sm);
  }

  .md {
    --seg-height: var(--control-md);
    --seg-type: var(--type-md);
  }
</style>
