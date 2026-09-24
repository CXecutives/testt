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
}

export const shell = new Shell();
