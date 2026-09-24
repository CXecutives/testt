// What the shell shows: the first-run page while nothing was ever fetched (and no run goes)
// instead of the Jobs view. A mailbox removed later does not bring it back: the jobs stay
// in view, "Abrufen" waits for a mailbox and the list says how to connect one.

import { app } from './app.svelte';
import { run } from './run.svelte';

class Shell {
  /** Until `start_run` answers the first-run page stays (a failed start never flashes). */
  get firstRun(): boolean {
    const state = app.state;
    const going = run.active && !run.starting;
    return state !== null && state.firstRun && !going && run.summary === null;
  }

  /**
   * The run card above the list is up: while a fetch or details run goes, after it until it
   * is hidden, and while a failed start has something to say.
   */
  get runCard(): boolean {
    return (
      run.fetching ||
      run.startError !== null ||
      (run.panel !== 'hidden' && (run.result ?? app.state?.lastRun ?? null) !== null)
    );
  }
}

export const shell = new Shell();
