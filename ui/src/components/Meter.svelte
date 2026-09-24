<!--
  Progress bar. Determinate (value 0..1) fills by scaleX (180 ms, ease-out). Indeterminate
  (value null) sweeps a short bar (linear, so it never seems to stall); under reduced motion
  it rests as a calm full track instead of a paused bar outside the track. Progress is navy
  on a navy wash (brand); a quota near its limit is ochre (warning).
-->
<script lang="ts" module>
  export type MeterTone = 'brand' | 'neutral' | 'warning';
  export type MeterSize = 'sm' | 'md' | 'lg';
</script>

<script lang="ts">
  import { cssVars } from '$lib/actions/cssVars';
  import { settled } from '$lib/motion/settled.svelte';

  interface Props {
    value: number | null;
    tone?: MeterTone;
    size?: MeterSize;
    label: string;
    testid?: string | null;
  }

  let { value, tone = 'brand', size = 'md', label, testid = null }: Props = $props();

  const motion = settled();
  const clamped = $derived(value === null ? null : Math.max(0, Math.min(1, value)));
</script>

<div
  class="meter {tone} {size}"
  class:indeterminate={clamped === null}
  class:ready={motion.ready}
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
    background-color: var(--meter-track-colour);
  }

  .fill {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: inherit;
    background-color: var(--meter-color);
    transform: scaleX(var(--progress));
    transform-origin: left center;
  }

  /* It fills only once the bar has been drawn (a meter that mounts shows its value). */
  .ready .fill {
    transition: transform var(--dur-slow) var(--ease-out);
  }

  .indeterminate .fill {
    right: auto;
    width: 40%;
    transform: translateX(-100%);
    transition: none;
    animation: sweep var(--dur-loop) linear infinite;
    animation-play-state: var(--loop-state);
  }

  /* Reduced motion: no sweep; a paused one would sit outside the track and vanish. */
  :global(:root[data-motion='reduce']) .indeterminate .fill {
    width: 100%;
    opacity: var(--opacity-muted);
    transform: none;
    animation: none;
  }

  .brand {
    --meter-color: var(--meter-fill);
    --meter-track-colour: var(--meter-track);
  }

  .neutral {
    --meter-color: var(--text-subtle);
    --meter-track-colour: var(--surface-muted);
  }

  .warning {
    --meter-color: var(--meter-warning);
    --meter-track-colour: var(--surface-muted);
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
