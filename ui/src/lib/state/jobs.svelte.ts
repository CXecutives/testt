// The job list, the selection and the reader.
//
// - List and counts come from one `list_jobs` call (pages of 500); the list renders in
//   windows of 60 rows that grow while scrolling, so 2000 jobs never block a frame.
// - Rows are plain objects (`$state.raw`): a change replaces the row, so only that row
//   renders again, and no proxy sits between the template and 2000 jobs.
// - During a run new jobs are inserted at the top (they fade in) and existing rows update in
//   place (rings fill live). The list re-sorts once, when the run finishes, and keeps the
//   selection.
// - `mark_read` only on a real click on a row (select(..., true)).
// - The day overview reads its own unfiltered list of every job (all pages), independent
//   of search and facet: its tiles, the new jobs per portal and the pinned jobs agree with
//   each other and with the sidebar whatever the list shows.

import { SvelteSet } from 'svelte/reactivity';
import { errorText } from '../i18n/texts';
import { invoke } from '../ipc/api';
import type {
  JobCounts,
  JobDetail,
  JobFacet,
  JobKey,
  JobQuery,
  JobSort,
  JobView,
  Portal,
  RunEvent,
} from '../ipc/types';
import { tokenMs } from '../tokens';
import { app } from './app.svelte';
import { run } from './run.svelte';

export const PAGE = 500;
export const WINDOW = 60;
/** Rows mounted per frame while a window fills (small: every frame stays well below 50 ms
 *  on a slow machine, the window still fills within a few frames). */
const CHUNK = 6;
const HIGH = 80;

/** A tile of the day overview, or a portal (its new jobs). */
export type JobFilter = 'high' | 'noDetail' | 'excluded' | 'pinned' | Portal;
type Status = 'idle' | 'loading' | 'ready' | 'error';

const ZERO: JobCounts = { new: 0, all: 0, excluded: 0, high: 0, noDetail: 0 };

export function keyOf(key: JobKey): string {
  return `${key.portal}:${key.id}`;
}

export function sameKey(a: JobKey | null, b: JobKey | null): boolean {
  return a !== null && b !== null && a.portal === b.portal && a.id === b.id;
}

export const isExcluded = (job: JobView): boolean => job.match?.status === 'excluded';
const TILES: readonly string[] = ['high', 'noDetail', 'excluded', 'pinned'];
const isPortal = (filter: JobFilter): filter is Portal => !TILES.includes(filter);

function matches(job: JobView, filter: JobFilter | null): boolean {
  switch (filter) {
    case null:
      return true;
    case 'high':
      return job.match?.status === 'scored' && job.match.score >= HIGH;
    case 'noDetail':
      return job.detail.kind !== 'ok';
    case 'excluded':
      return isExcluded(job);
    case 'pinned':
      return job.pinned;
    default:
      return job.portal === filter;
  }
}

