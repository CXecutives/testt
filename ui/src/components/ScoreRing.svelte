<!--
  The match of a job as a ring: sm 40 (list rows), md 56 (reader), lg 96.
  scored: the ring shows its value (r = 15.9155, circumference 100, no pathLength). It fills
  (360 ms, ease-out) with the number counting along only when that means something: when a
  score arrives while the ring is on screen (live scoring during a run), or the first time a
  job is opened (`animate` names the job; once per job and session). A view that comes back
  shows its rings as they are. At most 10 rings fill at the same time, the others are placed
  at once.
  A scored ring takes the colour of its decile (ten steps, red through orange and yellow
  to green; `d0` ... `d9`) with ink digits; the tinted disc of the larger rings follows
  the band. Every ring has the same solid track; the centre and the arc say the state:
  provisional (a score from a teaser only) looks exactly like a scored ring, the row's
  badge and the reader say that it is not final (its name says it too). none: not scored
  yet: the track with an empty centre. excluded: a pale red track and a ban icon.
  unscorable, and off (no usable profile, so no match at all): the track and a dash.
  pending: in the reader a quarter arc turns on the track; in the list (sm, where many
  turning arcs would cost frames and a still arc looks like a frozen spinner) the track
  breathes slowly (2 s, opacity only); under reduced motion both stand still. A score of
  100 sets its digits smaller in the list ring. A selected row passes a warm --ring-track.
-->
<script lang="ts" module>
  import type { Band, DetailState, JobMatch } from '$lib/ipc/types';

  export type RingState =
    | { status: 'scored'; score: number; band: Band }
    | { status: 'provisional'; score: number; band: Band }
    | { status: 'excluded' }
    | { status: 'unscorable' }
    | { status: 'pending' }
    | { status: 'none' }
    | { status: 'off' };

  /**
   * The ring state of a job's match (null = not scored yet). `detail` is the state of the
   * job's details: a score from a teaser is provisional, and a job whose details are still
   * coming is not "not rateable" yet but simply not scored.
   */
  export function ringState(
    match: JobMatch | null,
    pending = false,
    detail: DetailState['kind'] | null = null,
  ): RingState {
    if (match === null) return pending ? { status: 'pending' } : { status: 'none' };
    if (match.status === 'excluded') return { status: 'excluded' };
    if (match.status === 'unscorable') {
      return detail === 'pending' ? { status: 'none' } : { status: 'unscorable' };
    }
    if (detail === 'teaser') {
      return { status: 'provisional', score: match.score, band: match.band };
    }
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
  import { t } from '$lib/i18n/t';
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

  /** A number on the ring (final or provisional). */
  const valued = $derived(ring.status === 'scored' || ring.status === 'provisional');
  const score = $derived(valued && 'score' in ring ? Math.max(0, Math.min(100, ring.score)) : 0);

  const number = countUp(untrack(() => score));
  let shown = $state(untrack(() => score));
  let counting = false;
  let mounted = false;
  let wasScored = false;
  let arc = $state<SVGCircleElement | null>(null);
  const band = $derived(valued && 'band' in ring ? ring.band : null);
  /** The colour step: the decile of the score, 100 in the last one. */
  const step = $derived(valued ? `d${Math.min(9, Math.floor(score / 10))}` : '');

  const label = $derived.by(() => {
    switch (ring.status) {
      case 'scored':
        return `${t.score.value(formatPercent(ring.score))} · ${t.score.band[ring.band]}`;
      case 'provisional':
        return `${t.score.value(formatPercent(ring.score))} · ${t.score.band[ring.band]} · ${t.score.provisional}`;
      case 'excluded':
        return t.score.excluded;
      case 'unscorable':
        return t.score.unscorable;
      case 'pending':
        return t.score.pending;
      case 'off':
        return t.score.off;
      default:
        return t.score.none;
    }
  });

  function place(value: number): void {
    void number.set(value, { duration: 0 });
    shown = value;
  }

  $effect(() => {
    const scored = valued;
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
  class="ring {size} {ring.status} {band ?? ''} {step}"
  class:full={valued && score === 100}
  role="img"
  aria-label={label}
  data-testid={testid ?? undefined}
>
  <svg class="svg" viewBox="0 0 36 36" aria-hidden="true">
    <circle class="disc" cx="18" cy="18" r="15.9155" />
    <circle class="track" cx="18" cy="18" r="15.9155" />
    {#if valued}
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
    <!-- On its own HTML wrapper (a loop on an SVG child runs on the main thread): the
         reader's quarter arc turns, the list's track breathes; both stand still
         under reduced motion. -->
    <span class="wait" aria-hidden="true">
      <svg class="svg" viewBox="0 0 36 36">
        <circle class="arc" cx="18" cy="18" r="15.9155" />
      </svg>
    </span>
  {/if}
  <span class="center">
    {#if valued}
      {Math.round(number.current)}
    {:else if ring.status === 'excluded'}
      <Icon name="ban" size={size === 'sm' ? 'sm' : size === 'md' ? 'md' : 'lg'} />
    {:else if ring.status === 'unscorable' || ring.status === 'off'}
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

  /* The list ring that waits: no spinner shape, the whole track breathes instead (it moves
     onto the layer that breathes). */
  .sm.pending .track {
    stroke: none;
  }

  .sm .arc {
    stroke: var(--ring-track, var(--score-track));
    stroke-dasharray: none;
  }

  .sm .wait {
    animation: breathe var(--dur-breathe) var(--ease-standard) infinite;
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

  /* The disc of the larger rings follows the band; the digits are ink. */
  .scored,
  .provisional {
    --ring-text: var(--score-digits);
  }

  .high {
    --ring-surface: var(--score-high-surface);
  }

  .mid {
    --ring-surface: var(--score-mid-surface);
  }

  .low {
    --ring-surface: var(--score-low-surface);
  }

  /* The ring colour: the decile of the score. */
  .d0 {
    --ring-color: var(--score-ring-0);
  }

  .d1 {
    --ring-color: var(--score-ring-1);
  }

  .d2 {
    --ring-color: var(--score-ring-2);
  }

  .d3 {
    --ring-color: var(--score-ring-3);
  }

  .d4 {
    --ring-color: var(--score-ring-4);
  }

  .d5 {
    --ring-color: var(--score-ring-5);
  }

  .d6 {
    --ring-color: var(--score-ring-6);
  }

  .d7 {
    --ring-color: var(--score-ring-7);
  }

  .d8 {
    --ring-color: var(--score-ring-8);
  }

  .d9 {
    --ring-color: var(--score-ring-9);
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
  .sm.scored,
  .sm.provisional {
    --ring-surface: transparent;
  }

  /* Three digits would touch the 4 px stroke of the 40 px ring. */
  .sm.full {
    --ring-type: var(--weight-semibold) var(--font-xs) / var(--leading-xs) var(--font-sans);
  }
</style>
