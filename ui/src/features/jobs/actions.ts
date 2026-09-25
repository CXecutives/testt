// What a job can do where it is, with one name, icon and order on a row, in the reader and
// in the selection bar (the star, a flag of its own, comes last where there is one):
//   Eingang: Archivieren, Löschen · Archiv: In den Eingang, Löschen · Papierkorb:
//   Wiederherstellen, Endgültig löschen (asks first; the caller shows the dialog).
// A move folds the rows that leave the list (`moving`; a few, more simply go), opens the
// next job when the open one left (its row in view, with the focus when the focus was on the
// row that left), and says so in a toast that merges ("2 Jobs archiviert.") with one undo
// (Ctrl/Cmd+Z too, while the toast is up). The undo brings every job back to where it was,
// its row too, and opens the job again that was open when it left. A click within GUARD_MS
// after the list or the pane changed is ignored, so a double click never moves the job that
// slid under the pointer. The job the app opens by itself counts as read only once it has
// been looked at (DWELL_MS on screen in the Jobs view, or a click in the reader). A job that
// is already where it goes is no move. A move, its undo or the star that fails says so in
// the list header (`jobs.actionError`).

import type { IconName } from '$components/Icon.svelte';
import { SvelteSet } from 'svelte/reactivity';
import { displayTitle } from '$lib/i18n/format';
import { t } from '$lib/i18n/t';
import type { Deleted, JobKey, JobView, Place } from '$lib/ipc/types';
import { staggerLimit } from '$lib/motion/motion';
import { inFacet, jobs, keyOf, sameKey, type Unmove } from '$lib/state/jobs.svelte';
import { navigation } from '$lib/state/navigation.svelte';
import { exportText } from '$lib/state/run.svelte';
import { onUndo } from '$lib/input/input';
import { commandKey } from '$lib/platform';
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
  toInbox: 'briefcase',
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

// Ctrl/Cmd+Z takes back the newest move (or "all read") while its toast is up.
onUndo(() => toasts.undoLast());

const GUARD_MS = 500;
let guardUntil = 0;
/** How long the next job opened by the app stays on screen before it counts as read. */
const DWELL_MS = 2000;
/** The dwell counts in steps of this, only while the job is on screen. */
const DWELL_STEP = 250;
let dwell: ReturnType<typeof setInterval> | undefined;

/** A click right after the pane changed to the next job (a double click) does nothing. */
export function guarded(): boolean {
  return performance.now() < guardUntil;
}

/** A title in a toast: whole (the toast cuts it to its line and shows it in a tooltip); only
 *  an absurdly long one is cut here. */
const TOAST_TITLE = 120;
function title(job: JobView): string {
  const full = job.title ? displayTitle(job.title) : t.job.untitled;
  return full.length > TOAST_TITLE ? `${full.slice(0, TOAST_TITLE - 1).trimEnd()}…` : full;
}

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

/** The job to open when `gone` leave the list: the next one below, else the one above; none
 *  when the list did not hold them (a job opened from the day overview). */
function nextAfter(gone: readonly JobView[]): JobView | null {
  const rows = jobs.shown;
  const out = new Set(gone.map((job) => keyOf(job.key)));
  const last = Math.max(...gone.map((job) => rows.findIndex((row) => sameKey(row.key, job.key))));
  if (last < 0) return null;
  const below = rows.slice(last + 1).find((row) => !out.has(keyOf(row.key)));
  if (below) return below;
  return (
    rows
      .slice(0, Math.max(0, last))
      .reverse()
      .find((row) => !out.has(keyOf(row.key))) ?? null
  );
}

/** The list changed under the pointer: clicks on it wait a moment. */
function arm(): void {
  guardUntil = performance.now() + GUARD_MS;
}

/** Another list (another tab or place): nothing slid under the pointer, clicks count. */
export function disarm(): void {
  guardUntil = 0;
}

/** The job is on screen: the Jobs view is shown and the window is in front. */
function onScreen(): boolean {
  return (
    navigation.current === 'jobs' &&
    document.visibilityState !== 'hidden' &&
    document.documentElement.dataset['window'] !== 'inactive'
  );
}

