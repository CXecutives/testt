// Token-bound wrappers of svelte/transition, svelte/animate and svelte/motion.
// This folder is the only place allowed to import them (eslint + ui_contract.rs).
//
// Rules: only transform and opacity move (and the height of a folding row and of the job
// list's selection bar, each one small box); end values are whole pixels; at most
// --stagger-max list rows animate at once, and only rows that arrive while the list is on
// screen. Nothing staggers, nothing bounces. Under reduced motion every movement is dropped
// and what remains is a cross-fade of --dur-crossfade.
//
// No replay by construction: Svelte plays a local transition only when its own block has
// run before, so an entry never plays when a view, a list or the app mounts (main.ts also
// mounts with `intro: false`). One-shots (pulseOnce) are started only from event handlers.
//
// fade and rise never read the element's style: svelte/transition's versions call
// getComputedStyle for the element's own opacity and transform, which forces a style and
// layout pass in the middle of the script (a new job in the reader lays out the whole page
// twice in one task). The elements they move are opaque and untransformed, so the result is
// the same.

import { flip as svelteFlip } from 'svelte/animate';
import { Tween } from 'svelte/motion';
import { scale as svelteScale } from 'svelte/transition';
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

/** Opacity from 0 to 1 (in) or back (out), without reading the element's style. */
function opacity(ms: number, ease: (t: number) => number, delay = 0): TransitionConfig {
  return { duration: ms, easing: ease, delay, css: (t) => `opacity: ${t}` };
}

function crossfade(_node: Element, delay = 0): TransitionConfig {
  return opacity(crossfadeDuration(), easing('standard'), delay);
}

/** Opacity only. */
export function fade(node: Element, params: MotionParams = {}): TransitionConfig {
  if (params.on === false) return {};
  if (isReducedMotion()) return crossfade(node);
  return opacity(
    duration(params.duration ?? 'fast'),
    easing(params.easing ?? 'standard'),
    params.delay ?? 0,
  );
}

/** Fade in while rising by a token distance (default 4 px). */
export function rise(node: Element, params: RiseParams = {}): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  const y = move(params.distance ?? 'md');
  return {
    duration: duration(params.duration ?? 'base'),
    easing: easing(params.easing ?? 'out'),
    delay: params.delay ?? 0,
    css: (t, u) => `transform: translateY(${u * y}px); opacity: ${t}`,
  };
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

/**
 * Entry of a list row. A list that loads, filters or comes back into view is simply there;
 * only a row that arrives while the list is on screen (a new job during a run) fades in,
 * rising 4 px in 150 ms, and only among the first --stagger-max rows (the list decides).
 * A Web Animation started by the list after the row is in place, not a transition on
 * every row: the transition would run (and fire its events) for each of hundreds of rows
 * that simply appear.
 */
export function rowEnter(row: Element): void {
  const reduced = isReducedMotion();
  const from: Keyframe = reduced
    ? { opacity: 0 }
    : { opacity: 0, transform: `translateY(${move('md')}px)` };
  const to: Keyframe = reduced ? { opacity: 1 } : { opacity: 1, transform: 'none' };
  const motion = play(row, [from, to], {
    duration: 'base',
    easing: reduced ? 'standard' : 'out',
    crossfade: true,
  });
  motion?.addEventListener('finish', () => motion.cancel());
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
  return opacity(duration(out ? 'fast' : 'slow'), easing(out ? 'in' : 'out'));
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

/** Where the job list's selection bar stands: its top in the list and its height (px). */
export interface BarBox {
  top: number;
  height: number;
}

/**
 * The job list's one selection bar goes to another row like the sidebar's pill (180 ms,
 * emphasized): its top and its height move together, so it keeps the inset of rows of
 * either height. `glide` follows a row that glides to its new place instead, in the row's
 * own timing (150 ms, standard). The bar's style holds `to` already; the animation starts
 * from `from` and is dropped once it ends. Null under reduced motion (it is simply there).
 */
export function barSlide(
  bar: HTMLElement,
  from: BarBox,
  to: BarBox,
  glide = false,
): Animation | null {
  const at = (box: BarBox): Keyframe => ({
    transform: `translateY(${Math.round(box.top)}px)`,
    height: `${Math.round(box.height)}px`,
  });
  const motion = play(
    bar,
    [at(from), at(to)],
    glide ? { duration: 'base' } : { duration: 'slow', easing: 'emphasized' },
  );
  motion?.addEventListener('finish', () => motion.cancel());
  return motion;
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

/**
 * A row's tools come into being under the pointer, when their hover rule already holds: they
 * fade in like their CSS transition would (--dur-fast, standard ease); under reduced motion
 * they are simply there, as their CSS is then.
 */
export function toolsIn(_node: Element): TransitionConfig {
  if (isReducedMotion()) return {};
  return opacity(duration('fast'), easing('standard'));
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
  /** Where the bubble sits: it comes --move-sm out of its anchor as it appears. */
  placement: 'top' | 'bottom' | 'right';
}

/** The tooltip pops out of its anchor (100 ms, ease-out) and leaves with a 60 ms fade. */
export function tooltipIn(node: Element, { placement }: TooltipParams): TransitionConfig {
  if (isReducedMotion()) return crossfade(node);
  const scale = enterScale();
  if (placement === 'right') {
    const x = -move('sm');
    return {
      duration: duration('fast'),
      easing: easing('out'),
      css: (t) =>
        `opacity: ${t}; transform: translateX(${Math.round((1 - t) * x)}px) scale(${scale + (1 - scale) * t})`,
    };
  }
  const y = move('sm') * (placement === 'top' ? 1 : -1);
  return { duration: duration('fast'), easing: easing('out'), css: (t) => lifted(t, y, scale) };
}

export function tooltipOut(_node: Element): TransitionConfig {
  return opacity(duration('instant'), easing('in'));
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
