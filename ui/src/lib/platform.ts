// The one place that decides which OS the UI runs on, and the only place that knows how the
// two differ. Inside the window both are the same app; what differs does so by the
// convention of the OS (docs/PLAN.md, "Platforms"):
//   - the window frame is the native one of the OS (nothing of it is drawn here),
//   - dialog buttons: Windows puts the primary first, macOS last (right),
//   - scrollbars: slim styled ones on Windows, the native overlay scrollbars on macOS
//     (base.css keys them off `:root[data-platform]`, like the font smoothing),
//   - words that name OS things (Explorer / Finder, the password store),
//   - the editing keys of text fields (`keyConventions()`, applied by lib/input/input.ts).
// Components ask here (`primaryFirst()`, `platform()`), never compare OS names themselves.

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

/** Dialog buttons: the primary action comes first on Windows, last (right) on macOS. */
export function primaryFirst(): boolean {
  return platform() === 'windows';
}

/** How the keyboard of the OS edits text in a field (lib/input/input.ts applies it). */
export interface KeyConventions {
  /** Option types characters (@ is Option+L on a German Mac) and moves by word, like
   *  AltGr on Windows; on Windows a plain Alt is the menu and navigation key. */
  optionTypes: boolean;
  /** The modifier of the editing shortcuts: Cmd on macOS, Ctrl on Windows. */
  command: 'metaKey' | 'ctrlKey';
  /** Ctrl+Y redoes (Windows); macOS redoes with Cmd+Shift+Z only. */
  redoWithY: boolean;
  /** Ctrl+A/E/B/F/N/P/D/H/K move and delete like in every macOS text field. */
  controlEdits: boolean;
}

export function keyConventions(): KeyConventions {
  const mac = platform() === 'macos';
  return {
    optionTypes: mac,
    command: mac ? 'metaKey' : 'ctrlKey',
    redoWithY: !mac,
    controlEdits: mac,
  };
}
