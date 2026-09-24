// Token-bound wrappers of svelte/transition, svelte/animate and svelte/motion.
// This folder is the only place allowed to import them (eslint + ui_contract.rs).
//
// Rules: only transform and opacity move; end values are whole pixels; at most
// --stagger-max items are staggered. Under reduced motion every movement is dropped and
// what remains is a cross-fade of --dur-crossfade.

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
  staggerDelay,
  type Duration,
  type Easing,
  type Move,
} from './motion';

export interface MotionParams {
  duration?: Duration;
  easing?: Easing;
  /** Delay in ms, usually from `stagger(index)`. */
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

/** Fade in while rising by a token distance (default 8 px). */
export function rise(node: Element, params: RiseParams = {}): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  return svelteFly(node, {
    y: move(params.distance ?? 'lg'),
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

/** View switch: the new view rises in (base/out)... */
export function viewIn(node: Element): TransitionConfig {
  return rise(node, { duration: 'base', easing: 'out', distance: 'lg' });
}

/** ...while the old one fades out quickly (fast/in). */
export function viewOut(node: Element): TransitionConfig {
  return fade(node, { duration: 'fast', easing: 'in' });
}

/** Entry delay of the n-th list item (30 ms steps, capped at 10 items). */
export function stagger(index: number): number {
  return staggerDelay(index);
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
  count: number;
}

/**
 * Entry of a list row: the first --stagger-max rows rise one after another, the rest (and
 * every row of a list above FLIP_LIMIT) just fade in.
 */
export function rowIn(node: Element, { index, count }: RowParams): TransitionConfig {
  if (count > FLIP_LIMIT) return fade(node, { duration: 'fast' });
  return rise(node, { distance: 'md', duration: 'base', delay: stagger(index) });
}

function lifted(t: number, y: number, scale: number): string {
  const s = scale + (1 - scale) * t;
  return `opacity: ${t}; transform: translateY(${Math.round((1 - t) * y)}px) scale(${s})`;
}

/** Dialog entrance: slow/out, rising 4 px from --scale-enter. */
export function dialogIn(node: Element): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  const y = move('md');
  const scale = enterScale();
  return { duration: duration('slow'), easing: easing('out'), css: (t) => lifted(t, y, scale) };
}

/** Dialog exit: 0.7 × slow with ease-in. */
export function dialogOut(node: Element): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  const y = move('md');
  const scale = enterScale();
  return {
    duration: Math.round(duration('slow') * 0.7),
    easing: easing('in'),
    css: (t) => lifted(t, y, scale),
  };
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
    duration: out ? Math.round(duration('slow') * 0.7) : duration('slow'),
    easing: easing(out ? 'in' : 'out'),
  });
}

/**
 * A number that counts up (score rings, stat tiles). Rounds to whole numbers; under
 * reduced motion it jumps to the target.
 */
export function countUp(initial = 0): Tween<number> {
  return new Tween(initial, {
    duration: () => duration('reveal'),
    easing: easing('out'),
  });
}
