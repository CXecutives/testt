// The one "now" of the page for texts that name a time relative to it ("vor 5 Min.",
// "gestern", "Abgerufen 08:30" that gains its date after midnight). It moves at every full
// minute (so also at midnight), and at once when the window comes back to the front; while
// the window is hidden it rests. Only the texts that read it render again, never whole rows.

const MINUTE = 60_000;

class Clock {
  now = $state(new Date());
  #timer: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    this.#arm();
    addEventListener('focus', () => this.#tick());
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'hidden') this.#rest();
      else this.#tick();
    });
  }

  /** Now, and the next tick at the next full minute. */
  #tick(): void {
    this.now = new Date();
    this.#arm();
  }

  #arm(): void {
    this.#rest();
    const left = MINUTE - (Date.now() % MINUTE);
    this.#timer = setTimeout(() => this.#tick(), left);
  }

  #rest(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
  }
}

export const clock = new Clock();
