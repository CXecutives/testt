// The runs, built from the run events (api.ts: one channel per command call, all fanned out
// to `onRun`). Every run begins with `started` and its kind, so the page follows the runs
// the app starts by itself as what they are: the auto fetch at the start, a rescore after a
// profile or workspace change or an engine update. After a reload the snapshot of
// `app_state` replays the events that describe the current state.
//
// Three things are kept apart:
// - the run in progress (`active`, `kind`, steps, status, portal health), of any kind;
// - `summary`: the last finished fetch of this session (fetch or whole mailbox). It means the
//   same as `app.state.lastRun` ("the last fetch"), so `run.summary ?? app.state.lastRun` is
//   the last fetch wherever it is read (sidebar, failed-fetch retry, empty list);
// - `result` and `history`: what the run card shows, the last fetch or details run the card
//   followed. A rescore opens no run card and brings no fetch news; only when it failed or
//   could not write the files the card says so.

import { t } from '../i18n/t';
import { errorText } from '../i18n/texts';
import { invoke, IpcError, onRun } from '../ipc/api';
import type {
  ErrorInfo,
  Portal,
  PortalHealth,
  RunEvent,
  RunKindName,
  RunRequest,
  RunSnapshot,
  RunSummary,
  StatusCode,
  Step,
} from '../ipc/types';
import { app } from './app.svelte';
import { navigation } from './navigation.svelte';
import { toasts } from './toasts.svelte';

export const STEPS: readonly Step[] = ['scan', 'fetch', 'score'];
const ORDER: readonly Step[] = ['scan', 'fetch', 'score', 'export'];
/** The steps each kind goes through (a details run reads no mail, a rescore only scores). */
const KIND_STEPS: Record<RunKindName, readonly Step[]> = {
  fetch: STEPS,
  fullMailbox: STEPS,
  details: ['fetch', 'score'],
  rescore: ['score'],
};
const HISTORY_MAX = 200;
const SECOND = 1000;

export interface Progress {
  done: number;
  total: number;
}

export interface HistoryLine {
  at: number;
  text: string;
}

/**
 * A line of the history. Its words are made when it is shown, so after a switch of the
 * language the history reads in the new one (a class: `$state` leaves its instances as they
 * are, with the getter).
 */
class Line implements HistoryLine {
  constructor(
    readonly at: number,
    private readonly say: () => string,
  ) {}

  get text(): string {
    return this.say();
  }
}

/** A mailbox run: what "the last fetch" means (`app.state.lastRun`). */
export const isFetch = (kind: RunKindName): boolean => kind === 'fetch' || kind === 'fullMailbox';

class RunStore {
  active = $state(false);
  kind = $state<RunKindName | null>(null);
  step = $state<Step | null>(null);
  progress = $state<Partial<Record<Step, Progress>>>({});
  status = $state<{ code: StatusCode; portal: Portal | null; until: string | null } | null>(null);
  health = $state<Partial<Record<Portal, PortalHealth>>>({});
  loginNeeded = $state<Portal | null>(null);
  /** The last finished fetch of this session (see above). */
  summary = $state<RunSummary | null>(null);
  /** The finished run the run card shows (a fetch or details run, a rescore in trouble). */
  result = $state<RunSummary | null>(null);
  /** The history of the run the card shows. */
  history = $state<HistoryLine[]>([]);
  /** An error of `start_run` itself (busy, no mailbox ...); said in the current language. */
  #startFailure = $state.raw<{ error: unknown } | null>(null);
  /** `start_run` is on its way: nothing is known yet (the first-run page stays until then). */
  starting = $state(false);
  cancelling = $state(false);
  /** The run card above the list: open while running and right after, collapsible. */
  panel = $state<'open' | 'collapsed' | 'hidden'>('hidden');
  /** Ticks every second while a countdown is shown. */
  now = $state(Date.now());

  #ticker: ReturnType<typeof setInterval> | null = null;
  #listeners = new Set<(event: RunEvent) => void>();
  #installed = false;
  /** The last run the page started (a retry starts it again). */
  #request: RunRequest | null = null;
  /** Counts the runs begun, so a failed start leaves a run that began meanwhile alone. */
  #epoch = 0;
  /** A `started` came through the channel: the page follows the runs live. */
  #followed = false;
  /** The history of a rescore: shown only if the card takes it on. */
  #quiet: HistoryLine[] = [];

