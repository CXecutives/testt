// Archiving a job, the same from the list row and from the reader: the job leaves the list
// it no longer belongs to, the toast can take it back, and when it was the open job the
// next one of the list opens (the previous one at the end, else the day overview).
// Bringing a job back from the archive needs no toast: the row leaving the archive says it.

import { de } from '$lib/i18n/de';
import type { JobKey, JobView } from '$lib/ipc/types';
import { jobs, sameKey } from '$lib/state/jobs.svelte';
import { toasts } from '$lib/state/toasts.svelte';

async function undo(key: JobKey): Promise<void> {
  if ((await jobs.hide(key, false)) === null) void jobs.load(true);
}

/** Archive a job, or bring an archived one back. Resolves with the error text, or null. */
export async function archive(job: JobView): Promise<string | null> {
  const key = job.key;
  if (job.hidden) return jobs.hide(key, false);
  const open = sameKey(jobs.selected, key);
  const list = jobs.visible;
  const at = list.findIndex((row) => sameKey(row.key, key));
  const next = open && at >= 0 ? (list[at + 1] ?? list[at - 1] ?? null) : null;
  const error = await jobs.hide(key, true);
  if (error !== null) return error;
  toasts.show(de.toast.hidden, 'success', { label: de.common.undo, onclick: () => void undo(key) });
  if (open) {
    if (next !== null) void jobs.select(next, false);
    else jobs.clearSelection();
  }
  return null;
}
