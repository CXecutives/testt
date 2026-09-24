// Archiving a job, the same from the list row and from the reader.
// - The job leaves the list it no longer belongs to; the reader keeps showing it, marked
//   archived with "Wiederherstellen", so a second click never lands on another job.
// - A second toggle of the same job within GUARD_MS is ignored (a double click).
// - The toast names the job; archives in a row merge into one toast ("3 Jobs archiviert.")
//   whose "Rückgängig" takes them all back. Restoring says so too.
// - The counts over every job (sidebar, segments, Archiv) follow at once.

import { t } from '$lib/i18n/t';
import { displayTitle } from '$lib/i18n/format';
import type { JobKey, JobView } from '$lib/ipc/types';
import { jobs, keyOf } from '$lib/state/jobs.svelte';
import { toasts } from '$lib/state/toasts.svelte';

const GUARD_MS = 600;
const lastToggle = new Map<string, number>();
/** The archives the current toast speaks of. */
let batch: { keys: JobKey[]; toast: number } | null = null;

function title(job: JobView): string {
  return job.title ? displayTitle(job.title) : t.job.untitled;
}

/** A second toggle of the same job right after the first is a double click. */
function doubled(key: JobKey): boolean {
  const now = performance.now();
  const last = lastToggle.get(keyOf(key));
  lastToggle.set(keyOf(key), now);
  return last !== undefined && now - last < GUARD_MS;
}

async function undo(keys: JobKey[]): Promise<void> {
  // Taken back on purpose: the next toggle of these jobs is no double click.
  for (const key of keys) lastToggle.delete(keyOf(key));
  for (const key of keys) await jobs.archive(key, false);
  await Promise.all([jobs.load(true), jobs.loadOverview()]);
}

/** Archive a job, or bring an archived one back. Resolves with the error text, or null. */
export async function archive(job: JobView): Promise<string | null> {
  const key = job.key;
  if (doubled(key)) return null;
  const restoring = job.archived;
  const error = await jobs.archive(key, !restoring);
  if (error !== null) return error;
  void jobs.loadOverview();
  if (restoring) {
    toasts.show(t.toast.restored(title(job)));
    return null;
  }
  const open = batch !== null && toasts.items.some((item) => item.id === batch?.toast);
  const keys = open && batch !== null ? [...batch.keys, key] : [key];
  if (open && batch !== null) toasts.dismiss(batch.toast);
  toasts.show(
    keys.length === 1 ? t.toast.archivedOne(title(job)) : t.toast.archivedMany(keys.length),
    'success',
    { label: t.common.undo, onclick: () => void undo(keys) },
  );
  const shown = toasts.items.at(-1);
  batch = shown ? { keys, toast: shown.id } : null;
  return null;
}
