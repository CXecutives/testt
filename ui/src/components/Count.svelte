<!--
  A number in a small pill, 12/500 with tabular digits and one minimum width (4 and 12 weigh
  the same):
  - soft: the navy wash (the chosen segment, the excluded divider, sub-labels such as
    "Erfüllt", the run countdown, "n neu"),
  - strong: the deep navy pill (the sidebar unread count, nowhere else at rest),
  - plain: the same box without a pill (an unchosen segment), so a change of tone never
    moves anything.
  When the number changes while it is on screen it rolls 4 px in the direction of the
  change (150 ms); it never rolls when it first appears.
-->
<script lang="ts" module>
  export type CountTone = 'soft' | 'strong' | 'plain';
  export const COUNT_TONES: readonly CountTone[] = ['soft', 'strong', 'plain'];
</script>

<script lang="ts">
  import { untrack } from 'svelte';
  import { formatNumber } from '$lib/i18n/format';
  import { roll } from '$lib/motion/transitions';

  interface Props {
    value: number;
    tone?: CountTone;
    /** Accessible name when the number alone would not say what it counts. */
    label?: string | null;
    testid?: string | null;
  }

  let { value, tone = 'soft', label = null, testid = null }: Props = $props();

  let previous = untrack(() => value);
  let up = $state(true);

  $effect.pre(() => {
    const next = value;
    untrack(() => {
      up = next >= previous;
      previous = next;
    });
  });
</script>

<span
  class="count {tone}"
  role={label ? 'img' : undefined}
  aria-label={label ?? undefined}
  data-testid={testid ?? undefined}
>
  {#key value}<span class="digits" in:roll={{ up }}>{formatNumber(value)}</span>{/key}
</span>

<style>
  .count {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    min-width: var(--count-min);
    height: var(--badge-height);
    padding: 0 var(--space-6);
    overflow: hidden;
    border-radius: var(--radius-full);
    background-color: var(--count-pill);
    color: var(--count-text);
    font: var(--type-xs);
    font-weight: var(--weight-medium);
    font-variant-numeric: var(--numeric);
    transition:
      background-color var(--dur-base) var(--ease-standard),
      color var(--dur-base) var(--ease-standard);
  }

  .digits {
    display: inline-block;
  }

  .soft {
    --count-pill: var(--count-soft-bg);
    --count-text: var(--count-soft-fg);
  }

  .strong {
    --count-pill: var(--count-bg);
    --count-text: var(--count-fg);
  }

  .plain {
    --count-pill: transparent;
    --count-text: currentcolor;
  }
</style>
