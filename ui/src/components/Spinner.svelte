<!-- Indeterminate activity. Colour follows the text; stops under reduced motion. -->
<script lang="ts">
  import { de } from '$lib/i18n/de';

  interface Props {
    size?: 'sm' | 'md' | 'lg';
    /** Announced to screen readers; omit when the surrounding control says it already. */
    label?: string | null;
  }

  let { size = 'md', label = de.common.loading }: Props = $props();
</script>

<span
  class="spinner {size}"
  role={label ? 'status' : undefined}
  aria-label={label ?? undefined}
  aria-hidden={label ? undefined : 'true'}
></span>

<style>
  .spinner {
    display: inline-block;
    flex: none;
    width: var(--spinner-size);
    height: var(--spinner-size);
    border: var(--focus-width) solid currentcolor;
    border-right-color: transparent;
    border-radius: var(--radius-full);
    opacity: 0.8;
    /* Half a loop per turn: a full --dur-loop turn reads as stalled. */
    animation: spin calc(var(--dur-loop) / 2) linear infinite;
    animation-play-state: var(--loop-state);
  }

  .sm {
    --spinner-size: var(--icon-sm);
  }

  .md {
    --spinner-size: var(--icon-md);
  }

  .lg {
    --spinner-size: var(--icon-lg);
  }
</style>
