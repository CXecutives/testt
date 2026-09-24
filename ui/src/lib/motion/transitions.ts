// Token-bound wrappers of svelte/transition, svelte/animate and svelte/motion.
// This folder is the only place allowed to import them (eslint + ui_contract.rs).
//
// Rules: only transform and opacity move; end values are whole pixels; at most
// --stagger-max list rows animate at once, and only rows that arrive while the list is on
// screen. Nothing staggers, nothing bounces. Under reduced motion every movement is dropped
// and what remains is a cross-fade of --dur-crossfade.
//
// No replay by construction: Svelte plays a local transition only when its own block has
// run before, so an entry never plays when a view, a list or the app mounts (main.ts also
// mounts with `intro: false`). One-shots (pulseOnce) are started only from event handlers.

import { flip as svelteFlip } from 'svelte/animate';
import { Tween } from 'svelte/motion';
import { fade as svelteFade, fly as svelteFly, scale as svelteScale } from 'svelte/transition';
import type { AnimationConfig } from 'svelte/animate';
import type { TransitionConfig } from 'svelte/transition';
import {
  crossfadeDuration,
  duration,
  easing,
  enterScale,
  isReducedMotion,
  move,
  play,
  popScale,
  staggerLimit,
  type Duration,
  type Easing,
  type Move,
} from './motion';

export interface MotionParams {
  duration?: Duration;
  easing?: Easing;
  /** Delay in ms. */
  delay?: number;
  /** Play at all (false while the screen is still being built: it is simply there). */
  on?: boolean;
}

export interface RiseParams extends MotionParams {
  distance?: Move;
}

function crossfade(node: Element, delay = 0): TransitionConfig {
  return svelteFade(node, { duration: crossfadeDuration(), delay, easing: easing('standard') });
}

/** Opacity only. */
export function fade(node: Element, params: MotionParams = {}): TransitionConfig {
  if (params.on === false) return {};
  if (isReducedMotion()) return crossfade(node);
  return svelteFade(node, {
    duration: duration(params.duration ?? 'fast'),
    easing: easing(params.easing ?? 'standard'),
    delay: params.delay ?? 0,
  });
}

/** Fade in while rising by a token distance (default 4 px). */
export function rise(node: Element, params: RiseParams = {}): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  return svelteFly(node, {
    y: move(params.distance ?? 'md'),
    duration: duration(params.duration ?? 'base'),
    easing: easing(params.easing ?? 'out'),
    delay: params.delay ?? 0,
    opacity: 0,
  });
}

/** Fade in from --scale-enter (popovers, dialogs, tooltips). */
export function pop(node: Element, params: MotionParams = {}): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  return svelteScale(node, {
    start: enterScale(),
    duration: duration(params.duration ?? 'fast'),
    easing: easing(params.easing ?? 'out'),
    delay: params.delay ?? 0,
    opacity: 0,
  });
}

/** Maximum rows that may FLIP; above this a list cross-fades. */
export const FLIP_LIMIT = 100;

export interface FlipParams extends MotionParams {
  /** Rows in the list: above FLIP_LIMIT rows do not move (the list cross-fades). */
  count?: number;
}

/** FLIP reordering (`animate:flip={{ count: rows.length }}`). */
export function flip(
  node: Element,
  fromTo: { from: DOMRect; to: DOMRect },
  params: FlipParams = {},
): AnimationConfig {
  // Only rows the user can see move: a row that is off screen before and after is placed.
  const offscreen = (rect: DOMRect): boolean => rect.bottom < 0 || rect.top > innerHeight;
  const still =
    (params.count ?? 0) > FLIP_LIMIT || (offscreen(fromTo.from) && offscreen(fromTo.to));
  return svelteFlip(node, fromTo, {
    duration: still ? 0 : duration(params.duration ?? 'base'),
    easing: easing(params.easing ?? 'standard'),
    delay: params.delay ?? 0,
  });
}

export interface RowParams {
  index: number;
  /** The row arrived while the list was on screen (a new job during a run). */
  fresh: boolean;
}

/**
 * Entry of a list row. A list that loads, filters or comes back into view is simply there;
 * only a row that arrives while the list is on screen (a new job during a run) fades in,
 * rising 4 px in 150 ms, and only among the first --stagger-max rows.
 */
export function rowIn(node: Element, { index, fresh }: RowParams): TransitionConfig {
  if (!fresh || index >= staggerLimit()) return {};
  return rise(node, { distance: 'md', duration: 'base' });
}

function lifted(t: number, y: number, scale: number): string {
  const s = scale + (1 - scale) * t;
  return `opacity: ${t}; transform: translateY(${Math.round((1 - t) * y)}px) scale(${s})`;
}

/** Dialog entrance: slow/out (180 ms), rising 4 px from --scale-enter. */
export function dialogIn(node: Element): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  const y = move('md');
  const scale = enterScale();
  return { duration: duration('slow'), easing: easing('out'), css: (t) => lifted(t, y, scale) };
}

