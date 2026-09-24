// Reads design tokens from tokens.css at runtime, so TypeScript never repeats a value.

const cache = new Map<string, string>();

/** The raw value of a custom property on :root (cached; tokens do not change at runtime). */
export function token(name: `--${string}`): string {
  const hit = cache.get(name);
  if (hit !== undefined) return hit;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  if (value !== '') cache.set(name, value);
  return value;
}

/** A length token in pixels (`12px` -> 12). Unknown or non-pixel values give 0. */
export function tokenPx(name: `--${string}`): number {
  const match = /^(-?[\d.]+)px$/.exec(token(name));
  return match?.[1] === undefined ? 0 : Number(match[1]);
}

/** A time token in milliseconds (`150ms` -> 150, `0.3s` -> 300). */
export function tokenMs(name: `--${string}`): number {
  const match = /^([\d.]+)(ms|s)$/.exec(token(name));
  if (match?.[1] === undefined) return 0;
  const value = Number(match[1]);
  return match[2] === 's' ? value * 1000 : value;
}

/** A unitless number token (`0.97`, `10`). */
export function tokenNumber(name: `--${string}`): number {
  const value = Number(token(name));
  return Number.isFinite(value) ? value : 0;
}

/** Forget cached values (only needed by the gallery after it changed :root attributes). */
export function clearTokenCache(): void {
  cache.clear();
}
