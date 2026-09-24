// Window width classes the layout reacts to in script (CSS uses the same breakpoints):
// below 1100 px the sidebar becomes the icon rail, below 900 px the Jobs view is one column.

const RAIL_BELOW = 1100;
const NARROW_BELOW = 900;

function query(width: number): MediaQueryList {
  return matchMedia(`(width < ${width}px)`);
}

class Viewport {
  rail = $state(false);
  narrow = $state(false);

  constructor() {
    const rail = query(RAIL_BELOW);
    const narrow = query(NARROW_BELOW);
    this.rail = rail.matches;
    this.narrow = narrow.matches;
    rail.addEventListener('change', (event) => (this.rail = event.matches));
    narrow.addEventListener('change', (event) => (this.narrow = event.matches));
  }
}

export const viewport = new Viewport();
