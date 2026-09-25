// Short confirmations whose result is not visible otherwise (saved, copied, files written,
// run finished). At most three at once. A plain one leaves after --dur-toast (4 s); one
// with an undo stays --dur-toast-action (10 s), long enough to read and reach it. A toast
// waits while the pointer is on it, and every toast waits while the window is in the back
// or a modal dialog is open (`hold`): its time only runs while the user can act on it.
// Anything that needs an action stays inline where it belongs; the one exception is an
// undo of what the user just did (a job moved), which the toast may carry; results of the
// same kind in quick succession merge into one toast with one undo (`undoable`). Ctrl/Cmd+Z
// takes back the newest result, also when it merged into a toast that came up earlier. An
// undo names the jobs it concerns: when they are deleted for good, it goes (`forget`).

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

/** An undo; it may take a while (the next one of a merged toast waits for it). */
export type Undo = () => void | Promise<void>;

/** What a mergeable toast adds up: how many, their undos, when the last one came. */
interface Merged {
  kind: string;
  count: number;
  undos: Undo[];
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
  /** When each toast last got a result (a counter): Ctrl/Cmd+Z takes back the newest. */
  #latest = new Map<number, number>();
  #results = 0;
  /** The jobs (keyOf) each undo of a toast concerns. */
  #keys = new Map<number, Set<string>>();
  /** Why the toasts wait; each hold is released on its own. */
  #holds = new Set<symbol>();

  /** A toast; `keys` are the jobs (keyOf) its undo concerns. */
  show(
    text: string,
    tone: ToastTone = 'success',
    action: ToastAction | null = null,
    keys: readonly string[] = [],
  ): number {
    const id = this.#next++;
    this.items = [...this.items, { id, text, tone, action, round: 0 }];
    // Too many: the oldest without an action goes first, so a tip never takes an undo away.
    while (this.items.length > MAX) {
      const plain = this.items.find((item) => item.action === null && item.id !== id);
      this.dismiss((plain ?? this.items[0]!).id);
    }
    this.#timers.set(id, { timer: null, left: lifetime(action), since: 0 });
    this.#latest.set(id, ++this.#results);
    if (keys.length > 0) this.#keys.set(id, new Set(keys));
    this.#start(id);
    return id;
  }

  /**
   * A result the user may take back (jobs archived, deleted, restored). One of the same
   * `kind` within 2 s joins the toast that is up: `text(n)` says how many ("„Titel“
   * archiviert." for one, "2 Jobs archiviert." for more), the one undo (`undoLabel`) takes
   * back all of them, the last result first, and the toast stays its full time from the last
   * one. `count` is how many jobs this result moved, `keys` which ones.
   */
  undoable(
    kind: string,
    text: (n: number) => string,
    undoLabel: string,
    undo: Undo,
    count = 1,
    keys: readonly string[] = [],
  ): void {
    const now = Date.now();
    const open = [...this.#merged.entries()].find(
      ([id, merged]) =>
        merged.kind === kind &&
        now - merged.at < MERGE_MS &&
        this.items.some((item) => item.id === id),
    );
    if (open === undefined) {
      const merged: Merged = { kind, count, undos: [undo], at: now };
      const id = this.show(
        text(count),
        'success',
        {
          label: undoLabel,
          // The last result first, like an undo stack (each undo finds the list as it was).
          onclick: () => {
            void (async () => {
              for (const each of [...merged.undos].reverse()) await each();
            })();
          },
        },
        keys,
      );
      this.#merged.set(id, merged);
      return;
    }
    const [id, merged] = open;
    merged.count += count;
    merged.undos.push(undo);
    merged.at = now;
    this.#latest.set(id, ++this.#results);
    const known = new Set([...(this.#keys.get(id) ?? []), ...keys]);
    if (known.size > 0) this.#keys.set(id, known);
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

  /** Ctrl/Cmd+Z (lib/input/input.ts): the newest undo that is still up runs, as its button
   *  would; `true` if there was one. */
  undoLast(): boolean {
    const latest = (item: ToastItem): number => this.#latest.get(item.id) ?? 0;
    const newest = this.items
      .filter((item) => item.action !== null)
      .reduce<ToastItem | null>(
        (best, item) => (best === null || latest(item) > latest(best) ? item : best),
        null,
      );
    if (!newest?.action) return false;
    newest.action.onclick();
    this.dismiss(newest.id);
    return true;
  }

  dismiss(id: number): void {
    this.#stop(id);
    this.#timers.delete(id);
    this.#merged.delete(id);
    this.#hovered.delete(id);
    this.#latest.delete(id);
    this.#keys.delete(id);
    this.items = this.items.filter((item) => item.id !== id);
  }

  /** Jobs deleted for good: an undo that concerned only them can do nothing any more. */
  forget(deleted: ReadonlySet<string>): void {
    for (const item of this.items) {
      const keys = this.#keys.get(item.id);
      if (item.action && keys && [...keys].every((key) => deleted.has(key))) {
        this.dismiss(item.id);
      }
    }
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
