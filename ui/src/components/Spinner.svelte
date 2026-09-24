<!-- Indeterminate activity. Colour follows the text, or the navy of progress (`progress`:
     the run card, the sidebar status); stops under reduced motion. -->
<script lang="ts">
  import { t } from '$lib/i18n/t';

  interface Props {
    size?: 'sm' | 'md' | 'lg';
    /** Announced to screen readers; omit when the surrounding control says it already. */
    label?: string | null;
    /** In the navy of progress instead of the text colour. */
    progress?: boolean;
  }

  let { size = 'md', label, progress = false }: Props = $props();
  // Without a label the spinner says "loading" in the app's language (null says nothing).
  const spoken = $derived(label === undefined ? t.common.loading : label);
</script>

<span
  class="spinner {size}"
  class:progress
  role={spoken ? 'status' : undefined}
  aria-label={spoken ?? undefined}
  aria-hidden={spoken ? undefined : 'true'}
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

  .progress {
    color: var(--meter-fill);
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
