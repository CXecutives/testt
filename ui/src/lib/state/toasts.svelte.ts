// Short confirmations whose result is not visible otherwise (saved, copied, files written,
// run finished). At most three at once, each leaves after --dur-toast (one with an undo
// after --dur-toast-undo); the time stands still while it is hovered and while the window is
// in the background. Ctrl/Cmd+Z runs the newest undo (`undoLast`, lib/input/input.ts).
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

/** How long a toast stays: longer when it can take something back. */
const lifetime = (action: ToastAction | null): number =>
  tokenMs(action ? '--dur-toast-undo' : '--dur-toast');

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
  /** Hovered toasts (their time stands still). */
  #held = new Set<number>();
  /** The window is in the background: every toast waits. */
  #away = false;

  constructor() {
    if (typeof document === 'undefined') return;
    const root = document.documentElement;
    new MutationObserver(() => this.#window(root.dataset.window !== 'inactive')).observe(root, {
      attributes: true,
      attributeFilter: ['data-window'],
    });
  }

  show(text: string, tone: ToastTone = 'success', action: ToastAction | null = null): number {
    const id = this.#next++;
    this.items = [...this.items, { id, text, tone, action }];
    while (this.items.length > MAX) this.dismiss(this.items[0]!.id);
    this.#timers.set(id, { timer: null, left: lifetime(action), since: 0 });
    this.#start(id);
    return id;
  }

  /** Ctrl/Cmd+Z: the newest undo that is still up runs; `true` if there was one. */
  undoLast(): boolean {
    const newest = [...this.items].reverse().find((item) => item.action !== null);
    if (!newest?.action) return false;
    newest.action.onclick();
    this.dismiss(newest.id);
    return true;
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
      entry.left = tokenMs('--dur-toast-undo');
      this.#start(id);
    }
  }

  dismiss(id: number): void {
    const entry = this.#timers.get(id);
    if (entry?.timer) clearTimeout(entry.timer);
    this.#timers.delete(id);
    this.#merged.delete(id);
    this.#held.delete(id);
    this.items = this.items.filter((item) => item.id !== id);
  }

  /** Hovered: the toast stays. */
  pause(id: number): void {
    this.#held.add(id);
    this.#stop(id);
  }

  resume(id: number): void {
    this.#held.delete(id);
    this.#start(id);
  }

  #stop(id: number): void {
    const entry = this.#timers.get(id);
    if (!entry || entry.timer === null) return;
    clearTimeout(entry.timer);
    entry.timer = null;
    entry.left = Math.max(0, entry.left - (Date.now() - entry.since));
  }

  /** The time runs on, unless the toast is hovered or the window is in the background. */
  #start(id: number): void {
    const entry = this.#timers.get(id);
    if (!entry || entry.timer !== null || this.#away || this.#held.has(id)) return;
    entry.since = Date.now();
    entry.timer = setTimeout(() => this.dismiss(id), entry.left);
  }

  #window(active: boolean): void {
    this.#away = !active;
    for (const id of this.#timers.keys()) {
      if (active) this.#start(id);
      else this.#stop(id);
    }
  }
}

export const toasts = new Toasts();
