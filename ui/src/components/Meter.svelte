<!--
  Progress bar. Determinate (value 0..1) fills by scaleX; the brand tone carries a light
  edge travelling along the fill. Indeterminate (value null) sweeps a short bar.
-->
<script lang="ts" module>
  export type MeterTone = 'brand' | 'neutral' | 'warning';
  export type MeterSize = 'sm' | 'md' | 'lg';
</script>

<script lang="ts">
  import { cssVars } from '$lib/actions/cssVars';

  interface Props {
    value: number | null;
    tone?: MeterTone;
    size?: MeterSize;
    label: string;
    testid?: string | null;
  }

  let { value, tone = 'brand', size = 'md', label, testid = null }: Props = $props();

  const clamped = $derived(value === null ? null : Math.max(0, Math.min(1, value)));
</script>

<div
  class="meter {tone} {size}"
  class:indeterminate={clamped === null}
  role="progressbar"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={clamped === null ? undefined : Math.round(clamped * 100)}
  data-testid={testid ?? undefined}
>
  <span class="fill" use:cssVars={{ progress: clamped ?? 1 }}></span>
</div>

<style>
  .meter {
    position: relative;
    width: 100%;
    height: var(--meter-height);
    overflow: hidden;
    border-radius: var(--radius-full);
    background-color: var(--surface-muted);
  }

  .fill {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: inherit;
    background: var(--meter-fill);
    transform: scaleX(var(--progress));
    transform-origin: left center;
    transition: transform var(--dur-slow) var(--ease-out);
  }

  /* The travelling light edge of the brand tone. */
  .brand .fill::after {
    position: absolute;
    inset: 0;
    background: var(--grad-shimmer);
    content: '';
    animation: shimmer var(--dur-loop) var(--ease-standard) infinite;
    animation-play-state: var(--loop-state);
  }

  .indeterminate .fill {
    right: auto;
    width: 40%;
    transform: translateX(-100%);
    transition: none;
    animation: sweep var(--dur-loop) var(--ease-standard) infinite;
    animation-play-state: var(--loop-state);
  }

  .brand {
    --meter-fill: var(--grad-brand);
  }

  .neutral {
    --meter-fill: var(--text-subtle);
  }

  .warning {
    --meter-fill: var(--warning-strong);
  }

  .sm {
    --meter-height: var(--meter-sm);
  }

  .md {
    --meter-height: var(--meter-md);
  }

  .lg {
    --meter-height: var(--meter-lg);
  }
</style>
