// Short confirmations whose result is not visible otherwise (saved, copied, files written,
// run finished). At most three at once, each leaves after --dur-toast unless hovered.
// Anything that needs an action stays inline where it belongs; the one exception is an
// undo of what the user just did (a hidden job), which the toast may carry.

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

class Toasts {
  items = $state<ToastItem[]>([]);
  #next = 1;
  #timers = new Map<
    number,
    { timer: ReturnType<typeof setTimeout> | null; left: number; since: number }
  >();

  show(text: string, tone: ToastTone = 'success', action: ToastAction | null = null): void {
    const id = this.#next++;
    this.items = [...this.items, { id, text, tone, action }];
    while (this.items.length > MAX) this.dismiss(this.items[0]!.id);
    this.#timers.set(id, { timer: null, left: tokenMs('--dur-toast'), since: 0 });
    this.resume(id);
  }

  dismiss(id: number): void {
    const entry = this.#timers.get(id);
    if (entry?.timer) clearTimeout(entry.timer);
    this.#timers.delete(id);
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
