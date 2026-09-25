// Which of the three views is shown. No router: the app has exactly these three. The native
// menu may ask for one too (macOS: Cmd+, opens the settings). A view with unsaved work (the
// Profil editor) holds a guard: it may keep the switch and ask first, then switch itself;
// what was to happen with the switch (the place a click in the sidebar chose) waits for it.

import { onNavigate } from '../ipc/api';

export type ViewId = 'jobs' | 'profile' | 'settings';

export const VIEW_IDS: readonly ViewId[] = ['jobs', 'profile', 'settings'];

const isView = (value: string): value is ViewId => (VIEW_IDS as readonly string[]).includes(value);

/** `true` lets the switch to `next` happen; `false` keeps the current view. */
export type LeaveGuard = (next: ViewId) => boolean;

class Navigation {
  current = $state<ViewId>('jobs');
  #installed = false;
  #guard: LeaveGuard | null = null;
  /** What waits for a switch the guard kept (it runs once that switch happens). */
  #pending: { view: ViewId; then: () => void } | null = null;

  /**
   * Switch views; the guard of the current view may keep it (unless `force`). `then` runs
   * once the switch has happened (at once, or after the guard's question); the view that is
   * already current switches at once. Returns whether it happened now.
   */
  go(view: ViewId, force = false, then?: () => void): boolean {
    if (!force && view !== this.current && this.#guard !== null && !this.#guard(view)) {
      this.#pending = then === undefined ? null : { view, then };
      return false;
    }
    const pending = this.#pending;
    this.#pending = null;
    this.current = view;
    (then ?? (pending?.view === view ? pending.then : undefined))?.();
    return true;
  }

  /** The current view's guard; returns the function that removes it again. */
  guard(guard: LeaveGuard): () => void {
    this.#guard = guard;
    return () => {
      if (this.#guard === guard) this.#guard = null;
    };
  }

  /** Follow the native menu (App.svelte, once). */
  install(): void {
    if (this.#installed) return;
    this.#installed = true;
    onNavigate((view) => {
      if (isView(view)) this.go(view);
    });
  }
}

export const navigation = new Navigation();
