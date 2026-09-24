<!--
  The match of a job as a ring: sm 40 (list rows), lg 96 (reader).
  scored: the ring fills by stroke-dashoffset (r = 15.9155, circumference 100, no
  pathLength) and the number counts up once, the first time the ring is visible; a high
  band glows once. At most 10 rings animate at the same time, the rest are placed at once.
  excluded: dashed track and a ban icon. unscorable: dashed track and a dash.
  pending: skeleton. none: empty track.
-->
<script lang="ts" module>
  import type { Band, JobMatch } from '$lib/ipc/types';

  export type RingState =
    | { status: 'scored'; score: number; band: Band }
    | { status: 'excluded' }
    | { status: 'unscorable' }
    | { status: 'pending' }
    | { status: 'none' };

  /** The ring state of a job's match (null = not scored yet). */
  export function ringState(match: JobMatch | null, pending = false): RingState {
    if (match === null) return pending ? { status: 'pending' } : { status: 'none' };
    if (match.status === 'excluded') return { status: 'excluded' };
    if (match.status === 'unscorable' || match.score === null || match.band === null) {
      return { status: 'unscorable' };
    }
    return { status: 'scored', score: match.score, band: match.band };
  }

  const MAX_ANIMATING = 10;
  let animating = 0;
</script>

<script lang="ts">
  import { cssVars } from '$lib/actions/cssVars';
  import { reveal } from '$lib/actions/reveal';
  import { de } from '$lib/i18n/de';
  import { formatPercent } from '$lib/i18n/format';
  import { duration, isReducedMotion } from '$lib/motion/motion';
  import { countUp } from '$lib/motion/transitions';
  import Icon from './Icon.svelte';
  import Skeleton from './Skeleton.svelte';

  interface Props {
    ring: RingState;
    size?: 'sm' | 'lg';
    testid?: string | null;
  }

  let { ring, size = 'sm', testid = null }: Props = $props();

  const number = countUp(0);
  let shown = $state(0);
  let revealed = $state(false);
  let counting = $state(false);
  let glowing = $state(false);

  const score = $derived(ring.status === 'scored' ? Math.max(0, Math.min(100, ring.score)) : 0);
  const band = $derived(ring.status === 'scored' ? ring.band : null);

  const label = $derived.by(() => {
    switch (ring.status) {
      case 'scored':
        return `${de.score.value(formatPercent(ring.score))} · ${de.score.band[ring.band]}`;
      case 'excluded':
        return de.score.excluded;
      case 'unscorable':
        return de.score.unscorable;
      case 'pending':
        return de.score.pending;
      default:
        return de.score.none;
    }
  });

  function start(): void {
    revealed = true;
    if (ring.status !== 'scored') return;
    if (animating >= MAX_ANIMATING || isReducedMotion()) {
      number.set(score, { duration: 0 });
      shown = score;
      return;
    }
    animating += 1;
    counting = true;
    number.target = score;
    shown = score;
    setTimeout(() => {
      animating -= 1;
      counting = false;
      glowing = band === 'high';
    }, duration('reveal'));
  }

  // A rescore after the first reveal moves straight to the new value.
  $effect(() => {
    if (revealed && !counting && ring.status === 'scored') {
      number.set(score, { duration: 0 });
      shown = score;
    }
  });
</script>

{#if ring.status === 'pending'}
  <span class="ring {size}" role="img" aria-label={label} data-testid={testid ?? undefined}>
    <Skeleton shape="circle" size={size === 'sm' ? 'sm' : 'lg'} />
  </span>
{:else}
  <span
    class="ring {size} {ring.status} {band ?? ''}"
    class:counting
    class:glowing
    role="img"
    aria-label={label}
    data-testid={testid ?? undefined}
    use:reveal={start}
  >
    <svg class="svg" viewBox="0 0 36 36" aria-hidden="true">
      <circle class="disc" cx="18" cy="18" r="15.9155" />
      <circle class="track" cx="18" cy="18" r="15.9155" />
      {#if ring.status === 'scored'}
        <circle class="value" cx="18" cy="18" r="15.9155" use:cssVars={{ value: shown }} />
      {/if}
    </svg>
    <span class="center">
      {#if ring.status === 'scored'}
        {Math.round(number.current)}
      {:else if ring.status === 'excluded'}
        <Icon name="ban" size={size === 'sm' ? 'sm' : 'lg'} />
      {:else if ring.status === 'unscorable'}
        –
      {/if}
    </span>
  </span>
{/if}

<style>
  .ring {
    position: relative;
    display: inline-flex;
    flex: none;
    width: var(--ring-size);
    height: var(--ring-size);
    color: var(--ring-text);
    --ring-color: var(--score-track);
    --ring-text: var(--text-subtle);
    --ring-surface: transparent;
  }

  .svg {
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .disc {
    fill: var(--ring-surface);
  }

  .track,
  .value {
    fill: none;
    stroke-width: calc(var(--ring-stroke) * var(--ring-scale));
  }

  .track {
    stroke: var(--score-track);
  }

  .excluded .track,
  .unscorable .track {
    stroke-dasharray: 2.5 2.5;
  }

  .value {
    stroke: var(--ring-color);
    stroke-dasharray: 100 100;
    stroke-dashoffset: calc(100 - var(--value));
    stroke-linecap: round;
    transform: rotate(-90deg);
    transform-origin: center;
  }

  .counting .value {
    transition: stroke-dashoffset var(--dur-reveal) var(--ease-out);
  }

  .center {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font: var(--ring-type);
    font-variant-numeric: var(--numeric);
    letter-spacing: var(--tracking-tight);
  }

  /* One glow for a high match, faded in and out on ::after. */
  .ring::after {
    position: absolute;
    inset: 0;
    border-radius: var(--radius-full);
    box-shadow: var(--glow-high);
    content: '';
    opacity: 0;
    pointer-events: none;
  }

  .glowing::after {
    animation: pulse var(--dur-hero) var(--ease-standard) 1;
  }

  .high {
    --ring-color: var(--score-high-ring);
    --ring-text: var(--score-high-text);
    --ring-surface: var(--score-high-surface);
  }

  .mid {
    --ring-color: var(--score-mid-ring);
    --ring-text: var(--score-mid-text);
    --ring-surface: var(--score-mid-surface);
  }

  .low {
    --ring-color: var(--score-low-ring);
    --ring-text: var(--score-low-text);
    --ring-surface: var(--score-low-surface);
  }

  .excluded {
    --ring-text: var(--score-excluded);
  }

  .sm {
    --ring-size: var(--ring-sm);
    --ring-stroke: var(--ring-sm-stroke);
    --ring-scale: var(--ring-sm-scale);
    --ring-type: var(--weight-bold) var(--font-sm) / var(--leading-sm) var(--font-sans);
  }

  .lg {
    --ring-size: var(--ring-lg);
    --ring-stroke: var(--ring-lg-stroke);
    --ring-scale: var(--ring-lg-scale);
    --ring-type: var(--type-2xl);
  }

  /* The small ring stays calm: no tinted disc inside a 40 px row. */
  .sm.scored {
    --ring-surface: transparent;
  }
</style>
