// Typed stand-in for @tauri-apps/api in the harness build (`vite build --mode harness`
// aliases every `@tauri-apps/api/*` import to this file). It answers every IPC v3 command
// with fixed sample data, records the calls and lets a test push run events:
//
//   window.__harness.calls          [command, args][]
//   window.__harness.emit(event)    deliver a RunEvent on every open channel
//
// Dates are fixed so screenshots stay stable.

import type {
  AppState,
  CommandArgs,
  CommandName,
  CommandResult,
  JobDetail,
  JobView,
  ProfileInfo,
  RunEvent,
} from '../../ui/src/lib/ipc/types';

interface Harness {
  calls: [string, unknown][];
  emit: (event: RunEvent) => void;
}

declare global {
  interface Window {
    __harness: Harness;
  }
}

/* ------------------------------------------------------------------ channel */

const channels = new Set<Channel<RunEvent>>();

export class Channel<T = unknown> {
  onmessage: (message: T) => void = () => undefined;
}

/* ------------------------------------------------------------------ fixtures */

const NOW = '2026-09-24T09:30:00+02:00';

const job = (
  id: string,
  title: string,
  score: number | null,
  extra: Partial<JobView> = {},
): JobView => ({
  key: { portal: 'freelancermap', id },
  portal: 'freelancermap',
  title,
  company: 'Muster GmbH',
  location: 'Hamburg',
  workMode: 'hybrid',
  mailDate: NOW,
  firstSeenAt: NOW,
  unread: true,
  pinned: false,
  detail: 'ok',
  match:
    score === null
      ? null
      : {
          score,
          band: score >= 80 ? 'high' : score >= 40 ? 'mid' : 'low',
          status: 'scored',
          note: null,
          mustMet: 3,
          mustTotal: 4,
          top: [],
        },
  alsoOn: [],
  ...extra,
});

const JOBS: JobView[] = [
  job('1001', 'Interim CFO (m/w/d)', 87),
  job('1002', 'Controlling Lead Transformation', 64),
  job('1003', 'SAP FI/CO Berater', 31),
];

const PROFILE: ProfileInfo = {
  fileName: 'profil.json',
  bytes: 18_422,
  savedAt: NOW,
  quality: 'good',
  understood: { competenceCount: 42, competences: [], sources: [], criteria: [], warnings: [] },
  scoredAt: NOW,
  pending: 0,
};

const APP_STATE: AppState = {
  platform: 'windows',
  dryRun: true,
  firstRun: false,
  running: false,
  settings: { workspace: null, autoFetchOnStart: true },
  mailbox: { address: 'alerts@example.com', connected: true, lastScanAt: NOW },
  profile: PROFILE,
  portals: [],
  autoFetchOnStart: true,
  lastRun: null,
  counts: { new: 3, all: 3, excluded: 0, high: 1, noDetail: 0 },
  topMatches: JOBS.slice(0, 1),
  matchPending: 0,
  dataDir: 'C:/data',
  logDir: 'C:/data/logs',
  resetReport: null,
};

type Handlers = { [K in CommandName]: (args: CommandArgs<K>) => CommandResult<K> };

const handlers: Handlers = {
  app_state: () => APP_STATE,
  start_run: () => null,
  cancel_run: () => null,
  list_jobs: () => ({ jobs: JOBS, counts: APP_STATE.counts }),
  job_detail: ({ key }): JobDetail => ({
    job: JOBS.find((j) => j.key.id === key.id) ?? JOBS[0]!,
    text: null,
    url: 'https://www.freelancermap.de/projekt/1001',
    fetchedAt: NOW,
    mail: { subject: 'Neue Projekte', gmailUrl: null },
    match: null,
  }),
  mark_read: () => true,
  set_pinned: ({ on }) => on,
  pick_profile: () => PROFILE,
  remove_profile: () => true,
  save_profile_template: () => null,
  save_mailbox: ({ address }) => ({ address, connected: true, lastScanAt: null }),
  remove_mailbox: () => true,
  portal_login: () => true,
  portal_logout: () => true,
  pick_workspace: () => null,
  rewrite_txt: () => ({ txtWritten: 3, txtFailed: 0 }),
  clear_txt: () => 3,
  open_target: () => null,
  save_settings: () => APP_STATE,
  reset_all: () => null,
  report_ui_error: () => null,
};

const harness: Harness = {
  calls: [],
  emit(event) {
    for (const channel of channels) channel.onmessage(event);
  },
};
window.__harness = harness;

/* --------------------------------------------------------------------- core */

export async function invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  harness.calls.push([command, args]);
  if (args.channel instanceof Channel) channels.add(args.channel as Channel<RunEvent>);
  const handler = handlers[command as CommandName] as ((a: unknown) => unknown) | undefined;
  if (handler === undefined) throw { kind: 'unknownCommand', params: { command } };
  return handler(args) as T;
}

/* ------------------------------------------------------------------- window */

type ResizeHandler = () => void;

class FakeWindow {
  #maximized = false;
  #resized = new Set<ResizeHandler>();

  async minimize(): Promise<void> {
    harness.calls.push(['window.minimize', null]);
  }

  async toggleMaximize(): Promise<void> {
    harness.calls.push(['window.toggleMaximize', null]);
    this.#maximized = !this.#maximized;
    for (const handler of this.#resized) handler();
  }

  async close(): Promise<void> {
    harness.calls.push(['window.close', null]);
  }

  async startDragging(): Promise<void> {
    harness.calls.push(['window.startDragging', null]);
  }

  async isMaximized(): Promise<boolean> {
    return this.#maximized;
  }

  async onResized(handler: ResizeHandler): Promise<() => void> {
    this.#resized.add(handler);
    return () => this.#resized.delete(handler);
  }
}

const fakeWindow = new FakeWindow();

export function getCurrentWindow(): FakeWindow {
  return fakeWindow;
}