/** Dialog exit: fast (100 ms) with ease-in. */
export function dialogOut(node: Element): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  const y = move('md');
  const scale = enterScale();
  return { duration: duration('fast'), easing: easing('in'), css: (t) => lifted(t, y, scale) };
}

/** The scrim behind a dialog follows the dialog's timing. */
export function scrim(
  node: Element,
  _params: unknown,
  options: { direction: string },
): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  const out = options.direction === 'out';
  return svelteFade(node, {
    duration: out ? duration('fast') : duration('slow'),
    easing: easing(out ? 'in' : 'out'),
  });
}

export interface RollParams {
  /** The number went up (it rises from below) or down (it drops from above). */
  up: boolean;
  /** Roll at all (false while the view is still being built: the number is simply there). */
  on?: boolean;
}

/**
 * A count that changes while it is visible (sidebar count, segment counts, tiles, run
 * counters): `{#key value}<span class="roll" in:roll={{ up }}>{value}</span>{/key}` on an
 * inline-block span. The new number rises --move-md in the direction of the change and
 * fades in (150 ms, emphasized); the old one leaves at once (no out, so nothing stacks or
 * reflows). It does not play when the count first appears (a local transition).
 */
export function roll(node: Element, { up, on = true }: RollParams): TransitionConfig {
  if (!on) return {};
  if (isReducedMotion()) return crossfade(node);
  const y = move('md') * (up ? 1 : -1);
  return {
    duration: duration('base'),
    easing: easing('emphasized'),
    css: (t) => `opacity: ${t}; transform: translateY(${Math.round((1 - t) * y)}px)`,
  };
}

export interface CollapseParams {
  /** Collapse at all: only a job that the user moves out of the list (archive, delete,
   *  restore); a row that a filter or a search hides is simply gone. */
  on: boolean;
}

/**
 * A job moved out of the list: its row folds away (150 ms, ease-in) while the rows below
 * close the gap, so nothing jumps. The one animation of a height in the app, and it is
 * one row's (rows are contained, the rest only moves). Instant under reduced motion.
 * `out:rowCollapse={{ on }}` on the row's wrapper in the list.
 */
export function rowCollapse(node: Element, { on }: CollapseParams): TransitionConfig {
  if (!on || isReducedMotion()) return {};
  const height = node.getBoundingClientRect().height;
  return {
    duration: duration('base'),
    easing: easing('in'),
    css: (t) => `overflow: hidden; height: ${Math.round(t * height)}px; opacity: ${t}`,
  };
}

/**
 * The unread dot leaves when the job is read while its row is on screen: it shrinks and
 * fades (150 ms, ease-in). Local, so filtering or unmounting a row never plays it.
 */
export function dotOut(node: Element): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  return {
    duration: duration('base'),
    easing: easing('in'),
    css: (t) => `opacity: ${t}; transform: scale(${t})`,
  };
}

/** The toast enters rising --move-lg from --scale-enter (150 ms, ease-out). */
export function toastIn(node: Element): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  const y = move('lg');
  const scale = enterScale();
  return { duration: duration('base'), easing: easing('out'), css: (t) => lifted(t, y, scale) };
}

/** The toast leaves sideways by --move-lg while it fades (100 ms, ease-in). */
export function toastOut(node: Element): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  const x = move('lg');
  return {
    duration: duration('fast'),
    easing: easing('in'),
    css: (t) => `opacity: ${t}; transform: translateX(${Math.round((1 - t) * x)}px)`,
  };
}

export interface TooltipParams {
  /** Where the bubble sits: it moves --move-sm toward its anchor as it appears. */
  placement: 'top' | 'bottom';
}

/** The tooltip pops toward its anchor (100 ms, ease-out) and leaves with a 60 ms fade. */
export function tooltipIn(node: Element, { placement }: TooltipParams): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  const y = move('sm') * (placement === 'top' ? 1 : -1);
  const scale = enterScale();
  return { duration: duration('fast'), easing: easing('out'), css: (t) => lifted(t, y, scale) };
}

export function tooltipOut(node: Element): TransitionConfig {
  return svelteFade(node, { duration: duration('instant'), easing: easing('in') });
}

/**
 * The one pop of the UI: the star when a job is pinned (1, --scale-pop, 1 in 180 ms). Call
 * it from the click handler that pins (never from an effect, so it cannot replay), on the
 * HTML wrapper of the glyph. The finished animation is dropped at once.
 */
export function pulseOnce(element: Element): void {
  const peak = popScale();
  if (peak === 1) return;
  const animation = play(
    element,
    [
      { transform: 'scale(1)' },
      { transform: `scale(${peak})`, offset: 0.45 },
      { transform: 'scale(1)' },
    ],
    { duration: 'slow', easing: 'out' },
  );
  void animation?.finished.then(
    () => animation.cancel(),
    () => undefined,
  );
}

/**
 * A number that counts up (score rings, stat tiles). Rounds to whole numbers; under
 * reduced motion it jumps to the target.
 */
export function countUp(initial = 0, length: Duration = 'reveal'): Tween<number> {
  return new Tween(initial, {
    duration: () => duration(length),
    easing: easing('out'),
  });
}
