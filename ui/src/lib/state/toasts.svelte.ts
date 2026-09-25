// Short confirmations whose result is not visible otherwise (saved, copied, files written,
// run finished). At most three at once, each leaves after --dur-toast unless hovered.
// Anything that needs an action stays inline where it belongs; the one exception is an
// undo of what the user just did (a job moved), which the toast may carry; results of the
// same kind in quick succession merge into one toast with one undo (`undoable`).

import { tokenMs } from '../tokens';

export type ToastTone = 'success' | 'info';

/** An undo of what the user just did; clicking it also closes the toast. */
export interface ToastAction {
  label: string;
  onclick: () => void;
}

export interface ToastItem {
  id: number;
  text: string;
  tone: ToastTone;
  action: ToastAction | null;
}

const MAX = 3;
/** A result of the same kind within this time joins the toast that is up ("2 Jobs
 *  archiviert."), whose one undo takes back all of them. */
const MERGE_MS = 2000;

/** What a mergeable toast adds up: how many, their undos, when the last one came. */
interface Merged {
  kind: string;
  count: number;
  undos: (() => void)[];
  at: number;
}

class Toasts {
  items = $state<ToastItem[]>([]);
  #next = 1;
  #timers = new Map<
    number,
    { timer: ReturnType<typeof setTimeout> | null; left: number; since: number }
  >();

  #merged = new Map<number, Merged>();

  show(text: string, tone: ToastTone = 'success', action: ToastAction | null = null): number {
    const id = this.#next++;
    this.items = [...this.items, { id, text, tone, action }];
    while (this.items.length > MAX) this.dismiss(this.items[0]!.id);
    this.#timers.set(id, { timer: null, left: tokenMs('--dur-toast'), since: 0 });
    this.resume(id);
    return id;
  }

  /**
   * A result the user may take back (a job archived, deleted, restored). One of the same
   * `kind` within 2 s joins the toast that is up: `text(n)` says how many ("„Titel“
   * archiviert." for one, "2 Jobs archiviert." for more), the one undo (`undoLabel`) takes
   * back all of them, and the toast stays its full time from the last one.
   */
  undoable(kind: string, text: (n: number) => string, undoLabel: string, undo: () => void): void {
    const now = Date.now();
    const open = [...this.#merged.entries()].find(
      ([id, merged]) =>
        merged.kind === kind &&
        now - merged.at < MERGE_MS &&
        this.items.some((item) => item.id === id),
    );
    if (open === undefined) {
      const merged: Merged = { kind, count: 1, undos: [undo], at: now };
      const id = this.show(text(1), 'success', {
        label: undoLabel,
        // The last result first, like an undo stack (each undo finds the list as it was).
        onclick: () => {
          for (const each of [...merged.undos].reverse()) each();
        },
      });
      this.#merged.set(id, merged);
      return;
    }
    const [id, merged] = open;
    merged.count += 1;
    merged.undos.push(undo);
    merged.at = now;
    this.items = this.items.map((item) =>
      item.id === id ? { ...item, text: text(merged.count) } : item,
    );
    // The toast stays its full time from the last result.
    const entry = this.#timers.get(id);
    if (entry) {
      if (entry.timer !== null) clearTimeout(entry.timer);
      entry.timer = null;
      entry.left = tokenMs('--dur-toast');
      this.resume(id);
    }
  }

  dismiss(id: number): void {
    const entry = this.#timers.get(id);
    if (entry?.timer) clearTimeout(entry.timer);
    this.#timers.delete(id);
    this.#merged.delete(id);
    this.items = this.items.filter((item) => item.id !== id);
  }

  /** Hovered: the toast stays. */
  pause(id: number): void {
    const entry = this.#timers.get(id);
    if (!entry || entry.timer === null) return;
    clearTimeout(entry.timer);
    entry.timer = null;
    entry.left = Math.max(0, entry.left - (Date.now() - entry.since));
  }

  resume(id: number): void {
    const entry = this.#timers.get(id);
    if (!entry || entry.timer !== null) return;
    entry.since = Date.now();
    entry.timer = setTimeout(() => this.dismiss(id), entry.left);
  }
}

export const toasts = new Toasts();
