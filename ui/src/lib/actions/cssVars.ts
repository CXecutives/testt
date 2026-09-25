// The only way to hand a runtime value to CSS. The CSP forbids inline styles
// (`style-src 'self'`), but custom properties set through the CSSOM are allowed; so
// components set `--x` here and read it with `var(--x)` in their stylesheet.

import type { Action } from 'svelte/action';

export type CssVars = Record<string, string | number | null | undefined>;

/** Whole pixels as a CSS length (end values of motion must not be fractional). */
export function px(value: number): string {
  return `${Math.round(value)}px`;
}

/**
 * Set (or with null/undefined remove) custom properties; names are given without `--`. A value
 * that `previous` already set stays untouched: every write restyles the element, and a list
 * that renders its rows again (a reload, the end of a run) would restyle each of them.
 */
export function setVars(
  node: HTMLElement | SVGElement,
  vars: CssVars,
  previous: CssVars = {},
): void {
  for (const name of Object.keys(previous)) {
    if (!(name in vars)) node.style.removeProperty(`--${name}`);
  }
  for (const [name, value] of Object.entries(vars)) {
    if (name in previous && previous[name] === value) continue;
    if (value === null || value === undefined) node.style.removeProperty(`--${name}`);
    else node.style.setProperty(`--${name}`, String(value));
  }
}

/** `use:cssVars={{ progress: 0.4, x: px(12) }}` */
export const cssVars: Action<HTMLElement | SVGElement, CssVars> = (node, vars) => {
  let current = vars;
  setVars(node, current);
  return {
    update(next: CssVars) {
      setVars(node, next, current);
      current = next;
    },
  };
};
