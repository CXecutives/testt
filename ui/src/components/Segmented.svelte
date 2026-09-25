<!--
  Two to four options with an optional counter each, on one track. Each option is as wide as
  its label and count; the chosen one sits on its own white pill, so the pill always covers
  exactly its option whatever the labels, counts or window width (it cross-fades, 100 ms).
  When the track has less room than the options want, the labels shorten with an ellipsis
  (the counts stay); nothing ever overlaps. The chosen label is ink and its count a soft
  warm pill, the others stay muted with a plain count (same box, so nothing moves); an option
  may keep one tone whatever is chosen (the unread count stays warm). An
  unchosen option washes on hover and darkens while pressed. Counts roll when they change.
  Like native radio buttons the group is one Tab stop and the arrows, Home and End choose.
-->
<script lang="ts" module>
  export interface SegmentedOption<Id extends string = string> {
    id: Id;
    label: string;
    count?: number | null;
    /** The count's tone whatever is chosen (default: soft when chosen, plain otherwise). */
    tone?: 'soft' | 'plain' | null;
  }
</script>

<script lang="ts" generics="Id extends string">
  import { tooltip } from '$lib/actions/tooltip';
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

  /** One Tab stop: the chosen option (the arrows move between them, lib/input/input.ts). */
  const stop = $derived(options.some((option) => option.id === value) ? value : options[0]?.id);
</script>

<div
  class="segmented {size}"
  role="radiogroup"
  aria-label={label}
  data-testid={testid ?? undefined}
>
  {#each options as option (option.id)}
    {@const chosen = option.id === value}
    <button
      type="button"
      role="radio"
      class="option"
      aria-checked={chosen}
      tabindex={option.id === stop ? 0 : -1}
      onclick={() => onchange(option.id)}
    >
      <span class="pill" aria-hidden="true"></span>
      <span class="label" use:tooltip={{ text: option.label, truncated: true }}>{option.label}</span
      >
      {#if option.count !== undefined && option.count !== null}
        <Count value={option.count} tone={option.tone ?? (chosen ? 'soft' : 'plain')} />
      {/if}
    </button>
  {/each}
</div>

<style>
  /* It may shrink inside a flex row too (its options then shorten their labels). */
  .segmented {
    display: inline-flex;
    min-width: 0;
    max-width: 100%;
    height: var(--seg-height);
    padding: var(--space-2);
    border-radius: var(--radius-control);
    background-color: var(--surface-track);
    isolation: isolate;
  }

  /* As wide as its content; it gives way (the label shortens) when the track is short. */
  .option {
    position: relative;
    display: inline-flex;
    flex: 0 1 auto;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
    min-width: 0;
    padding: 0 var(--space-12);
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    font: var(--seg-type);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    transition: color var(--dur-base) var(--ease-standard);
  }

  /* The pill of the chosen option, and the hover wash of the others: one box each,
     exactly the option's own. */
  .pill {
    position: absolute;
    z-index: var(--z-below);
    inset: 0;
    border-radius: inherit;
    background-color: var(--surface-hover);
    opacity: 0;
    transition:
      opacity var(--dur-fast) var(--ease-standard),
      background-color var(--dur-fast) var(--ease-standard);
  }

  .option[aria-checked='true'] .pill {
    background-color: var(--surface);
    box-shadow: var(--sh-thumb);
    opacity: 1;
  }

  .option[aria-checked='false']:hover {
    color: var(--text);
    transition-duration: var(--dur-hover);
  }

  .option[aria-checked='false']:hover .pill {
    opacity: 1;
    transition-duration: var(--dur-hover);
  }

  :global(:where(:root:not([data-aux-press]))) .option[aria-checked='false']:active:hover .pill {
    background-color: var(--surface-press);
  }

  .option[aria-checked='true'] {
    color: var(--nav-active-fg);
  }

  :global(:root[data-window='inactive']) .option[aria-checked='true'] {
    color: var(--text);
  }

  .label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
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