/** What one job adds to the counts (to keep them exact while rows change live). */
function share(job: JobView): JobCounts {
  const out = isExcluded(job);
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

/** `list` with the row of `key` replaced by `change(row)` (the same array if absent). */
function replaced(list: JobView[], key: JobKey, change: (job: JobView) => JobView): JobView[] {
  const index = list.findIndex((job) => sameKey(job.key, key));
  return index < 0 ? list : list.with(index, change(list[index]!));
}

class JobsStore {
  facet = $state<JobFacet>('new');
  sortChoice = $state<JobSort>('match');
  search = $state('');
  filter = $state<JobFilter | null>(null);

  rows = $state.raw<JobView[]>([]);
  counts = $state<JobCounts>(ZERO);
  /** Rows the server has for the current query (for paging). */
  total = $state(0);
  status = $state<Status>('idle');
  slow = $state(false);
  error = $state<string | null>(null);
  window = $state(WINDOW);
  /** Rows mounted so far (grows towards `window` chunk by chunk). */
  rendered = $state(CHUNK);
  /** Keys inserted while the list was on screen (they fade in). */
  fresh = new SvelteSet<string>();

  selected = $state<JobKey | null>(null);
  detail = $state.raw<JobDetail | null>(null);
  detailStatus = $state<Status>('idle');
  detailSlow = $state(false);
  detailError = $state<string | null>(null);

  /** Every job, unfiltered (the day overview). */
  overview = $state.raw<JobView[]>([]);
  /** The counts over every job, unfiltered (tiles and sidebar). */
  overviewCounts = $state<JobCounts | null>(null);
  overviewStatus = $state<Status>('idle');

  #request = 0;
  #detailRequest = 0;
  #overviewRequest = 0;
  #pumping = false;
  #searchTimer: ReturnType<typeof setTimeout> | null = null;
  #installed = false;

  /** The sort actually used: without a profile there is no match to sort by. */
  get sort(): JobSort {
    return app.hasProfile ? this.sortChoice : 'newest';
  }

  /** Rows of the list after the tile filter, excluded ones last (behind the divider). */
  readonly visible = $derived(
    this.filter === null ? this.rows : this.rows.filter((job) => matches(job, this.filter)),
  );

  readonly shown = $derived(this.visible.slice(0, this.rendered));

  /** More rows exist beyond the window (the sentinel shows once the window is rendered). */
  readonly more = $derived(
    this.rendered >= this.window &&
      (this.window < this.visible.length || this.rows.length < this.total),
  );

  /** Jobs the user pinned ("Merken"), from the unfiltered overview. */
  readonly pinned = $derived(this.overview.filter((job) => job.pinned).length);

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
    // The app state knows the counts already: no zeros while the first page loads.
    if (counts) this.counts = counts;
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
      value === '' ? 0 : tokenMs('--dur-base'),
    );
  }

  setFilter(filter: JobFilter | null): void {
    this.filter = filter;
    // A tile counts over all jobs; a portal chip counts its new ones.
    if (filter !== null) this.facet = isPortal(filter) ? 'new' : 'all';
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
      this.total = page.jobs.length < PAGE ? page.jobs.length : this.countOf(page.counts);
      this.window = keep ? Math.max(WINDOW, this.window) : WINDOW;
      this.rendered = keep ? Math.min(this.rendered, this.window) : CHUNK;
      this.fresh.clear();
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

  /** Rows the query has: under Neu the unread excluded jobs come on top of the count. */
  private countOf(counts: JobCounts): number {
    return this.facet === 'new' ? Number.MAX_SAFE_INTEGER : counts.all;
  }

  private query(offset: number): JobQuery {
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
    if (request !== this.#request) return false;
    if (page.jobs.length < PAGE) this.total = this.rows.length + page.jobs.length;
    if (page.jobs.length === 0) return false;
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

  /** Every job for the day overview (all pages; on an error it says nothing, not "nothing new"). */
  async loadOverview(): Promise<void> {
    const request = ++this.#overviewRequest;
    if (this.overviewStatus !== 'ready') this.overviewStatus = 'loading';
    try {
      const all: JobView[] = [];
      let counts: JobCounts = ZERO;
      for (;;) {
        const page = await invoke('list_jobs', {
          query: { facet: 'all', sort: 'newest', search: null, limit: PAGE, offset: all.length },
        });
        if (request !== this.#overviewRequest) return;
        all.push(...page.jobs);
        counts = page.counts;
        if (page.jobs.length < PAGE || all.length >= counts.all) break;
      }
      this.overview = all;
      this.overviewCounts = counts;
      this.overviewStatus = 'ready';
    } catch {
      if (request !== this.#overviewRequest) return;
      this.overviewStatus = 'error';
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
  }

  /** Change a row (the reader, the overview) in place, keeping the counts exact. */
  private patch(key: JobKey, change: Partial<JobView>): void {
    const before = this.rows.find((job) => sameKey(job.key, key)) ?? null;
    if (before !== null) {
      const after = { ...before, ...change };
      this.counts = add(add(this.counts, before, -1), after, 1);
      this.rows = replaced(this.rows, key, () => after);
    }
    if (this.detail && sameKey(this.detail.job.key, key)) {
      this.detail = { ...this.detail, job: { ...this.detail.job, ...change } };
    }
    this.patchOverview(key, (job) => ({ ...job, ...change }));
  }

  private patchOverview(key: JobKey, change: (job: JobView) => JobView): void {
    const before = this.overview.find((job) => sameKey(job.key, key)) ?? null;
    if (before === null) return;
    const after = change(before);
    this.overview = replaced(this.overview, key, () => after);
    if (this.overviewCounts !== null) {
      this.overviewCounts = add(add(this.overviewCounts, before, -1), after, 1);
    }
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
      this.rows = this.rows.with(index, job);
    } else if (this.search.trim() === '' && (this.facet === 'all' || job.unread)) {
      // Under Neu an unread excluded job also shows (grey, behind the divider), uncounted.
      this.counts = add(this.counts, job, 1);
      this.total += 1;
      const at = isExcluded(job) ? this.rows.findIndex(isExcluded) : 0;
      const rows = [...this.rows];
      rows.splice(at < 0 ? rows.length : at, 0, job);
      this.rows = rows;
      this.rendered += 1;
      this.window = Math.max(this.window, this.rendered);
      this.fresh.add(keyOf(job.key));
    }
    if (this.overview.some((row) => sameKey(row.key, job.key))) {
      this.patchOverview(job.key, () => job);
    } else {
      this.overview = [job, ...this.overview];
      if (this.overviewCounts !== null) this.overviewCounts = add(this.overviewCounts, job, 1);
    }
    if (this.detail && sameKey(this.detail.job.key, job.key)) void this.loadDetail(job.key);
  }

  private async afterRun(): Promise<void> {
    await Promise.all([this.load(true), this.loadOverview()]);
    if (this.selected !== null) void this.loadDetail(this.selected);
  }
}

export const jobs = new JobsStore();
