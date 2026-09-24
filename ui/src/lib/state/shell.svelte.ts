// What the shell shows: the first-run page while nothing was ever fetched (and no run goes)
// instead of the Jobs view. A mailbox removed later does not bring it back: the jobs stay
// in view, "Abrufen" waits for a mailbox and the list says how to connect one.

import { app } from './app.svelte';
import { run } from './run.svelte';

class Shell {
  get firstRun(): boolean {
    const state = app.state;
    return state !== null && state.firstRun && !run.active && run.summary === null;
  }

  /** The run card above the list is up: during a run, and after it until it is hidden. */
  get runCard(): boolean {
    return (
      run.active || (run.panel !== 'hidden' && (run.summary ?? app.state?.lastRun ?? null) !== null)
    );
  }
}

export const shell = new Shell();
