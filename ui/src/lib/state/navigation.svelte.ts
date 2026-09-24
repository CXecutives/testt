// Which of the three views is shown. No router: the app has exactly these three. The native
// menu may ask for one too (macOS: Cmd+, opens the settings). A view with unsaved work (the
// Profil editor) holds a guard: it may keep the switch and ask first, then switch itself.

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

  /** Switch views; the guard of the current view may keep it (unless `force`). */
  go(view: ViewId, force = false): void {
    if (!force && view !== this.current && this.#guard !== null && !this.#guard(view)) return;
    this.current = view;
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
