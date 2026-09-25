// What can be done with all chosen jobs at once: the list header's selection bar and the
// reader's summary offer the same actions (those that fit every chosen job, then the star),
// and share the one question before deleting for good (the dialog sits in the list header).

import type { SelectionAction } from '$components/SelectionBar.svelte';
import { t } from '$lib/i18n/t';
import type { JobView } from '$lib/ipc/types';
import { isExcluded, jobs } from '$lib/state/jobs.svelte';
import { run } from '$lib/state/run.svelte';
import { actionsFor, hasStar, move, purge, toggleStar } from './actions';
import { selection } from './selection.svelte';

class Bulk {
  /** The chosen jobs as the list shows them (active first, then the excluded ones). */
  readonly chosen = $derived.by((): JobView[] => {
    const shown = jobs.shown;
    return selection.jobs([
      ...shown.filter((job) => !isExcluded(job)),
      ...shown.filter(isExcluded),
    ]);
  });

  /** Two or more chosen: the bar and the summary are up. */
  get active(): boolean {
    return this.chosen.length >= 2;
  }

  confirmPurge = $state(false);
  purging = $state(false);
  purgeError = $state<string | null>(null);
  /** A move that failed (said in the list header). */
  error = $state<string | null>(null);

  readonly actions = $derived.by((): SelectionAction[] => {
    const chosen = this.chosen;
    const out: SelectionAction[] = actionsFor(chosen).map((action) => ({
      icon: action.icon,
      label: action.label,
      testid: `selection-${action.id}`,
      // Deleting for good waits for a run (the backend refuses meanwhile).
      disabled: action.id === 'purge' && run.active,
      disabledReason: run.busyText,
      onclick: () => {
        if (action.id === 'purge') {
          this.purgeError = null;
          this.confirmPurge = true;
          return;
        }
        const list = this.chosen;
        selection.clear();
        void move(list, action.id).then((failed) => (this.error = failed));
      },
    }));
    if (chosen.every((job) => hasStar(job.place))) {
      const on = chosen.some((job) => !job.pinned);
      out.push({
        icon: 'star',
        label: on ? t.reader.pin : t.reader.unpin,
        testid: 'selection-star',
        onclick: () => toggleStar(this.chosen),
      });
    }
    return out;
  });

  async purgeChosen(): Promise<void> {
    this.purging = true;
    this.purgeError = await purge(this.chosen);
    this.purging = false;
    if (this.purgeError === null) {
      this.confirmPurge = false;
      selection.clear();
    }
  }
}

export const bulk = new Bulk();
