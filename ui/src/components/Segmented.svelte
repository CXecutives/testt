<!--
  Two or three equal-width options with an optional counter each. A white thumb slides
  under the chosen option (translateX by whole option widths).
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
  import { formatNumber } from '$lib/i18n/format';

  interface Props {
    options: readonly SegmentedOption<Id>[];
    value: Id;
    label: string;
    size?: 'sm' | 'md';
    testid?: string | null;
    onchange: (id: Id) => void;
  }

  let { options, value, label, size = 'md', testid = null, onchange }: Props = $props();

  const index = $derived(
    Math.max(
      0,
      options.findIndex((option) => option.id === value),
    ),
  );
</script>

<div
  class="segmented {size}"
  role="radiogroup"
  aria-label={label}
  data-testid={testid ?? undefined}
  use:cssVars={{ count: options.length, index }}
>
  <span class="thumb" aria-hidden="true"></span>
  {#each options as option (option.id)}
    <button
      type="button"
      role="radio"
      class="option"
      aria-checked={option.id === value}
      onclick={() => onchange(option.id)}
    >
      <span class="label">{option.label}</span>
      {#if option.count !== undefined && option.count !== null}
        <span class="count">{formatNumber(option.count)}</span>
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
    transition: transform var(--dur-base) var(--ease-out);
  }

  .option {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-6);
    padding: 0 var(--space-12);
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    font: var(--seg-type);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    transition: color var(--dur-fast) var(--ease-standard);
  }

  .option:hover,
  .option[aria-checked='true'] {
    color: var(--text);
  }

  .option:focus-visible {
    box-shadow: var(--focus-ring-inset);
  }

  .count {
    color: var(--text-muted);
    font-variant-numeric: var(--numeric);
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
