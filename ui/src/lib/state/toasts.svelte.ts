// Short confirmations whose result is not visible otherwise (saved, copied, files written,
// run finished). At most three at once. A plain one leaves after --dur-toast (4 s); one
// with an undo stays --dur-toast-action (10 s), long enough to read and reach it. A toast
// waits while the pointer is on it, and every toast waits while the window is in the back
// or a modal dialog is open (`hold`): its time only runs while the user can act on it.
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
  /** Counts up when a merged result starts the toast's time again (its line restarts). */
  round: number;
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

interface Timer {
  timer: ReturnType<typeof setTimeout> | null;
  /** Time left (ms) while the timer does not run. */
  left: number;
  since: number;
}

/** How long a toast stays: longer when it carries an undo. */
function lifetime(action: ToastAction | null): number {
  return tokenMs(action === null ? '--dur-toast' : '--dur-toast-action');
}

class Toasts {
  items = $state<ToastItem[]>([]);
  /** Every toast waits (the window is in the back, a modal dialog is open): for the view. */
  held = $state(false);
  #next = 1;
  #timers = new Map<number, Timer>();
  #merged = new Map<number, Merged>();
  /** Toasts under the pointer. */
  #hovered = new Set<number>();
  /** Why the toasts wait; each hold is released on its own. */
  #holds = new Set<symbol>();

  show(text: string, tone: ToastTone = 'success', action: ToastAction | null = null): number {
    const id = this.#next++;
    this.items = [...this.items, { id, text, tone, action, round: 0 }];
    while (this.items.length > MAX) this.dismiss(this.items[0]!.id);
    this.#timers.set(id, { timer: null, left: lifetime(action), since: 0 });
    this.#start(id);
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
      item.id === id ? { ...item, text: text(merged.count), round: item.round + 1 } : item,
    );
    // The toast stays its full time from the last result.
    const entry = this.#timers.get(id);
    if (entry) {
      this.#stop(id);
      const item = this.items.find((each) => each.id === id);
      entry.left = lifetime(item?.action ?? null);
      this.#start(id);
    }
  }

  dismiss(id: number): void {
    this.#stop(id);
    this.#timers.delete(id);
    this.#merged.delete(id);
    this.#hovered.delete(id);
    this.items = this.items.filter((item) => item.id !== id);
  }

  /** Hovered: the toast stays. */
  pause(id: number): void {
    this.#hovered.add(id);
    this.#stop(id);
  }

  resume(id: number): void {
    this.#hovered.delete(id);
    this.#start(id);
  }

  /**
   * Every toast waits until the returned release is called: while the window is in the
   * back (Toast.svelte) or a modal dialog is open (Dialog.svelte), so an undo cannot run
   * out while the user cannot reach it.
   */
  hold(): () => void {
    // Only the plain set decides (reading `held` here would make a calling effect depend
    // on what it writes).
    const reason = Symbol('hold');
    this.#holds.add(reason);
    if (this.#holds.size === 1) {
      this.held = true;
      for (const id of this.#timers.keys()) this.#stop(id);
    }
    return () => {
      if (!this.#holds.delete(reason) || this.#holds.size > 0) return;
      this.held = false;
      for (const id of this.#timers.keys()) this.#start(id);
    };
  }

  /** The time runs (unless the toast is hovered or all toasts are held). */
  #start(id: number): void {
    const entry = this.#timers.get(id);
    if (!entry || entry.timer !== null || this.#holds.size > 0 || this.#hovered.has(id)) return;
    entry.since = Date.now();
    entry.timer = setTimeout(() => this.dismiss(id), entry.left);
  }

  /** The time stops where it is. */
  #stop(id: number): void {
    const entry = this.#timers.get(id);
    if (!entry || entry.timer === null) return;
    clearTimeout(entry.timer);
    entry.timer = null;
    entry.left = Math.max(0, entry.left - (Date.now() - entry.since));
  }
}

export const toasts = new Toasts();
