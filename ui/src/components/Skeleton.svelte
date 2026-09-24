<!-- Loading placeholder: line | block | circle, with a shimmer sweeping on ::after. -->
<script lang="ts">
  import { cssVars } from '$lib/actions/cssVars';

  interface Props {
    shape?: 'line' | 'block' | 'circle';
    /** Width in percent of the container (lines and blocks). */
    width?: number;
    /** Circle diameter. */
    size?: 'sm' | 'md' | 'lg';
  }

  let { shape = 'line', width = 100, size = 'sm' }: Props = $props();
</script>

<span
  class="skeleton {shape} {size}"
  aria-hidden="true"
  use:cssVars={{ 'skeleton-width': `${Math.max(0, Math.min(100, width))}%` }}
></span>

<style>
  .skeleton {
    position: relative;
    display: block;
    overflow: hidden;
    background-color: var(--surface-muted);
    isolation: isolate;
  }

  .skeleton::after {
    position: absolute;
    inset: 0;
    background: var(--grad-shimmer);
    content: '';
    transform: translateX(-100%);
    animation: shimmer var(--dur-loop) var(--ease-standard) infinite;
    animation-play-state: var(--loop-state);
  }

  .line {
    width: var(--skeleton-width);
    height: var(--space-12);
    border-radius: var(--radius-full);
  }

  .block {
    width: var(--skeleton-width);
    height: var(--row-height);
    border-radius: var(--radius-md);
  }

  .circle {
    width: var(--circle);
    height: var(--circle);
    border-radius: var(--radius-full);
  }

  .circle.sm {
    --circle: var(--ring-sm);
  }

  .circle.md {
    --circle: var(--ring-md);
  }

  .circle.lg {
    --circle: var(--ring-lg);
  }
</style>