/** The focus was on a row of the list, or on one of its tools. */
function inRow(): boolean {
  return document.activeElement?.closest('[data-key]') != null;
}

/**
 * When the open job left the list, the next one opens, not yet read (see `seen`): its row
 * comes into view, and takes the focus when the focus was on the row (or its tool) that left.
 * `open` is the job that was open before the action (deleting it for good already closed it).
 */
function openNext(
  gone: readonly JobView[],
  next: JobView | null,
  focus: boolean,
  open: JobKey | null,
): void {
  if (open === null || !gone.some((job) => sameKey(job.key, open))) return;
  clearInterval(dwell);
  if (!next) {
    jobs.clearSelection();
    return;
  }
  void jobs.select(next, false);
  jobs.reveal = { key: keyOf(next.key), focus };
  const key = next.key;
  let looked = 0;
  dwell = setInterval(() => {
    if (!sameKey(jobs.selected, key)) {
      clearInterval(dwell);
      return;
    }
    if (onScreen()) looked += DWELL_STEP;
    if (looked >= DWELL_MS) seen(key);
  }, DWELL_STEP);
}

/** The job the app opened has been looked at (the dwell, or a click in the reader). */
export function seen(key: JobKey | null = jobs.selected): void {
  clearInterval(dwell);
  if (key !== null && sameKey(jobs.selected, key)) jobs.markSeen(key);
}

/**
 * Takes one move back (the jobs to where they were, their rows too), then opens the job
 * again that was open when the move took it away, if it is listed again.
 */
async function undo(
  back: readonly Unmove[],
  generation: number,
  reopen: JobKey | null,
): Promise<void> {
  const result = await jobs.moveBack(back, generation);
  jobs.actionError = 'error' in result ? result.error : null;
  void jobs.loadOverview();
  // Only a job that really came back opens again (one already back is left as it is).
  if ('error' in result || reopen === null) return;
  if (!result.moved.some((key) => sameKey(key, reopen))) return;
  const row = jobs.rows.find((job) => sameKey(job.key, reopen));
  if (row) {
    await jobs.select(row, false);
    jobs.reveal = { key: keyOf(row.key), focus: false };
  }
}

/** Jobs deleted for good: their undo toasts can do nothing any more (the others stay), and
 *  a result file that could not follow says so in the list header. */
function deletedFor(deleted: Deleted): void {
  toasts.forget(new Set(deleted.keys.map(keyOf)));
  jobs.exportNote = exportText(deleted.exportError);
}

/** Single moves in this session; after the third one a tip says several go at once. */
let singles = 0;
const TIP_KEY = 'jobs-tip-choose';
const TIP_AFTER = 3;

function tipOnce(): void {
  singles += 1;
  if (singles !== TIP_AFTER) return;
  try {
    if (localStorage.getItem(TIP_KEY) !== null) return;
    localStorage.setItem(TIP_KEY, '1');
  } catch {
    // Without a store the tip would come every session: better not at all.
    return;
  }
  toasts.show(t.selection.tip(t.selection.commandKey[commandKey()]), 'info');
}

/** Two or more jobs chosen and moved at once: the tip about choosing is known. */
function tipKnown(): void {
  try {
    localStorage.setItem(TIP_KEY, '1');
  } catch {
    // Without a store the tip may still come once this session.
    return;
  }
}

/**
 * Moves jobs (the row's, the reader's or the selection's). Resolves with the error text,
 * which the caller shows where the move was asked (the list header for a row or the chosen
 * jobs, the reader for its own).
 */
