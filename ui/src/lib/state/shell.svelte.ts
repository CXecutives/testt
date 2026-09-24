// What the shell shows: the first-run page (no mailbox yet, or nothing ever fetched and no
// run going) instead of the Jobs view.

import { app } from './app.svelte';
import { run } from './run.svelte';

class Shell {
  get firstRun(): boolean {
    const state = app.state;
    return (
      state !== null && (!app.hasMailbox || (state.firstRun && !run.active && run.summary === null))
    );
  }

  /** The run card above the list is up: during a run, and after it until it is hidden. */
  get runCard(): boolean {
    return (
      run.active || (run.panel !== 'hidden' && (run.summary ?? app.state?.lastRun ?? null) !== null)
    );
  }
}

export const shell = new Shell();
