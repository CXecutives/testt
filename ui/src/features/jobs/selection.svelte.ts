// The rows chosen like in a mail app: Ctrl+click (Cmd on macOS) takes a row in or out, Shift+
// click takes the range from the anchor, and so do Shift+ArrowUp/ArrowDown and Shift+Home/End
// from the row reached last (its moving end, like Explorer and Mail); a plain click chooses
// one row and opens it.
// Two or more chosen rows (one in one column) show the selection bar in the list header;
// Esc clears them. The open job belongs to a selection that starts from it: a Ctrl+click
// adds to it, and while nothing else is chosen a range starts from it (also when the app
// opened it: the next job after a move, a job of the day overview); otherwise from the
// anchor, the row clicked last.

import type { JobView } from '$lib/ipc/types';
import { jobs, keyOf } from '$lib/state/jobs.svelte';

class Selection {
  /** The chosen rows (keyOf), in the order they were taken. */
  keys = $state<string[]>([]);
  /** The same keys to look up (each row asks whether it is chosen). */
  readonly #chosen = $derived(new Set(this.keys));
  /** Where a Shift range starts (the last row clicked, or the start of the last range). */
  #anchor: string | null = null;
  /** The row a choice reached last (clicked with Ctrl or Shift, or by Shift+Arrow). */
  #end: string | null = null;

  get size(): number {
    return this.keys.length;
  }

  has(job: JobView): boolean {
    return this.#chosen.has(keyOf(job.key));
  }

  clear(): void {
    this.keys = [];
    this.#anchor = null;
    this.#end = null;
  }

  /** Where Shift+Arrow moves on from: the row reached last while rows are chosen, else the
   *  open job. */
  get end(): string | null {
    const open = jobs.selected ? keyOf(jobs.selected) : null;
    return this.keys.length > 0 ? (this.#end ?? open) : open;
  }

  /** A plain click: this row alone (it opens). */
  only(job: JobView): void {
    this.keys = [];
    this.#anchor = keyOf(job.key);
  }

  /** The open job, as the start of a selection. */
  #seed(): string[] {
    if (this.keys.length > 0) return this.keys;
    return jobs.selected ? [keyOf(jobs.selected)] : [];
  }

  /** Ctrl/Cmd+click: the row in or out. */
  toggle(job: JobView): void {
    const key = keyOf(job.key);
    const keys = this.#seed();
    this.keys = keys.includes(key) ? keys.filter((other) => other !== key) : [...keys, key];
    this.#anchor = key;
    this.#end = key;
  }

  /** Shift+click: every row from the start of the range to this one, in the order of the
   *  list. Without a start that is still listed the click takes this row in or out. */
  range(job: JobView, rows: readonly JobView[]): void {
    const order = rows.map((row) => keyOf(row.key));
    const open = jobs.selected ? keyOf(jobs.selected) : null;
    const starts = this.keys.length === 0 ? [open, this.#anchor] : [this.#anchor, open];
    const start = starts.find((key) => key !== null && order.includes(key));
    const to = order.indexOf(keyOf(job.key));
    if (start === undefined || start === null || to === -1) {
      this.toggle(job);
      return;
    }
    const from = order.indexOf(start);
    this.keys = order.slice(Math.min(from, to), Math.max(from, to) + 1);
    this.#anchor = start;
    this.#end = keyOf(job.key);
  }

  /** Rows that left the list leave the choice too; `true` if any did. */
  prune(listed: ReadonlySet<string>): boolean {
    if (this.keys.every((key) => listed.has(key))) return false;
    this.keys = this.keys.filter((key) => listed.has(key));
    return true;
  }

  /** The chosen jobs as the list holds them, in the order of the list. */
  jobs(rows: readonly JobView[]): JobView[] {
    const chosen = this.#chosen;
    return rows.filter((row) => chosen.has(keyOf(row.key)));
  }
}

export const selection = new Selection();
