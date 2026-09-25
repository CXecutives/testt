// Window width classes the layout reacts to in script (CSS uses the same breakpoints):
// below 1100 px the sidebar becomes the icon rail, below 900 px the Jobs view is one column.
// From 1100 px on the user may fold the sidebar to its rail herself (a click on its edge,
// Ctrl+B or Cmd+B, the macOS menu); that choice is kept and holds again once the window is
// wide enough. The width itself is followed too (the list column's limits depend on it).

const RAIL_BELOW = 1100;
const NARROW_BELOW = 900;
/** Where the folded sidebar is kept (this browser profile). */
const KEPT = 'sidebar-rail';

function query(width: number): MediaQueryList {
  return matchMedia(`(width < ${width}px)`);
}

function kept(): boolean {
  try {
    return localStorage.getItem(KEPT) === '1';
  } catch {
    return false;
  }
}

function keep(folded: boolean): void {
  try {
    if (folded) localStorage.setItem(KEPT, '1');
    else localStorage.removeItem(KEPT);
  } catch {
    // Without a store the choice lasts for this session only.
    return;
  }
}

class Viewport {
  /** Below 1100 px there is no room for the full sidebar: it is the rail, whatever was chosen. */
  forcedRail = $state(false);
  /** The user folded the sidebar to its rail (kept). */
  pinnedRail = $state(kept());
  narrow = $state(false);
  /** The inner width of the window in px. */
  width = $state(innerWidth);

  constructor() {
    const rail = query(RAIL_BELOW);
    const narrow = query(NARROW_BELOW);
    this.forcedRail = rail.matches;
    this.narrow = narrow.matches;
    rail.addEventListener('change', (event) => (this.forcedRail = event.matches));
    narrow.addEventListener('change', (event) => (this.narrow = event.matches));
    addEventListener('resize', () => (this.width = innerWidth));
  }

  /** The sidebar shows its icons only: forced by the width or chosen by the user. */
  get rail(): boolean {
    return this.forcedRail || this.pinnedRail;
  }

  /** Fold the sidebar to its rail or unfold it; below 1100 px nothing changes (it is the
   *  rail anyway, and a choice made there unseen would surprise later). */
  toggleRail(): void {
    if (this.forcedRail) return;
    this.pinnedRail = !this.pinnedRail;
    keep(this.pinnedRail);
  }
}

export const viewport = new Viewport();
