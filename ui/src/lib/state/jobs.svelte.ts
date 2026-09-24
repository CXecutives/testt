// The job list, the selection and the reader.
//
// - List and counts come from one `list_jobs` call (pages of 500); the list renders in
//   windows of 60 rows that grow while scrolling, so 2000 jobs never block a frame.
// - During a run new jobs are inserted at the top with a short accent tint and existing
//   rows update in place (rings fill live). The list re-sorts once, when the run finishes,
//   and keeps the selection.
// - `mark_read` only on a real click on a row (select(..., true)).
// - The day overview reads its own unfiltered page, independent of search and facet.

import { SvelteSet } from 'svelte/reactivity';
import { errorText } from '../i18n/texts';
import { invoke } from '../ipc/api';
import type {
  JobCounts,
  JobDetail,
  JobFacet,
  JobKey,
  JobSort,
  JobView,
  RunEvent,
} from '../ipc/types';
import { tokenMs } from '../tokens';
import { app } from './app.svelte';
import { run } from './run.svelte';

export const PAGE = 500;
export const WINDOW = 60;
/** Rows mounted per frame while a window fills. */
const CHUNK = 15;
const HIGH = 80;

export type JobFilter = 'high' | 'noDetail' | 'excluded';
type Status = 'idle' | 'loading' | 'ready' | 'error';

const ZERO: JobCounts = { new: 0, all: 0, excluded: 0, high: 0, noDetail: 0 };

export function keyOf(key: JobKey): string {
  return `${key.portal}:${key.id}`;
}

export function sameKey(a: JobKey | null, b: JobKey | null): boolean {
  return a !== null && b !== null && a.portal === b.portal && a.id === b.id;
}

const excluded = (job: JobView): boolean => job.match?.status === 'excluded';

function matches(job: JobView, filter: JobFilter | null): boolean {
  switch (filter) {
    case null:
      return true;
    case 'high':
      return job.match?.status === 'scored' && job.match.score >= HIGH;
    case 'noDetail':
      return job.detail.kind !== 'ok';
    case 'excluded':
      return excluded(job);
  }
}

/** What one job adds to the counts (to keep them exact while rows change live). */
function share(job: JobView): JobCounts {
  const out = excluded(job);
  return {
    all: 1,
    new: job.unread && !out ? 1 : 0,
    excluded: out ? 1 : 0,
    high: job.match?.status === 'scored' && job.match.score >= HIGH ? 1 : 0,
    noDetail: job.detail.kind !== 'ok' ? 1 : 0,
  };
}

function add(counts: JobCounts, job: JobView | null, sign: 1 | -1): JobCounts {
  if (job === null) return counts;
  const s = share(job);
  return {
    all: counts.all + sign * s.all,
    new: counts.new + sign * s.new,
    excluded: counts.excluded + sign * s.excluded,
    high: counts.high + sign * s.high,
    noDetail: counts.noDetail + sign * s.noDetail,
  };
}

class JobsStore {
  facet = $state<JobFacet>('new');
  sortChoice = $state<JobSort>('match');
  search = $state('');
  filter = $state<JobFilter | null>(null);

  rows = $state<JobView[]>([]);
  counts = $state<JobCounts>(ZERO);
  /** Rows the server has for the current query (for paging). */
  total = $state(0);
  status = $state<Status>('idle');
  slow = $state(false);
  error = $state<string | null>(null);
  window = $state(WINDOW);
  /** Rows mounted so far (grows towards `window` chunk by chunk). */
  rendered = $state(CHUNK);
  /** Keys inserted during the current run (accent tint). */
  fresh = new SvelteSet<string>();

  selected = $state<JobKey | null>(null);
  detail = $state<JobDetail | null>(null);
  detailStatus = $state<Status>('idle');
  detailSlow = $state(false);
  detailError = $state<string | null>(null);

  overview = $state<JobView[]>([]);

  #request = 0;
  #detailRequest = 0;
  #pumping = false;
  #searchTimer: ReturnType<typeof setTimeout> | null = null;
  #installed = false;

  /** The sort actually used: without a profile there is no match to sort by. */
  get sort(): JobSort {
    return app.hasProfile ? this.sortChoice : 'newest';
  }

