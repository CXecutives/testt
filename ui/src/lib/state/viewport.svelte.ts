// Window width classes the layout reacts to in script (CSS uses the same breakpoints):
// below 1100 px the sidebar becomes the icon rail, below 900 px the Jobs view is one column.
// The width itself is followed too (the list column's limits depend on it).

const RAIL_BELOW = 1100;
const NARROW_BELOW = 900;

function query(width: number): MediaQueryList {
  return matchMedia(`(width < ${width}px)`);
}

class Viewport {
  /** Below 1100 px there is no room for the full sidebar: it is the rail. */
  forcedRail = $state(false);
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

  /** The sidebar shows its icons only (below 1100 px). */
  get rail(): boolean {
    return this.forcedRail;
  }
}

export const viewport = new Viewport();
