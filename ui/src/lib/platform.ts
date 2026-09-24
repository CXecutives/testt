// The one place that decides which OS the UI runs on, and the only place that knows how the
// two differ. Inside the window both are the same app; what differs does so by the
// convention of the OS (docs/PLAN.md, "Platforms"):
//   - the window frame is the native one of the OS (nothing of it is drawn here); on macOS
//     its title bar is transparent over the page (unified toolbar row, base.css) and the
//     page marks the empty parts of that row as drag regions (`dragBands()`),
//   - dialog buttons: Windows puts the primary first, macOS last (right),
//   - scrollbars: slim styled ones on Windows, the native overlay scrollbars on macOS
//     (base.css keys them off `:root[data-platform]`, like the font smoothing),
//   - words that name OS things (Explorer / Finder, the password store).
// Components ask here (`dragBands()`, `primaryFirst()`, `platform()`), never compare OS names
// themselves. The window's focus state is the same on both: `:root[data-window]` is
// 'inactive' while the window is in the background, and selections grey out against it as
// in Mail and Explorer.

import { onWindowFocus } from './ipc/api';

export type Platform = 'windows' | 'macos';

function isPlatform(value: string | null | undefined): value is Platform {
  return value === 'windows' || value === 'macos';
}

/** `?platform=macos|windows` (harness and gallery) wins over the user agent. */
function detect(): Platform {
  const requested = new URLSearchParams(location.search).get('platform');
  if (isPlatform(requested)) return requested;
  return /Macintosh|Mac OS X/.test(navigator.userAgent) ? 'macos' : 'windows';
}

/** Sets `data-platform` on <html> unless the host already did. Call once before mounting. */
export function applyPlatform(): Platform {
  const root = document.documentElement;
  if (!isPlatform(root.dataset.platform)) root.dataset.platform = detect();
  return platform();
}

/**
 * Keeps `data-window` on <html> in step with the OS window ('active' | 'inactive'); the
 * components style against it with a colour transition, nothing per component in JS. Call
 * once before mounting.
 */
export function trackWindowFocus(): void {
  const root = document.documentElement;
  root.dataset.window = 'active';
  onWindowFocus((focused) => {
    root.dataset.window = focused ? 'active' : 'inactive';
  });
}

export function platform(): Platform {
  const value = document.documentElement.dataset.platform;
  return isPlatform(value) ? value : 'windows';
}

/**
 * The page keeps the toolbar row free and marks its empty parts as drag regions (macOS: the
 * title bar is transparent over the page and WKWebView has no app-region). Windows has its
 * native title bar above the page.
 */
export function dragBands(): boolean {
  return platform() === 'macos';
}

/** Dialog buttons: the primary action comes first on Windows, last (right) on macOS. */
export function primaryFirst(): boolean {
  return platform() === 'windows';
}
