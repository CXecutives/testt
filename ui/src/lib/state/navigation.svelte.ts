// Which of the three views is shown. No router: the app has exactly these three. The native
// menu may ask for one too (macOS: Cmd+, opens the settings).

import { onNavigate } from '../ipc/api';

export type ViewId = 'jobs' | 'profile' | 'settings';

export const VIEW_IDS: readonly ViewId[] = ['jobs', 'profile', 'settings'];

const isView = (value: string): value is ViewId => (VIEW_IDS as readonly string[]).includes(value);

class Navigation {
  current = $state<ViewId>('jobs');
  #installed = false;

  go(view: ViewId): void {
    this.current = view;
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