export async function move(all: readonly JobView[], action: MoveId): Promise<string | null> {
  const to = TARGET[action];
  // A job that already lies there is no move (and no toast says it moved).
  const list = all.filter((job) => job.place !== to);
  if (list.length === 0 || guarded()) return null;
  // Only rows that leave the list fold away (a favourite archived stays among Favoriten), and
  // only a few: many rows folding at once would hold the page for frames.
  const leaving = list.filter((job) => !inFacet({ ...job, place: to }, jobs.facet));
  const next = leaving.length > 0 ? nextAfter(leaving) : null;
  const focus = inRow();
  const folding = leaving.length <= staggerLimit() ? leaving : [];
  for (const job of folding) moving.add(keyOf(job.key));
  // What the undo brings back: each job as the list held it, and where its row stood.
  const generation = jobs.generation;
  const back: Unmove[] = list.map((job) => {
    const at = jobs.rows.findIndex((row) => sameKey(row.key, job.key));
    return { job: jobs.rows[at] ?? job, to, at };
  });
  const open = jobs.selected;
  const reopen =
    open !== null && leaving.some((job) => sameKey(job.key, open)) ? { ...open } : null;
  // The list changes now, not when the backend answers: the second click of a double click
  // may come first (it would take the job straight back from where it went).
  if (leaving.length > 0) arm();
  const result = await jobs.move(
    list.map((job) => job.key),
    to,
  );
  setTimeout(() => {
    for (const job of folding) moving.delete(keyOf(job.key));
  }, 400);
  if ('error' in result) return result.error;
  if (leaving.length > 0) arm();
  openNext(leaving, next, focus, open);
  // Moved into the listed place without being listed (opened from elsewhere): list it.
  if (
    list.some(
      (job) => !leaving.includes(job) && !jobs.rows.some((row) => sameKey(row.key, job.key)),
    )
  ) {
    void jobs.load(true);
  }
  void jobs.loadOverview();
  if (list.length === 1) tipOnce();
  else tipKnown();
  // A toast and its undo only for the jobs that really moved.
  const moved = new Set(result.moved.map(keyOf));
  const undone = back.filter((entry) => moved.has(keyOf(entry.job.key)));
  const first = undone[0];
  if (first === undefined) return null;
  toasts.undoable(
    `move-${action}`,
    said(action, first.job),
    t.common.undo,
    () => undo(undone, generation, reopen),
    undone.length,
    undone.map((entry) => keyOf(entry.job.key)),
  );
  return null;
}

/** Deletes jobs of the trash for good (after the dialog). Resolves with the error text. */
export async function purge(list: readonly JobView[]): Promise<string | null> {
  if (list.length === 0) return null;
  const next = nextAfter(list);
  const focus = inRow();
  const folding = list.length <= staggerLimit() ? list : [];
  for (const job of folding) moving.add(keyOf(job.key));
  const open = jobs.selected;
  const result = await jobs.purge(list.map((job) => job.key));
  setTimeout(() => {
    for (const job of folding) moving.delete(keyOf(job.key));
  }, 400);
  if ('error' in result) return result.error;
  arm();
  openNext(list, next, focus, open);
  deletedFor(result);
  // Like a move: one job by its title, more by their number.
  const gone = list.filter((job) => result.keys.some((key) => sameKey(key, job.key)));
  const only = result.count === 1 ? (gone[0] ?? list[0]) : undefined;
  toasts.show(
    only === undefined ? t.toast.deletedMany(result.count) : t.toast.deletedOne(title(only)),
  );
  void jobs.loadOverview();
  return null;
}

/** After the trash was emptied: no undo can reach its jobs any more. */
export function trashEmptied(deleted: Deleted): void {
  deletedFor(deleted);
}

/**
 * The actions for chosen jobs: those of their place; chosen from several places (Favoriten
 * holds inbox and archive) only what fits every one of them.
 */
export function actionsFor(list: readonly JobView[]): JobAction[] {
  const places = [...new Set(list.map((job) => job.place))];
  if (places.length === 1 && places[0]) return actionsOf(places[0]);
  return actionsOf('inbox').filter((action) => action.id === 'trash');
}

/** The star: a favourite, or not any more (one that fails says so in the list header). */
export function toggleStar(list: readonly JobView[]): void {
  const on = list.some((job) => !job.pinned);
  for (const job of list) {
    if (job.pinned !== on) {
      void jobs.pin(job.key, on).then((error) => {
        if (error !== null) jobs.actionError = error;
      });
    }
  }
}
