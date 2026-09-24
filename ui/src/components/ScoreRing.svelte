<!--
  The match of a job as a ring: sm 40 (list rows), md 56 (reader), lg 96.
  scored: the ring shows its value (r = 15.9155, circumference 100, no pathLength). It fills
  (360 ms, ease-out) with the number counting along only when that means something: when a
  score arrives while the ring is on screen (live scoring during a run), or the first time a
  job is opened (`animate` names the job; once per job and session). A view that comes back
  shows its rings as they are. At most 10 rings fill at the same time, the others are placed
  at once.
  One silhouette for every state, a solid track everywhere; the centre says the state:
  excluded: a pale red track and a ban icon. unscorable: the track and a dash. pending: the
  track and a quarter arc (turning only in the reader). none: the track alone. A score of
  100 sets its digits smaller in the list ring. A selected row passes a navy --ring-track.
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
    if (match.status === 'unscorable') return { status: 'unscorable' };
    return { status: 'scored', score: match.score, band: match.band };
  }

  const MAX_ANIMATING = 10;
  let animating = 0;
  /** Jobs whose ring has filled on opening already (it does not replay; not reactive). */
  const filled: Record<string, true> = {};
</script>

<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { cssVars } from '$lib/actions/cssVars';
  import { de } from '$lib/i18n/de';
  import { formatPercent } from '$lib/i18n/format';
  import { duration, isReducedMotion, play } from '$lib/motion/motion';
  import { countUp } from '$lib/motion/transitions';
  import Icon from './Icon.svelte';

  interface Props {
    ring: RingState;
    size?: 'sm' | 'md' | 'lg';
    /** Fill on mount the first time this job is shown (the reader passes the job's key). */
    animate?: string | null;
    testid?: string | null;
  }

  let { ring, size = 'sm', animate = null, testid = null }: Props = $props();

  const score = $derived(ring.status === 'scored' ? Math.max(0, Math.min(100, ring.score)) : 0);

  const number = countUp(untrack(() => score));
  let shown = $state(untrack(() => score));
  let counting = false;
  let mounted = false;
  let wasScored = false;
  let arc = $state<SVGCircleElement | null>(null);
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

  function place(value: number): void {
    void number.set(value, { duration: 0 });
    shown = value;
  }

  $effect(() => {
    const scored = ring.status === 'scored';
    const value = score;
    untrack(() => {
      const first = !mounted && animate !== null && !(animate in filled);
      if (first && scored && animate !== null) filled[animate] = true;
      const grow = scored && (first || (mounted && !wasScored));
      mounted = true;
      wasScored = scored;
      if (!scored || counting) return;
      if (!grow || isReducedMotion() || animating >= MAX_ANIMATING) {
        place(value);
        return;
      }
      animating += 1;
      counting = true;
      // The arc holds its final value in CSS; the fill is one Web Animation from empty, so
      // it cannot depend on when the engine first computes the style of a new circle.
      shown = value;
      void number.set(0, { duration: 0 });
      number.target = value;
      let fill: Animation | null = null;
      // After the flush: a circle that was just created is bound by then.
      void tick().then(() => {
        if (arc === null) return;
        fill = play(arc, [{ strokeDashoffset: 100 }, { strokeDashoffset: 100 - value }], {
          duration: 'reveal',
          easing: 'out',
        });
      });
      setTimeout(() => {
        // The animation fills both ways: drop it, or a later score would stay masked.
        fill?.cancel();
        animating -= 1;
        counting = false;
        place(score);
      }, duration('reveal'));
    });
  });
</script>

<span
  class="ring {size} {ring.status} {band ?? ''}"
  class:full={ring.status === 'scored' && score === 100}
  role="img"
  aria-label={label}
  data-testid={testid ?? undefined}
>
  <svg class="svg" viewBox="0 0 36 36" aria-hidden="true">
    <circle class="disc" cx="18" cy="18" r="15.9155" />
    <circle class="track" cx="18" cy="18" r="15.9155" />
    {#if ring.status === 'scored'}
      <circle
        bind:this={arc}
        class="value"
        cx="18"
        cy="18"
        r="15.9155"
        use:cssVars={{ value: shown }}
      />
    {/if}
  </svg>
  {#if ring.status === 'pending'}
    <!-- A quarter arc on its own HTML wrapper: it turns only in the reader (md), never in
         the list, and stops under reduced motion. -->
    <span class="wait" aria-hidden="true">
      <svg class="svg" viewBox="0 0 36 36">
        <circle class="arc" cx="18" cy="18" r="15.9155" />
      </svg>
    </span>
  {/if}
  <span class="center">
    {#if ring.status === 'scored'}
      {Math.round(number.current)}
    {:else if ring.status === 'excluded'}
      <Icon name="ban" size={size === 'sm' ? 'sm' : size === 'md' ? 'md' : 'lg'} />
    {:else if ring.status === 'unscorable'}
      –
    {/if}
  </span>
</span>

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
    stroke: var(--ring-track, var(--score-track));
  }

  .excluded .track {
    stroke: var(--score-excluded-track);
  }

  /* Pending: a quarter arc over the track (25 of the 100 units), from 12 o'clock. */
  .wait {
    position: absolute;
    inset: 0;
    display: flex;
  }

  .arc {
    fill: none;
    stroke: var(--border-strong);
    stroke-width: calc(var(--ring-stroke) * var(--ring-scale));
    stroke-dasharray: 25 75;
    stroke-dashoffset: 25;
    stroke-linecap: round;
  }

  /* Only the reader's ring turns while it waits; many turning rings in a list cost frames. */
  .md .wait {
    animation: spin var(--dur-loop) linear infinite;
    animation-play-state: var(--loop-state);
  }

  .value {
    stroke: var(--ring-color);
    stroke-dasharray: 100 100;
    stroke-dashoffset: calc(100 - var(--value));
    stroke-linecap: round;
    transform: rotate(-90deg);
    transform-origin: center;
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
    --ring-type: var(--weight-semibold) var(--font-sm) / var(--leading-sm) var(--font-sans);
  }

  .md {
    --ring-size: var(--ring-md);
    --ring-stroke: var(--ring-md-stroke);
    --ring-scale: var(--ring-md-scale);
    --ring-type: var(--weight-semibold) var(--font-lg) / var(--leading-lg) var(--font-sans);
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

  /* Three digits would touch the 4 px stroke of the 40 px ring. */
  .sm.full {
    --ring-type: var(--weight-semibold) var(--font-xs) / var(--leading-xs) var(--font-sans);
  }
</style>
