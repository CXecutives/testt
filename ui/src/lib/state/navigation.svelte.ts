// Which of the three views is shown. No router: the app has exactly these three.

export type ViewId = 'jobs' | 'profile' | 'settings';

export const VIEW_IDS: readonly ViewId[] = ['jobs', 'profile', 'settings'];

class Navigation {
  current = $state<ViewId>('jobs');

  go(view: ViewId): void {
    this.current = view;
  }
}

export const navigation = new Navigation();
