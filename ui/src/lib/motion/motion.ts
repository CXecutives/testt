// Motion values for JavaScript-driven animation (Svelte transitions, WAAPI, tweens).
//
// Everything comes from the tokens in tokens.css, read once at start. Reduced motion is
// enforced HERE and not through the CSS media query, because Svelte transitions and the Web
// Animations API ignore it: moves and durations become 0, cross-fades stay at
// --dur-crossfade, loops stop, numbers jump to their final value. The same switch sets
// `:root[data-motion="reduce"]`, which zeroes the CSS-side tokens.

import { tokenMs, tokenNumber, tokenPx, token } from '../tokens';

export type Duration = 'instant' | 'hover' | 'fast' | 'base' | 'slow' | 'reveal' | 'loop';
export type Easing = 'standard' | 'out' | 'in' | 'emphasized';
export type Move = 'xs' | 'sm' | 'md' | 'lg';
export type EasingFn = (t: number) => number;

interface Values {
  durations: Record<Duration, number>;
  crossfade: number;
  easings: Record<Easing, EasingFn>;
  moves: Record<Move, number>;
  staggerMax: number;
  enterScale: number;
  popScale: number;
  tooltipDelay: number;
}

const DURATIONS: readonly Duration[] = [
  'instant',
  'hover',
  'fast',
  'base',
  'slow',
  'reveal',
  'loop',
];
const EASINGS: readonly Easing[] = ['standard', 'out', 'in', 'emphasized'];
const MOVES: readonly Move[] = ['xs', 'sm', 'md', 'lg'];

let values: Values | null = null;
let reduced = false;
const listeners = new Set<(reduced: boolean) => void>();

/** Cubic Bézier easing as used by CSS (`cubic-bezier(x1, y1, x2, y2)`). */
export function cubicBezier(x1: number, y1: number, x2: number, y2: number): EasingFn {
  const cx = 3 * x1;
  const bx = 3 * (x2 - x1) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * y1;
  const by = 3 * (y2 - y1) - cy;
  const ay = 1 - cy - by;
  const sampleX = (t: number): number => ((ax * t + bx) * t + cx) * t;
  const sampleY = (t: number): number => ((ay * t + by) * t + cy) * t;
  const slopeX = (t: number): number => (3 * ax * t + 2 * bx) * t + cx;
  const solveT = (x: number): number => {
    let t = x;
    for (let i = 0; i < 8; i += 1) {
      const error = sampleX(t) - x;
      if (Math.abs(error) < 1e-6) return t;
      const slope = slopeX(t);
      if (Math.abs(slope) < 1e-6) break;
      t -= error / slope;
    }
    let lo = 0;
    let hi = 1;
    t = x;
    while (hi - lo > 1e-6) {
      if (sampleX(t) < x) lo = t;
      else hi = t;
      t = (lo + hi) / 2;
    }
    return t;
  };
  return (x: number): number => (x <= 0 ? 0 : x >= 1 ? 1 : sampleY(solveT(x)));
}

function parseEasing(value: string): EasingFn {
  const numbers = /cubic-bezier\(([^)]+)\)/.exec(value)?.[1]?.split(',').map(Number) ?? [];
  const [x1, y1, x2, y2] = numbers;
  if (x1 === undefined || y1 === undefined || x2 === undefined || y2 === undefined) {
    return (t) => t;
  }
  return cubicBezier(x1, y1, x2, y2);
}

function read(): Values {
  return {
    durations: Object.fromEntries(DURATIONS.map((d) => [d, tokenMs(`--dur-${d}`)])) as Record<
      Duration,
      number
    >,
    crossfade: tokenMs('--dur-crossfade'),
    easings: Object.fromEntries(
      EASINGS.map((e) => [e, parseEasing(token(`--ease-${e}`))]),
    ) as Record<Easing, EasingFn>,
    moves: Object.fromEntries(MOVES.map((m) => [m, tokenPx(`--move-${m}`)])) as Record<
      Move,
      number
    >,
    staggerMax: tokenNumber('--stagger-max'),
    enterScale: tokenNumber('--scale-enter'),
    popScale: tokenNumber('--scale-pop'),
    tooltipDelay: tokenMs('--delay-tooltip'),
  };
}

function current(): Values {
  values ??= read();
  return values;
}

function apply(next: boolean): void {
  reduced = next;
  document.documentElement.dataset.motion = next ? 'reduce' : 'full';
  for (const listener of listeners) listener(next);
}

/** Call once in main.ts, after the stylesheets are in place and before mounting. */
export function installMotion(): void {
  if (values !== null) return;
  // Read the full values BEFORE the reduce switch can zero the CSS tokens.
  current();
  const query = matchMedia('(prefers-reduced-motion: reduce)');
  apply(query.matches);
  query.addEventListener('change', (event) => apply(event.matches));
}

export function isReducedMotion(): boolean {
  return reduced;
}

/** Notified whenever the reduced-motion preference changes. Returns an unsubscribe function. */
export function onMotionChange(listener: (reduced: boolean) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Duration in ms; 0 under reduced motion. */
export function duration(name: Duration): number {
  return reduced ? 0 : current().durations[name];
}

/** Cross-fades survive reduced motion: they carry state changes, not movement. */
export function crossfadeDuration(): number {
  return current().crossfade;
}

export function easing(name: Easing): EasingFn {
  return current().easings[name];
}

/** Travel distance in whole px; 0 under reduced motion. */
export function move(name: Move): number {
  return reduced ? 0 : current().moves[name];
}

export function enterScale(): number {
  return reduced ? 1 : current().enterScale;
}

/** Peak of the one-shot pop (the star on pin); 1 under reduced motion. WAAPI keyframes
 *  cannot take var(), so the token is resolved here. */
export function popScale(): number {
  return reduced ? 1 : current().popScale;
}

/** How many items of a list may animate at the same time; the others are simply there. */
export function staggerLimit(): number {
  return current().staggerMax;
}

export function tooltipDelay(): number {
  return current().tooltipDelay;
}

export interface PlayOptions {
  duration: Duration;
  easing?: Easing;
  delay?: number;
  /** Kept at --dur-crossfade under reduced motion (opacity-only animations). */
  crossfade?: boolean;
}

/**
 * The only use of the Web Animations API in the UI. Keyframes may animate transform,
 * opacity and stroke-dashoffset only. Under reduced motion the animation jumps to its
 * end state unless it is a cross-fade.
 */
export function play(
  element: Element,
  keyframes: Keyframe[],
  options: PlayOptions,
): Animation | null {
  const ms = options.crossfade && reduced ? crossfadeDuration() : duration(options.duration);
  const timing: KeyframeAnimationOptions = {
    duration: ms,
    delay: reduced ? 0 : (options.delay ?? 0),
    fill: 'both',
  };
  if (ms === 0 && timing.delay === 0) return null;
  // WAAPI takes the CSS easing string of the token as it is.
  timing.easing = token(`--ease-${options.easing ?? 'standard'}`);
  return element.animate(keyframes, timing);
}
