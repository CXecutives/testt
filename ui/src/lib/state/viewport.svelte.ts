// Window width classes the layout reacts to in script (CSS uses the same breakpoints):
// below 1100 px the sidebar becomes the icon rail, below 900 px the Jobs view is one column.
// The width itself is followed too (the list column's limits depend on it).

const RAIL_BELOW = 1100;
const NARROW_BELOW = 900;

function query(width: number): MediaQueryList {
  return matchMedia(`(width < ${width}px)`);
}

class Viewport {
  /** Below 1100 px there is no room for the full sidebar: it shows its icons only. */
  rail = $state(false);
  narrow = $state(false);
  /** The inner width of the window in px. */
  width = $state(innerWidth);

  constructor() {
    const rail = query(RAIL_BELOW);
    const narrow = query(NARROW_BELOW);
    this.rail = rail.matches;
    this.narrow = narrow.matches;
    rail.addEventListener('change', (event) => (this.rail = event.matches));
    narrow.addEventListener('change', (event) => (this.narrow = event.matches));
    addEventListener('resize', () => (this.width = innerWidth));
  }
}

export const viewport = new Viewport();
