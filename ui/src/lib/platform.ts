// The one place that decides which OS the UI runs on. Per-OS markup exists only in
// TitleBar and WindowControls; CSS may key off `:root[data-platform]` for font smoothing.

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

export function platform(): Platform {
  const value = document.documentElement.dataset.platform;
  return isPlatform(value) ? value : 'windows';
}