  /** Subscribe once to the run channel (App.svelte). */
  install(): void {
    if (this.#installed) return;
    this.#installed = true;
    onRun((event) => this.handle(event));
  }

  /** Other stores that react to run events (the job list). */
  listen(listener: (event: RunEvent) => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /**
   * The run in progress at the first load (a reload of the page). A run whose `started`
   * already came through the channel is followed live; its snapshot could be older than
   * what arrived since (even its end).
   */
  attach(snapshot: RunSnapshot | null): void {
    if (snapshot === null || this.#followed) return;
    this.begin(snapshot.kind);
    for (const event of snapshot.replay) this.handle(event, false);
  }

  /** A run the run card shows is going (a fetch or details run, never a rescore). */
  get fetching(): boolean {
    return this.active && this.kind !== 'rescore';
  }

  /** The words of a failed `start_run`, or null. */
  get startError(): string | null {
    return this.#startFailure === null ? null : errorText(this.#startFailure.error);
  }

  /** Why an action waits while a run goes. */
  get busyText(): string {
    return this.kind === 'rescore' ? t.run.rescoring : t.settings.running;
  }

  /** The steps of the run in progress. */
  get steps(): readonly Step[] {
    return KIND_STEPS[this.kind ?? 'fetch'];
  }

  private begin(kind: RunKindName): void {
    this.#epoch += 1;
    this.active = true;
    this.kind = kind;
    this.step = null;
    this.progress = {};
    this.status = null;
    this.health = {};
    this.loginNeeded = null;
    this.cancelling = false;
    this.tick(false);
    if (kind === 'rescore') {
      // The card keeps showing the last fetch.
      this.#quiet = [];
      return;
    }
    this.result = null;
    this.history = [];
    this.#startFailure = null;
    if (this.panel === 'hidden') this.panel = 'open';
  }

  async start(request: RunRequest): Promise<boolean> {
    if (this.active) return false;
    // Nothing is lost when the start fails: the card shows what it showed before.
    const before = { result: this.result, history: this.history, panel: this.panel };
    this.#request = request;
    this.starting = true;
    this.begin(request.kind);
    const epoch = this.#epoch;
    try {
      await invoke('start_run', { request });
      return true;
    } catch (error) {
      if (this.#epoch === epoch) {
        this.active = false;
        this.kind = null;
        this.result = before.result;
        this.history = before.history;
        this.panel = before.panel;
      }
      this.#startFailure = { error };
      if (error instanceof IpcError && error.kind === 'busy') void app.load();
      return false;
    } finally {
      this.starting = false;
    }
  }

  /** Start a finished run again: the same request (a details run with its jobs). */
  retry(summary: RunSummary | null): void {
    const kind = summary?.kind ?? 'fetch';
    const same = this.#request !== null && this.#request.kind === kind ? this.#request : null;
    void this.start(same ?? (kind === 'details' ? { kind: 'fetch' } : { kind }));
  }

  /** Close the run card (and the note of a failed start with it). */
  hide(): void {
    this.panel = 'hidden';
    this.#startFailure = null;
  }

  async cancel(): Promise<void> {
    if (!this.active || this.cancelling) return;
    this.cancelling = true;
    try {
      await invoke('cancel_run');
    } catch (error) {
      this.cancelling = false;
      this.#startFailure = { error };
    }
  }

  /** Remaining milliseconds of the current pause, or null. */
  get waitLeft(): number | null {
    const until = this.status?.code === 'waiting' ? this.status.until : null;
    if (until === null) return null;
    return Math.max(0, new Date(until).getTime() - this.now);
  }

  /** Overall progress of the current step (null = indeterminate). */
  get fraction(): number | null {
    const step = this.step;
    if (step === null) return null;
    const p = this.progress[step];
    if (!p || p.total === 0) return null;
    return p.done / p.total;
  }

  stepState(step: Step): 'done' | 'current' | 'waiting' {
    if (!this.active) return this.result === null ? 'waiting' : 'done';
    const current = this.step === null ? -1 : ORDER.indexOf(this.step);
    const index = ORDER.indexOf(step);
    return index < current ? 'done' : index === current ? 'current' : 'waiting';
  }

  /** Adds a line to the history; `say` makes its words whenever it is shown. */
  private log(say: () => string): void {
    const line = new Line(Date.now(), say);
    if (this.kind === 'rescore') {
      this.#quiet = [...this.#quiet, line].slice(-HISTORY_MAX);
      return;
    }
    const next = [...this.history, line];
    this.history = next.length > HISTORY_MAX ? next.slice(-HISTORY_MAX) : next;
  }

  private tick(on: boolean): void {
    if (on && this.#ticker === null) {
      this.now = Date.now();
      this.#ticker = setInterval(() => (this.now = Date.now()), SECOND);
    } else if (!on && this.#ticker !== null) {
      clearInterval(this.#ticker);
      this.#ticker = null;
    }
  }

  handle(event: RunEvent, live = true): void {
    switch (event.type) {
      case 'started':
        if (!live) break;
        this.#followed = true;
        // The page's own start began it already; any other run is one the app started.
        if (!this.active || this.kind !== event.kind) this.begin(event.kind);
        break;
      case 'progress': {
        if (!this.active) break;
        this.step = event.step;
        // The step's progress counts over all portals (the backend sends `portal: null`).
        if (event.portal === null) {
          this.progress = {
            ...this.progress,
            [event.step]: { done: event.done, total: event.total },
          };
        }
        break;
      }
      case 'status': {
        if (!this.active) break;
        const changed = this.status?.code !== event.code || this.status?.portal !== event.portal;
        this.status = { code: event.code, portal: event.portal, until: event.until };
        this.tick(event.code === 'waiting' && event.until !== null);
        if (changed) this.log(() => t.run.statusOf(event.code, event.portal));
        break;
      }
      case 'alert':
        if (this.active) this.log(() => t.run.alert(event.portal, event.postings));
        break;
      case 'portalHealth':
        app.setHealth(event.portal, event.health);
        if (!this.active) break;
        this.health = { ...this.health, [event.portal]: event.health };
        if (event.health.kind !== 'ok') {
          const kind = event.health.kind;
          this.log(() => t.run.health(event.portal, kind));
        }
        break;
      case 'loginNeeded':
        this.loginNeeded = event.waiting ? event.portal : null;
        break;
      case 'finished':
        this.finish(event.summary, live);
        break;
      case 'jobUpdated':
        break;
    }
    if (live) for (const listener of this.#listeners) listener(event);
  }

  private finish(summary: RunSummary, live: boolean): void {
    const kind = summary.kind;
    this.active = false;
    this.kind = kind;
    this.cancelling = false;
    this.status = null;
    this.loginNeeded = null;
    this.tick(false);
    if (kind === 'rescore') {
      // Quiet unless something needs attention: then the card says it.
      const trouble = summary.outcome.kind === 'failed' || exportError(summary) !== null;
      if (trouble) {
        this.result = summary;
        this.history = [...this.#quiet, new Line(Date.now(), () => outcomeText(summary))];
        if (live) this.panel = 'open';
      }
      this.#quiet = [];
    } else {
      if (isFetch(kind)) this.summary = summary;
      this.result = summary;
      this.log(() => outcomeText(summary));
      if (live && this.panel === 'hidden') this.panel = 'open';
    }
    if (!live) return;
    // In the Jobs view the run card says it; elsewhere a toast brings the news. A rescore
    // speaks where it was started (the Profil view), not as a fetch.
    if (summary.outcome.kind === 'completed' && navigation.current !== 'jobs') {
      if (isFetch(kind)) {
        toasts.show(
          exportError(summary) === null
            ? t.toast.runDone(summary.newJobs?.count ?? 0)
            : t.toast.runDoneFilesOld,
        );
      } else if (kind === 'rescore' && navigation.current !== 'profile') {
        toasts.show(t.toast.rescored);
      }
    }
    void app.load();
  }
}

/** The export error of a finished run, if its files could not all be written. */
export function exportError(summary: RunSummary): ErrorInfo | null {
  return summary.export?.error ?? null;
}

/** The title of a finished run: done, cancelled or failed, in the words of its kind. */
export function outcomeText(summary: RunSummary): string {
  const outcome = summary.outcome.kind;
  switch (summary.kind) {
    case 'rescore':
      return outcome === 'completed'
        ? t.run.rescored
        : outcome === 'cancelled'
          ? t.run.rescore.cancelled
          : t.run.rescore.failed;
    case 'details': {
      if (outcome === 'cancelled') return t.run.details.cancelled;
      if (outcome === 'failed') return t.run.details.failed;
      const fetched = summary.perPortal.reduce((sum, p) => sum + p.fetched, 0);
      return fetched > 0 ? t.run.details.done : t.run.details.none;
    }
    default:
      return outcome === 'completed'
        ? t.run.done
        : outcome === 'cancelled'
          ? t.run.cancelled
          : t.run.failed;
  }
}

export const run = new RunStore();
