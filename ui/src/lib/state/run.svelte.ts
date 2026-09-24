// The run in progress, built from the events of the one run channel: steps with progress,
// one line per portal, the current status (with the countdown of a pause), portal health,
// a short history and the summary of the finished run. After a reload the snapshot of
// `app_state` replays the events that describe the current state.

import { de } from '../i18n/de';
import { errorText } from '../i18n/texts';
import { invoke, IpcError, onRun } from '../ipc/api';
import type {
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
import { toasts } from './toasts.svelte';

export const STEPS: readonly Step[] = ['scan', 'fetch', 'score'];
const ORDER: readonly Step[] = ['scan', 'fetch', 'score', 'export'];
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

class RunStore {
  active = $state(false);
  kind = $state<RunKindName | null>(null);
  step = $state<Step | null>(null);
  progress = $state<Partial<Record<Step, Progress>>>({});
  portals = $state<Partial<Record<Portal, Progress>>>({});
  status = $state<{ code: StatusCode; portal: Portal | null; until: string | null } | null>(null);
  health = $state<Partial<Record<Portal, PortalHealth>>>({});
  loginNeeded = $state<Portal | null>(null);
  history = $state<HistoryLine[]>([]);
  summary = $state<RunSummary | null>(null);
  /** An error of `start_run` itself (busy, no mailbox ...). */
  startError = $state<string | null>(null);
  cancelling = $state(false);
  /** The run card above the list: open while running and right after, collapsible. */
  panel = $state<'open' | 'collapsed' | 'hidden'>('hidden');
  /** Ticks every second while a countdown is shown. */
  now = $state(Date.now());

  #ticker: ReturnType<typeof setInterval> | null = null;
  #listeners = new Set<(event: RunEvent) => void>();
  #installed = false;

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

  attach(snapshot: RunSnapshot | null): void {
    if (snapshot === null) return;
    this.reset(snapshot.kind);
    for (const event of snapshot.replay) this.handle(event, false);
  }

  reset(kind: RunKindName): void {
    this.active = true;
    this.kind = kind;
    this.step = null;
    this.progress = {};
    this.portals = {};
    this.status = null;
    this.health = {};
    this.loginNeeded = null;
    this.history = [];
    this.summary = null;
    this.startError = null;
    this.cancelling = false;
    if (this.panel === 'hidden') this.panel = 'open';
  }

  async start(request: RunRequest): Promise<boolean> {
    if (this.active) return false;
    this.reset(request.kind);
    try {
      await invoke('start_run', { request });
      return true;
    } catch (error) {
      this.active = false;
      this.kind = null;
      this.startError = errorText(error);
      if (error instanceof IpcError && error.kind === 'busy') void app.load();
      return false;
    }
  }

  async cancel(): Promise<void> {
    if (!this.active || this.cancelling) return;
    this.cancelling = true;
    try {
      await invoke('cancel_run');
    } catch (error) {
      this.cancelling = false;
      this.startError = errorText(error);
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
    if (!this.active) return this.summary === null ? 'waiting' : 'done';
    const current = this.step === null ? -1 : ORDER.indexOf(this.step);
    const index = ORDER.indexOf(step);
    return index < current ? 'done' : index === current ? 'current' : 'waiting';
  }

  private log(text: string): void {
    const next = [...this.history, { at: Date.now(), text }];
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
      case 'progress': {
        if (!this.active && live) this.reset(this.kind ?? 'fetch');
        this.step = event.step;
        this.progress = {
          ...this.progress,
          [event.step]: { done: event.done, total: event.total },
        };
        if (event.portal !== null && event.step === 'fetch') {
          this.portals = {
            ...this.portals,
            [event.portal]: { done: event.done, total: event.total },
          };
        }
        break;
      }
      case 'status': {
        const changed = this.status?.code !== event.code || this.status?.portal !== event.portal;
        this.status = { code: event.code, portal: event.portal, until: event.until };
        this.tick(event.code === 'waiting' && event.until !== null);
        if (changed) {
          const portal = event.portal ? ` · ${de.portal[event.portal]}` : '';
          this.log(`${de.run.status[event.code]}${portal}`);
        }
        break;
      }
      case 'alert':
        this.log(de.run.alert(event.portal, event.postings));
        break;
      case 'portalHealth':
        this.health = { ...this.health, [event.portal]: event.health };
        app.setHealth(event.portal, event.health);
        if (event.health.kind !== 'ok') this.log(de.run.health(event.portal));
        break;
      case 'loginNeeded':
        this.loginNeeded = event.waiting ? event.portal : null;
        break;
      case 'finished':
        this.summary = event.summary;
        this.kind = event.summary.kind;
        this.active = false;
        this.cancelling = false;
        this.status = null;
        this.tick(false);
        this.log(
          event.summary.outcome.kind === 'completed'
            ? de.run.done
            : event.summary.outcome.kind === 'cancelled'
              ? de.run.cancelled
              : de.run.failed,
        );
        if (live) {
          if (this.panel === 'hidden') this.panel = 'open';
          if (event.summary.outcome.kind === 'completed') {
            toasts.show(de.toast.runDone(this.newJobs));
          }
          void app.load();
        }
        break;
      case 'jobUpdated':
        break;
    }
    if (live) for (const listener of this.#listeners) listener(event);
  }

  /** New jobs of the finished run over all portals. */
  get newJobs(): number {
    return this.summary?.perPortal.reduce((sum, p) => sum + p.new, 0) ?? 0;
  }
}

export const run = new RunStore();
