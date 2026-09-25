// What a job can do where it is, with one name, icon and order on a row, in the reader and
// in the selection bar (the star, a flag of its own, comes last where there is one):
//   Eingang: Archivieren, Löschen · Archiv: In den Eingang, Löschen · Papierkorb:
//   Wiederherstellen, Endgültig löschen (asks first; the caller shows the dialog).
// A move folds the rows that leave the list (`moving`), opens the next job when the open one
// left, and says so in a toast that merges ("2 Jobs archiviert.") with one undo. A second
// click within GUARD_MS after the pane changed is ignored, so a double click never moves the
// job that just opened.

import type { IconName } from '$components/Icon.svelte';
import { SvelteSet } from 'svelte/reactivity';
import { displayTitle } from '$lib/i18n/format';
import { t } from '$lib/i18n/t';
import type { JobKey, JobView, Place } from '$lib/ipc/types';
import { inFacet, isExcluded, jobs, keyOf, sameKey } from '$lib/state/jobs.svelte';
import { toasts } from '$lib/state/toasts.svelte';

export type MoveId = 'archive' | 'toInbox' | 'trash' | 'restore';
export type ActionId = MoveId | 'purge';

export interface JobAction {
  id: ActionId;
  icon: IconName;
  label: string;
}

const TARGET: Record<MoveId, Place> = {
  archive: 'archive',
  toInbox: 'inbox',
  trash: 'trash',
  restore: 'inbox',
};

const ICON: Record<ActionId, IconName> = {
  archive: 'archive',
  toInbox: 'inbox',
  trash: 'trash-2',
  restore: 'undo-2',
  purge: 'trash-2',
};

const OF_PLACE: Record<Place, readonly ActionId[]> = {
  inbox: ['archive', 'trash'],
  archive: ['toInbox', 'trash'],
  trash: ['restore', 'purge'],
};

/** The actions of a job in this place, in their one order (the star is not one of them). */
export function actionsOf(place: Place): JobAction[] {
  return OF_PLACE[place].map((id) => ({ id, icon: ICON[id], label: t.actions[id] }));
}

/** A favourite never lies in the trash: the star is there in the inbox and the archive. */
export const hasStar = (place: Place): boolean => place !== 'trash';

/** Rows that fold away because the user moved them, until they are gone. */
export const moving = new SvelteSet<string>();

const GUARD_MS = 600;
let guardUntil = 0;

/** A click right after the pane changed to the next job (a double click) does nothing. */
export function guarded(): boolean {
  return performance.now() < guardUntil;
}

const title = (job: JobView): string => (job.title ? displayTitle(job.title) : t.job.untitled);

function said(action: MoveId, job: JobView): (count: number) => string {
  switch (action) {
    case 'archive':
      return (n) => (n === 1 ? t.toast.archivedOne(title(job)) : t.toast.archivedMany(n));
    case 'trash':
      return (n) => (n === 1 ? t.toast.trashedOne(title(job)) : t.toast.trashedMany(n));
    case 'toInbox':
      return (n) => (n === 1 ? t.toast.inboxOne(title(job)) : t.toast.inboxMany(n));
    case 'restore':
      return (n) => (n === 1 ? t.toast.restored(title(job)) : t.toast.restoredMany(n));
  }
}

/** The rows as the list shows them: the active ones, then the excluded ones. */
function order(): JobView[] {
  const shown = jobs.shown;
  return [...shown.filter((job) => !isExcluded(job)), ...shown.filter(isExcluded)];
}

/** The job to open when `gone` leave the list: the next one below, else the one above. */
function nextAfter(gone: readonly JobView[]): JobView | null {
  const rows = order();
  const out = new Set(gone.map((job) => keyOf(job.key)));
  const last = Math.max(...gone.map((job) => rows.findIndex((row) => sameKey(row.key, job.key))));
  const below = rows.slice(last + 1).find((row) => !out.has(keyOf(row.key)));
  if (below) return below;
  return (
    rows
      .slice(0, Math.max(0, last))
      .reverse()
      .find((row) => !out.has(keyOf(row.key))) ?? null
  );
}

/** When the open job left the list, the next one opens (and a second click is ignored). */
function openNext(gone: readonly JobView[], next: JobView | null): void {
  const open = jobs.selected;
  if (open === null || !gone.some((job) => sameKey(job.key, open))) return;
  guardUntil = performance.now() + GUARD_MS;
  if (next) void jobs.select(next, true);
  else jobs.clearSelection();
}

let undoing = 0;

/** Takes one move back; the list loads once every undo of the toast has landed. */
async function undo(key: JobKey, from: Place): Promise<void> {
  undoing += 1;
  await jobs.move([key], from);
  undoing -= 1;
  if (undoing === 0) await Promise.all([jobs.load(true), jobs.loadOverview()]);
}

/** Moves jobs (the row's, the reader's or the selection's). Resolves with the error text. */
export async function move(list: readonly JobView[], action: MoveId): Promise<string | null> {
  if (list.length === 0 || guarded()) return null;
  const to = TARGET[action];
  // Only rows that leave the list fold away (a favourite archived stays among Favoriten).
  const leaving = list.filter((job) => !inFacet({ ...job, place: to }, jobs.facet));
  const next = leaving.length > 0 ? nextAfter(leaving) : null;
  for (const job of leaving) moving.add(keyOf(job.key));
  const error = await jobs.move(
    list.map((job) => job.key),
    to,
  );
  setTimeout(() => {
    for (const job of leaving) moving.delete(keyOf(job.key));
  }, 400);
  if (error !== null) return error;
  openNext(leaving, next);
  void jobs.loadOverview();
  for (const job of list) {
    const from = job.place;
    toasts.undoable(`move-${action}`, said(action, job), t.common.undo, () => {
      void undo(job.key, from);
    });
  }
  return null;
}

/** Deletes jobs of the trash for good (after the dialog). Resolves with the error text. */
export async function purge(list: readonly JobView[]): Promise<string | null> {
  if (list.length === 0) return null;
  const next = nextAfter(list);
  for (const job of list) moving.add(keyOf(job.key));
  const result = await jobs.purge(list.map((job) => job.key));
  setTimeout(() => {
    for (const job of list) moving.delete(keyOf(job.key));
  }, 400);
  if ('error' in result) return result.error;
  openNext(list, next);
  toasts.show(t.toast.deleted(result.count));
  void jobs.loadOverview();
  return null;
}

/** The star: a favourite, or not any more. */
export function toggleStar(list: readonly JobView[]): void {
  const on = list.some((job) => !job.pinned);
  for (const job of list) {
    if (job.pinned !== on) void jobs.pin(job.key, on);
  }
}
