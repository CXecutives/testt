// The rows chosen like in a mail app: Ctrl+click (Cmd on macOS) takes a row in or out, Shift+
// click takes the range from the last clicked row; a plain click chooses one row and opens it.
// Two or more chosen rows show the selection bar in the list header; Esc clears them. The
// open job belongs to a selection that starts from it.

import type { JobView } from '$lib/ipc/types';
import { jobs, keyOf } from '$lib/state/jobs.svelte';

class Selection {
  /** The chosen rows (keyOf), in the order they were taken. */
  keys = $state<string[]>([]);
  /** Where a Shift range starts (the last row clicked). */
  #anchor: string | null = null;

  get size(): number {
    return this.keys.length;
  }

  has(job: JobView): boolean {
    return this.keys.includes(keyOf(job.key));
  }

  clear(): void {
    this.keys = [];
    this.#anchor = null;
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
  }

  /** Shift+click: every row from the anchor to this one, in the order of the list. */
  range(job: JobView, rows: readonly JobView[]): void {
    const order = rows.map((row) => keyOf(row.key));
    const key = keyOf(job.key);
    const from = order.indexOf(this.#anchor ?? (jobs.selected ? keyOf(jobs.selected) : key));
    const to = order.indexOf(key);
    if (from === -1 || to === -1) {
      this.toggle(job);
      return;
    }
    this.keys = order.slice(Math.min(from, to), Math.max(from, to) + 1);
  }

  /** The chosen jobs as the list holds them, in the order of the list. */
  jobs(rows: readonly JobView[]): JobView[] {
    return rows.filter((row) => this.keys.includes(keyOf(row.key)));
  }
}

export const selection = new Selection();
