// Token-bound wrappers of svelte/transition, svelte/animate and svelte/motion.
// This folder is the only place allowed to import them (eslint + ui_contract.rs).
//
// Rules: only transform and opacity move; end values are whole pixels; at most
// --stagger-max list rows animate at once, and only rows that arrive while the list is on
// screen. Nothing staggers, nothing bounces. Under reduced motion every movement is dropped
// and what remains is a cross-fade of --dur-crossfade.

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
}

export interface RiseParams extends MotionParams {
  distance?: Move;
}

function crossfade(node: Element, delay = 0): TransitionConfig {
  return svelteFade(node, { duration: crossfadeDuration(), delay, easing: easing('standard') });
}

/** Opacity only. */
export function fade(node: Element, params: MotionParams = {}): TransitionConfig {
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

/**
 * View switch: the old view leaves at once (no cross-fade of two full views), the new one
 * fades in while rising 4 px (base/out, 150 ms).
 */
export function viewIn(node: Element): TransitionConfig {
  return rise(node, { duration: 'base', easing: 'out', distance: 'md' });
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
  const still = (params.count ?? 0) > FLIP_LIMIT;
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