  /** Rows of the list after the tile filter, excluded ones last (behind the divider). */
  get visible(): JobView[] {
    return this.filter === null ? this.rows : this.rows.filter((job) => matches(job, this.filter));
  }

  get shown(): JobView[] {
    return this.visible.slice(0, this.rendered);
  }

  /** More rows exist beyond the window (the sentinel shows once the window is rendered). */
  get more(): boolean {
    return (
      this.rendered >= this.window &&
      (this.window < this.visible.length || this.rows.length < this.total)
    );
  }

  /** Mount the window in chunks, one per frame: no frame builds 60 rows at once. */
  private pump(): void {
    if (this.#pumping) return;
    const target = (): number => Math.min(this.window, this.visible.length);
    if (this.rendered >= target()) return;
    this.#pumping = true;
    requestAnimationFrame(() => {
      this.#pumping = false;
      this.rendered = Math.min(this.rendered + CHUNK, Math.max(target(), CHUNK));
      this.pump();
    });
  }

  install(): void {
    if (this.#installed) return;
    this.#installed = true;
    run.listen((event) => this.onRun(event));
  }

  /** First load after the app state: open "Alle" when nothing is new. */
  async start(): Promise<void> {
    const counts = app.state?.counts;
    if (counts && counts.new === 0 && counts.all > 0) this.facet = 'all';
    await Promise.all([this.load(), this.loadOverview()]);
  }

  setFacet(facet: JobFacet): void {
    this.facet = facet;
    this.filter = null;
    void this.load();
  }

  setSort(sort: JobSort): void {
    this.sortChoice = sort;
    void this.load(true);
  }

  setSearch(value: string): void {
    this.search = value;
    if (this.#searchTimer !== null) clearTimeout(this.#searchTimer);
    this.#searchTimer = setTimeout(
      () => void this.load(),
      value === '' ? 0 : tokenMs('--dur-fast'),
    );
  }

  setFilter(filter: JobFilter | null): void {
    this.filter = filter;
    if (filter !== null && this.facet !== 'all') this.facet = 'all';
    void this.load();
  }

  /**
   * Load the first page (and with a tile filter every page, the filter is local).
   * `keep` = the same rows in a new order (sort, end of a run): the mounted rows stay and move.
   */
  async load(keep = false): Promise<void> {
    const request = ++this.#request;
    this.status = 'loading';
    this.error = null;
    const timer = setTimeout(() => {
      if (request === this.#request) this.slow = true;
    }, tokenMs('--dur-fast'));
    try {
      const page = await invoke('list_jobs', { query: this.query(0) });
      if (request !== this.#request) return;
      this.rows = page.jobs;
      this.counts = page.counts;
      this.total = this.facet === 'new' ? page.counts.new : page.counts.all;
      this.window = keep ? Math.max(WINDOW, this.window) : WINDOW;
      this.rendered = keep ? Math.min(this.rendered, this.window) : CHUNK;
      this.status = 'ready';
      if (this.filter !== null) await this.loadAll(request);
      this.pump();
    } catch (error) {
      if (request !== this.#request) return;
      this.status = 'error';
      this.error = errorText(error);
    } finally {
      clearTimeout(timer);
      if (request === this.#request) this.slow = false;
    }
  }

  private query(offset: number) {
    return {
      facet: this.facet,
      sort: this.sort,
      search: this.search.trim() === '' ? null : this.search.trim(),
      limit: PAGE,
      offset,
    };
  }

  private async page(request: number): Promise<boolean> {
    if (this.rows.length >= this.total) return false;
    const page = await invoke('list_jobs', { query: this.query(this.rows.length) });
    if (request !== this.#request || page.jobs.length === 0) return false;
    const known = new Set(this.rows.map((job) => keyOf(job.key)));
    this.rows = [...this.rows, ...page.jobs.filter((job) => !known.has(keyOf(job.key)))];
    this.counts = page.counts;
    return true;
  }

  private async loadAll(request: number): Promise<void> {
    while (request === this.#request && (await this.page(request)));
  }

  /** The sentinel at the end of the list became visible: show the next window. */
  async grow(): Promise<void> {
    if (this.window < this.visible.length) {
      this.window += WINDOW;
      this.pump();
      return;
    }
    const request = this.#request;
    try {
      if (await this.page(request)) {
        this.window += WINDOW;
        this.pump();
      }
    } catch (error) {
      this.error = errorText(error);
    }
  }

  async loadOverview(): Promise<void> {
    try {
      const page = await invoke('list_jobs', {
        query: { facet: 'all', sort: this.sort, search: null, limit: PAGE, offset: 0 },
      });
      this.overview = page.jobs;
    } catch {
      this.overview = [];
    }
  }

  /** Select a job. `click` = the user clicked it: only then it counts as read. */
  async select(job: JobView, click: boolean): Promise<void> {
    this.selected = job.key;
    if (click && job.unread) void this.markRead(job.key);
    await this.loadDetail(job.key);
  }

  clearSelection(): void {
    this.selected = null;
    this.detail = null;
    this.detailStatus = 'idle';
    this.#detailRequest++;
  }

  private async markRead(key: JobKey): Promise<void> {
    this.patch(key, { unread: false });
    try {
      await invoke('mark_read', { key });
    } catch {
      this.patch(key, { unread: true });
    }
  }

  async loadDetail(key: JobKey): Promise<void> {
    const request = ++this.#detailRequest;
    // The previous job stays until the next one is there (no blank flash between two jobs).
    this.detailStatus = 'loading';
    this.detailError = null;
    const timer = setTimeout(() => {
      if (request === this.#detailRequest) this.detailSlow = true;
    }, tokenMs('--dur-fast'));
    try {
      const detail = await invoke('job_detail', { key });
      if (request !== this.#detailRequest) return;
      this.detail = detail;
      this.detailStatus = 'ready';
    } catch (error) {
      if (request !== this.#detailRequest) return;
      this.detailStatus = 'error';
      this.detailError = errorText(error);
    } finally {
      clearTimeout(timer);
      if (request === this.#detailRequest) this.detailSlow = false;
    }
  }

  async pin(key: JobKey, on: boolean): Promise<void> {
    this.patch(key, { pinned: on });
    try {
      await invoke('set_pinned', { key, on });
    } catch {
      this.patch(key, { pinned: !on });
    }
    void this.loadOverview();
  }

  /** Change a row (and the reader) in place, keeping the counts exact. */
  private patch(key: JobKey, change: Partial<JobView>): void {
    const index = this.rows.findIndex((job) => sameKey(job.key, key));
    if (index >= 0) {
      const before = this.rows[index]!;
      const after = { ...before, ...change };
      this.counts = add(add(this.counts, before, -1), after, 1);
      this.rows[index] = after;
    }
    if (this.detail && sameKey(this.detail.job.key, key)) {
      this.detail = { ...this.detail, job: { ...this.detail.job, ...change } };
    }
    const inOverview = this.overview.findIndex((job) => sameKey(job.key, key));
    if (inOverview >= 0) this.overview[inOverview] = { ...this.overview[inOverview]!, ...change };
  }

  private onRun(event: RunEvent): void {
    if (event.type === 'jobUpdated') this.upsert(event.job);
    else if (event.type === 'finished') void this.afterRun();
    else if (event.type === 'progress' && event.step === 'scan' && event.done === 0) {
      this.fresh.clear();
    }
  }

  private upsert(job: JobView): void {
    const index = this.rows.findIndex((row) => sameKey(row.key, job.key));
    if (index >= 0) {
      this.counts = add(add(this.counts, this.rows[index]!, -1), job, 1);
      this.rows[index] = job;
    } else if (
      this.search.trim() === '' &&
      (this.facet === 'all' || (job.unread && !excluded(job)))
    ) {
      this.counts = add(this.counts, job, 1);
      this.total += 1;
      const at = excluded(job) ? this.rows.findIndex(excluded) : 0;
      this.rows.splice(at < 0 ? this.rows.length : at, 0, job);
      this.rendered += 1;
      this.window = Math.max(this.window, this.rendered);
      this.fresh.add(keyOf(job.key));
    }
    if (this.detail && sameKey(this.detail.job.key, job.key)) void this.loadDetail(job.key);
  }

  private async afterRun(): Promise<void> {
    await Promise.all([this.load(true), this.loadOverview()]);
    if (this.selected !== null) void this.loadDetail(this.selected);
  }
}

export const jobs = new JobsStore();
