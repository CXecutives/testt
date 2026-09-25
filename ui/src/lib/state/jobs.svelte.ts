// The job list, the selection and the reader.
//
// - List and counts come from one `list_jobs` call (pages of 120: a page is read in the task
//   that shows its first rows, and 500 rows made that task several times longer than the 60
//   it shows); the list renders in windows of 60 rows that grow while scrolling, chunk by
//   chunk, so 2000 jobs never block a frame. A reload of the same list (sort, undo, the end
//   of a run) asks for as many rows as it holds, keeps the rows that did not change and
//   builds at most a chunk of new ones at once. Another list (place, tab, search, filter) is
//   a new generation of rows: the old rows go as one piece instead of one by one.
// - Rows are plain objects (`$state.raw`): a change replaces the row, so only that row
//   renders again, and no proxy sits between the template and 2000 jobs.
// - Every number comes from the backend (one truth): the counts of the list (with the
//   search) and the counts over every job (tiles, sidebar, new jobs per portal, favourites).
//   A change the page makes itself (read, a move) or a run update of a listed row moves
//   them at once; during a run a counts-only query follows every update (throttled), so
//   they stay exact for rows the page does not hold.
// - During a run the new jobs of the run are inserted at the top (they fade in) and listed
//   rows update in place (rings fill live); a job further down the list stays where the
//   next load puts it. The list re-sorts once, when the run finishes, and keeps the
//   selection.
// - `mark_read` when the user opens a job (a click, the keyboard); a job the app opens by
//   itself (the next one after a move) only once it has been looked at (`markSeen`).
// - Like Mail, Neu keeps the jobs read in this visit and the open one until the list is
//   entered again (another tab, back from another view); the last inbox tab is kept.
// - Like mail: every job is in one place (inbox, archive, trash); the favourite (the star)
//   is a flag of its own, and "fits anyway" another. A move takes the row out of a list it
//   no longer belongs to; deleting for good (only from the trash) removes it.
// - "Neu" is the unread jobs of the inbox; "Favoriten" the starred jobs of inbox and archive.

import { SvelteSet } from 'svelte/reactivity';
import { errorText } from '../i18n/texts';
import { invoke } from '../ipc/api';
import type {
  Deleted,
  JobCounts,
  JobDetail,
  JobKey,
  JobQuery,
  JobSort,
  JobView,
  Place,
  Portal,
  RunEvent,
} from '../ipc/types';
import { tokenMs } from '../tokens';
import { app } from './app.svelte';
import { run } from './run.svelte';

export const PAGE = 120;
/** The most rows one `list_jobs` call returns (core::view::MAX_PAGE). */
const MAX_PAGE = 500;
export const WINDOW = 60;
/** Rows mounted per frame while a window fills (small: every frame stays well below 50 ms
 *  on a slow machine, the window still fills within a few frames). */
const CHUNK = 6;
const HIGH = 80;
/** At most one counts query per this many ms while a run updates jobs. */
const COUNTS_EVERY = 400;

/** A tile of the day overview, or a portal (its new jobs). */
export type JobFilter = 'high' | 'noDetail' | 'excluded' | 'pinned' | Portal;
type Status = 'idle' | 'loading' | 'ready' | 'error';
/** What the list shows: the unread or all jobs of the inbox, the favourites, a place. */
export type JobFacet = 'new' | 'all' | 'favourites' | 'archived' | 'trash';

const ZERO: JobCounts = {
  inbox: 0,
  unread: 0,
  favourites: 0,
  archive: 0,
  trash: 0,
  excluded: 0,
  high: 0,
  noDetail: 0,
  newByPortal: [],
};

/** The place a facet lists. */
export function placeOf(facet: JobFacet): Place {
  return facet === 'archived' ? 'archive' : facet === 'trash' ? 'trash' : 'inbox';
}

export function keyOf(key: JobKey): string {
  return `${key.portal}:${key.id}`;
}

export function sameKey(a: JobKey | null, b: JobKey | null): boolean {
  return a !== null && b !== null && a.portal === b.portal && a.id === b.id;
}

export const isExcluded = (job: JobView): boolean => job.match?.status === 'excluded';

/** Does a job belong to the list of a facet (the backend's rule, store::job_page)? */
export function inFacet(job: JobView, facet: JobFacet): boolean {
  switch (facet) {
    case 'new':
      return job.place === 'inbox' && job.unread;
    case 'all':
      return job.place === 'inbox';
    case 'favourites':
      return job.pinned && job.place !== 'trash';
    case 'archived':
      return job.place === 'archive';
    case 'trash':
      return job.place === 'trash';
  }
}
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

/**
 * What one job adds to the counts (the backend's definitions, store::job_page): the inbox
 * counts only inbox jobs, a favourite counts until it goes to the trash.
 */
function add(counts: JobCounts, job: JobView | null, sign: 1 | -1): JobCounts {
  if (job === null) return counts;
  const shown = job.place === 'inbox' ? sign : 0;
  const out = isExcluded(job);
  const isNew = job.unread && !out ? shown : 0;
  const high = job.match?.status === 'scored' && job.match.score >= HIGH;
  return {
    inbox: counts.inbox + shown,
    unread: counts.unread + isNew,
    favourites: counts.favourites + (job.pinned && job.place !== 'trash' ? sign : 0),
    archive: counts.archive + (job.place === 'archive' ? sign : 0),
    trash: counts.trash + (job.place === 'trash' ? sign : 0),
    excluded: counts.excluded + (out ? shown : 0),
    high: counts.high + (high ? shown : 0),
    noDetail: counts.noDetail + (job.detail.kind !== 'ok' ? shown : 0),
    newByPortal: counts.newByPortal.map((line) =>
      line.portal === job.portal ? { ...line, new: line.new + isNew } : line,
    ),
  };
}

/** `counts` after `before` became `after` (the same job). */
function moved(counts: JobCounts, before: JobView, after: JobView): JobCounts {
  return add(add(counts, before, -1), after, 1);
}

/** A job a move took away, to bring back (`moveBack`): as it was, where it went, and its
 *  index in the list then (-1: the list did not hold it). */
export interface Unmove {
  job: JobView;
  to: Place;
  at: number;
}

/** The text of a row as it came from the backend, kept per object (each is read once). */
const texts = new WeakMap<JobView, string>();

function textOf(job: JobView): string {
  let text = texts.get(job);
  if (text === undefined) {
    text = JSON.stringify(job);
    texts.set(job, text);
  }
  return text;
}

/**
 * `next` with every row that equals the one held keeping the held object: a reload of the
 * same list (a sort, an undo, the end of a run) renders only the rows that changed, not all
 * of them again.
 */
function reused(held: readonly JobView[], next: JobView[]): JobView[] {
  const byKey = new Map(held.map((job) => [keyOf(job.key), job]));
  return next.map((job) => {
    const old = byKey.get(keyOf(job.key));
    return old !== undefined && textOf(old) === textOf(job) ? old : job;
  });
}

/** `list` with the row of `key` replaced by `change(row)` (the same array if absent). */
function replaced(list: JobView[], key: JobKey, change: (job: JobView) => JobView): JobView[] {
  const index = list.findIndex((job) => sameKey(job.key, key));
  return index < 0 ? list : list.with(index, change(list[index]!));
}

/** Where the order of the list is kept (one choice for every list, per user). */
const SORT_KEY = 'jobs-sort';

/** The kept order; a store that cannot be read keeps the default. */
function keptSort(): JobSort {
  try {
    const value = localStorage.getItem(SORT_KEY);
    return value === 'newest' ? 'newest' : 'match';
  } catch {
    return 'match';
  }
}

function keepSort(sort: JobSort): void {
  try {
    localStorage.setItem(SORT_KEY, sort);
  } catch {
    // Without a store the order lasts for this session only.
    return;
  }
}

class JobsStore {
  facet = $state<JobFacet>('new');
  sortChoice = $state<JobSort>(keptSort());
  search = $state('');
  filter = $state<JobFilter | null>(null);

  rows = $state.raw<JobView[]>([]);
  /** The counts of the list: with the search, whatever the facet. */
  counts = $state<JobCounts>(ZERO);
  /** Rows the backend has for the current query: its count, or exactly how many once a page
   *  came back short. */
  total = $state(0);
  /**
   * How far the page has read the backend's list of this query: the offset of the next page.
   * It counts the rows the backend served, less those that left the query since (read under
   * Neu, unstarred under Favoriten, moved, deleted) and plus those that came back: such rows
   * keep their place in the list for a while, so the number of rows is no offset. Rows the
   * page put in itself (a run's new jobs, the open job kept under Neu) do not count: where
   * they stand in the backend's order is unknown, and at worst the next page repeats a row
   * (it is dropped), it never skips one.
   */
  #served = $state(0);
  /** Keys of the rows the page put in itself (see #served). */
  #own = new Set<string>();
  status = $state<Status>('idle');
  /** The list has taken --delay-placeholder to load: its placeholder rows show (at once). */
  slow = $state(false);
  error = $state<string | null>(null);
  /** The next page did not load (the list stays, the end of it offers a retry). */
  pageError = $state<string | null>(null);
  /**
   * A job action of the list that failed (a move of a row or of the chosen jobs, its undo,
   * the star, "all read" and its undo): one sentence in the list header until the next
   * action succeeds or another list comes (place, tab, search, order).
   */
  actionError = $state<string | null>(null);
  /** Jobs were deleted for good, but a result file could not follow (the Excel file is open
   *  elsewhere): said in the list header like `actionError`. */
  exportNote = $state<string | null>(null);

  /** Another list or view: what the header said about the last action goes. */
  quiet(): void {
    this.actionError = null;
    this.exportNote = null;
  }
  window = $state(WINDOW);
  /** Rows mounted so far (grows towards `window` chunk by chunk). */
  rendered = $state(CHUNK);
  /** Counts the lists: a load that is not the same list in a new order starts a new one. */
  generation = $state(0);
  /** Keys inserted while the list was on screen (they fade in). */
  fresh = new SvelteSet<string>();
  /** A row the list brings into view once it is mounted (a job the keys opened further down,
   *  the open job after a re-sort), and whether it takes the focus. */
  reveal = $state<{ key: string; focus: boolean } | null>(null);

  selected = $state<JobKey | null>(null);
  detail = $state.raw<JobDetail | null>(null);
  detailStatus = $state<Status>('idle');
  /** The open job has taken --delay-placeholder to load: until then the reader keeps what it
   *  showed, then its placeholder shows (at once). */
  detailSlow = $state(false);
  detailError = $state<string | null>(null);

  /** The counts over every job, without the search (tiles, sidebar, new per portal). */
  overviewCounts = $state<JobCounts | null>(null);
  overviewStatus = $state<Status>('idle');

  #request = 0;
  #detailRequest = 0;
  #overviewRequest = 0;
  #pumping = false;
  #searchTimer: ReturnType<typeof setTimeout> | null = null;
  #countsTimer: ReturnType<typeof setTimeout> | null = null;
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

  /**
   * More rows exist beyond the window (the sentinel shows once the window is rendered). The
   * last window of a page can reach past its rows: rendered is then all of them, and the next
   * page follows.
   */
  readonly more = $derived(
    this.rendered >= Math.min(this.window, this.visible.length) &&
      (this.window < this.visible.length || this.#served < this.total),
  );

  /** The favourites (the star) of inbox and archive. */
  get pinned(): number {
    return this.overviewCounts?.favourites ?? 0;
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

  /** First load after the app state: open "Alle" when nothing is new (and keep it as the
   *  inbox tab, so Jobs in the sidebar comes back to it). */
  async start(): Promise<void> {
    const counts = app.state?.counts;
    // The app state knows the counts already: no zeros while the first page loads.
    if (counts) {
      this.counts = counts;
      this.overviewCounts = counts;
    }
    if (counts && counts.unread === 0 && counts.inbox > 0) {
      this.facet = 'all';
      this.inboxFacet = 'all';
    }
    await Promise.all([this.load(), this.loadOverview()]);
  }

  /** The last tab of the inbox (Neu, Alle, Favoriten): Jobs in the sidebar goes back to it. */
  inboxFacet = $state<JobFacet>('new');

  setFacet(facet: JobFacet): void {
    // Another place: an open job of the one left behind closes, like a mail of another
    // folder (here, not in the list: the place also changes from Profil or Einstellungen).
    const selected = this.selected;
    const open =
      this.detail?.job ??
      (selected ? this.rows.find((row) => sameKey(row.key, selected)) : undefined) ??
      null;
    if (open !== null && placeOf(facet) !== open.place && !inFacet(open, facet)) {
      this.clearSelection();
    }
    this.facet = facet;
    if (facet === 'new' || facet === 'all' || facet === 'favourites') this.inboxFacet = facet;
    this.filter = null;
    this.quiet();
    void this.load();
  }

  setSort(sort: JobSort): void {
    this.sortChoice = sort;
    keepSort(sort);
    this.quiet();
    // The open job keeps its row in view in the new order, also when that is further down.
    const open = this.selected;
    const listed = this.rows.some((job) => sameKey(job.key, open));
    void this.load(true).then(() => {
      if (open !== null && listed && sameKey(this.selected, open) && this.status === 'ready') {
        void this.reach(open, false);
      }
    });
  }

  setSearch(value: string): void {
    this.search = value;
    this.quiet();
    if (this.#searchTimer !== null) clearTimeout(this.#searchTimer);
    this.#searchTimer = setTimeout(
      () => void this.load(),
      value === '' ? 0 : tokenMs('--dur-base'),
    );
  }

  setFilter(filter: JobFilter | null): void {
    this.filter = filter;
    this.quiet();
    // A tile counts over all jobs; a portal chip counts its new ones.
    if (filter !== null) this.facet = isPortal(filter) ? 'new' : 'all';
    void this.load();
  }

  /**
   * Load the first page (and with a tile filter every page, the filter is local).
   * `keep` = the same rows in a new order (sort, end of a run): the mounted rows stay and move,
   * and as many rows come back as the list holds (a list scrolled far down stays as long).
   */
  async load(keep = false): Promise<void> {
    const request = ++this.#request;
    this.status = 'loading';
    this.error = null;
    this.pageError = null;
    if (!keep) this.reveal = null;
    const timer = setTimeout(() => {
      if (request === this.#request) this.slow = true;
    }, tokenMs('--delay-placeholder'));
    try {
      const limit = keep ? Math.min(MAX_PAGE, Math.max(PAGE, this.rows.length)) : PAGE;
      const page = await invoke('list_jobs', { query: this.query(0, limit) });
      if (request !== this.#request) return;
      const mounted = keep ? new Set(this.shown.map((job) => keyOf(job.key))) : null;
      this.#own.clear();
      this.#served = page.jobs.length;
      this.rows = this.withOpen(keep ? reused(this.rows, page.jobs) : page.jobs);
      this.counts = page.counts;
      this.total = page.jobs.length < limit ? page.jobs.length : this.countOf(page.counts);
      this.window = keep ? Math.max(WINDOW, this.window) : WINDOW;
      this.rendered = mounted ? this.kept(mounted, Math.min(this.rendered, this.window)) : CHUNK;
      if (!keep) this.generation += 1;
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

  /**
   * Neu keeps the open job listed after it was read, where it stood, until another job is
   * opened (like Mail): a reload never pulls the job away from under the reader.
   */
  private withOpen(rows: JobView[]): JobView[] {
    const open = this.detail?.job ?? null;
    if (this.facet !== 'new' || this.search.trim() !== '') return rows;
    if (open === null || !sameKey(open.key, this.selected)) return rows;
    if (open.place !== 'inbox' || rows.some((job) => sameKey(job.key, open.key))) return rows;
    const at = this.rows.findIndex((job) => sameKey(job.key, open.key));
    // Among its kind: the excluded jobs stay behind the others.
    const out = rows.findIndex(isExcluded);
    const split = out < 0 ? rows.length : out;
    const [low, high] = isExcluded(open) ? [split, rows.length] : [0, split];
    const index = Math.max(low, Math.min(at < 0 ? low : at, high));
    this.#own.add(keyOf(open.key));
    return [...rows.slice(0, index), open, ...rows.slice(index)];
  }

  /**
   * How many rows stay mounted when the same list comes back in a new order (the sort, the
   * end of a run, an undo): up to `limit`, but never more than CHUNK rows that are not
   * mounted yet, so no frame builds a whole window of new rows (another sort shows other
   * jobs in its first rows). The rest follows chunk by chunk.
   */
  private kept(mounted: ReadonlySet<string>, limit: number): number {
    const rows = this.visible;
    const end = Math.min(limit, rows.length);
    let fresh = 0;
    for (let index = 0; index < end; index += 1) {
      if (!mounted.has(keyOf(rows[index]!.key))) fresh += 1;
      if (fresh > CHUNK) return Math.max(CHUNK, index);
    }
    return end;
  }

  /** Rows the query has: under Neu the unread excluded jobs come on top of the count. */
  private countOf(counts: JobCounts): number {
    switch (this.facet) {
      case 'new':
        return Number.MAX_SAFE_INTEGER;
      case 'all':
        return counts.inbox;
      case 'favourites':
        return counts.favourites;
      case 'archived':
        return counts.archive;
      case 'trash':
        return counts.trash;
    }
  }

  private query(offset: number, limit = PAGE): JobQuery {
    return {
      place: placeOf(this.facet),
      unread: this.facet === 'new',
      favourites: this.facet === 'favourites',
      sort: this.sort,
      search: this.search.trim() === '' ? null : this.search.trim(),
      limit,
      offset,
    };
  }

  /** The next page of the backend's list (see #served): its rows the list does not hold yet. */
  private async page(request: number): Promise<boolean> {
    if (this.#served >= this.total) return false;
    const page = await invoke('list_jobs', { query: this.query(this.#served) });
    if (request !== this.#request) return false;
    this.#served += page.jobs.length;
    if (page.jobs.length < PAGE) this.total = this.#served;
    if (page.jobs.length === 0) return false;
    const known = new Set(this.rows.map((job) => keyOf(job.key)));
    this.rows = [...this.rows, ...page.jobs.filter((job) => !known.has(keyOf(job.key)))];
    this.counts = page.counts;
    return true;
  }

  private async loadAll(request: number): Promise<void> {
    while (request === this.#request && (await this.page(request)));
  }

  /**
   * The row at `target` of the list (an index, the last row, or a job), loading the pages up
   * to it, and a window that reaches it: its row mounts within a few frames (chunk by chunk,
   * like every window) and the list then brings it into view (`reveal`). Resolves with the
   * job, or null when the list has no such row or another list replaced it meanwhile.
   */
  async reach(target: number | 'last' | JobKey, focus: boolean): Promise<JobView | null> {
    const request = this.#request;
    const index = (): number =>
      target === 'last'
        ? this.visible.length - 1
        : typeof target === 'number'
          ? target
          : this.visible.findIndex((job) => sameKey(job.key, target));
    const missing = (): boolean =>
      target === 'last' || index() < 0 || index() >= this.visible.length;
    this.pageError = null;
    try {
      while (request === this.#request && missing() && (await this.page(request)));
    } catch (error) {
      if (request === this.#request) this.pageError = errorText(error);
    }
    if (request !== this.#request) return null;
    const at = Math.min(index(), this.visible.length - 1);
    const job = at < 0 ? undefined : this.visible[at];
    if (job === undefined) return null;
    if (at >= this.window) this.window = Math.ceil((at + 1) / WINDOW) * WINDOW;
    this.pump();
    this.reveal = { key: keyOf(job.key), focus };
    return job;
  }

  /** The sentinel at the end of the list became visible: show the next window. */
  async grow(): Promise<void> {
    if (this.window < this.visible.length) {
      this.window += WINDOW;
      this.pump();
      return;
    }
    const request = this.#request;
    this.pageError = null;
    try {
      if (await this.page(request)) {
        this.window += WINDOW;
        this.pump();
      }
    } catch (error) {
      if (request === this.#request) this.pageError = errorText(error);
    }
  }

  /**
   * The counts over every job for the day overview and the sidebar: one counts-only query
   * (on an error the overview says nothing, not "nothing new").
   */
  async loadOverview(): Promise<void> {
    const request = ++this.#overviewRequest;
    if (this.overviewStatus !== 'ready') this.overviewStatus = 'loading';
    try {
      const page = await invoke('list_jobs', {
        query: {
          place: 'inbox',
          unread: false,
          favourites: false,
          sort: 'newest',
          search: null,
          limit: 0,
          offset: 0,
        },
      });
      if (request !== this.#overviewRequest) return;
      this.overviewCounts = page.counts;
      this.overviewStatus = 'ready';
    } catch {
      if (request !== this.#overviewRequest) return;
      this.overviewStatus = 'error';
    }
  }

  /** Both counts again from the backend (the rows stay). */
  private async refreshCounts(): Promise<void> {
    const request = this.#request;
    const searching = this.search.trim() !== '';
    try {
      const page = await invoke('list_jobs', { query: this.query(0, 0) });
      if (request !== this.#request) return;
      this.counts = page.counts;
      // Without a search the list's counts are the counts over every job.
      if (!searching) {
        this.#overviewRequest++;
        this.overviewCounts = page.counts;
        this.overviewStatus = 'ready';
      }
    } catch {
      // Try again while the run goes; its end reloads everything anyway.
      if (run.active) this.countsSoon();
    }
    if (searching) await this.loadOverview();
  }

  /** A counts query soon, at most one per COUNTS_EVERY ms (a run sends many updates). */
  private countsSoon(): void {
    if (this.#countsTimer !== null) return;
    this.#countsTimer = setTimeout(() => {
      this.#countsTimer = null;
      void this.refreshCounts();
    }, COUNTS_EVERY);
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

  /** The open job has been looked at (a job the app opened by itself). */
  markSeen(key: JobKey): void {
    const job = this.held(key);
    if (job?.unread) void this.markRead(key);
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
    }, tokenMs('--delay-placeholder'));
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

  /** The favourite (the star), a flag of its own whatever the place. Resolves with the
   *  error text (the star goes back), or null. */
  async pin(key: JobKey, on: boolean): Promise<string | null> {
    const before = this.held(key);
    if (before === null || before.pinned === on) return null;
    this.patch(key, { pinned: on });
    try {
      await invoke('set_pinned', { key, on });
      return null;
    } catch (error) {
      this.patch(key, { pinned: before.pinned });
      return errorText(error);
    }
  }

  /** The job as the page holds it (a row or the reader). */
  private held(key: JobKey): JobView | null {
    const row = this.rows.find((job) => sameKey(job.key, key));
    if (row) return row;
    return this.detail && sameKey(this.detail.job.key, key) ? this.detail.job : null;
  }

  /**
   * "All read": every unread job of the current place; with a search only its hits (what the
   * list shows). Resolves with the keys for the undo (`markUnread`), or the error text.
   */
  async markAllRead(): Promise<{ keys: JobKey[] } | { error: string }> {
    const search = this.search.trim() === '' ? null : this.search.trim();
    try {
      const keys = await invoke('mark_all_read', { place: placeOf(this.facet), search });
      this.patchAll(keys, { unread: false });
      void this.refreshCounts();
      return { keys };
    } catch (error) {
      return { error: errorText(error) };
    }
  }

  /** The undo of "all read". Resolves with the error text, or null. */
  async markUnread(keys: JobKey[]): Promise<string | null> {
    try {
      await invoke('mark_unread', { keys });
      this.patchAll(keys, { unread: true });
      void this.refreshCounts();
      return null;
    } catch (error) {
      return errorText(error);
    }
  }

  /**
   * "Fits anyway": an excluded job counts as scored with its fit score, or the engine's
   * verdict applies again. The backend assesses it anew, so the row and the reader follow
   * its answer. Resolves with the error text, or null.
   */
  async setOverride(key: JobKey, include: boolean): Promise<string | null> {
    try {
      await invoke('set_override', { key, include });
    } catch (error) {
      return errorText(error);
    }
    try {
      const detail = await invoke('job_detail', { key });
      this.patch(key, detail.job);
      if (sameKey(this.selected, key)) {
        this.#detailRequest++;
        this.detail = detail;
        this.detailStatus = 'ready';
      }
    } catch {
      void this.load(true);
    }
    return null;
  }

  /**
   * "Endgültig löschen": deletes jobs of the trash for good (rows, text files; a later scan
   * never brings them back). Resolves with what the backend did, or the error text.
   */
  async purge(keys: JobKey[]): Promise<Deleted | { error: string }> {
    return this.forget(() => invoke('purge_jobs', { keys }));
  }

  /**
   * Empties the trash like Mail: every job in it is deleted for good, whatever the list
   * shows; the result names how many and which.
   */
  async emptyTrash(): Promise<Deleted | { error: string }> {
    return this.forget(() => invoke('empty_trash'));
  }

  private async forget(command: () => Promise<Deleted>): Promise<Deleted | { error: string }> {
    try {
      const deleted = await command();
      // What the backend deleted: the rows, the open job (also one the list does not hold).
      const gone = new Set(deleted.keys.map(keyOf));
      const rows = this.rows.filter((job) => !gone.has(keyOf(job.key)));
      for (const job of this.rows) {
        if (gone.has(keyOf(job.key))) this.recount(job, null);
      }
      this.rows = rows;
      if (this.selected !== null && gone.has(keyOf(this.selected))) this.clearSelection();
      await this.refreshCounts();
      return deleted;
    } catch (error) {
      return { error: errorText(error) };
    }
  }

  /**
   * Moves jobs to the inbox, the archive or the trash. The rows leave a list they no longer
   * belong to at once. Resolves with the keys that really moved (toasts and undos only for
   * those; a job already there or gone did not), or the error text; on an error, or when not
   * every job moved, the list loads again.
   */
  async move(keys: JobKey[], to: Place): Promise<{ moved: JobKey[] } | { error: string }> {
    const before = keys.map((key) => this.held(key)).filter((job): job is JobView => job !== null);
    for (const job of before) {
      this.patch(job.key, { place: to });
      this.dropStray(job.key);
    }
    try {
      const moved = await invoke('move_jobs', { keys, to });
      if (moved.length < keys.length) void this.load(true);
      return { moved };
    } catch (error) {
      for (const job of before) this.patch(job.key, { place: job.place });
      void this.load(true);
      return { error: errorText(error) };
    }
  }

  /**
   * Takes moves back (the undo of a toast): every job goes back to the place it came from
   * (one call per place). A row the list lost comes back where it stood when the list is
   * still the one it left (Neu keeps a read job, like before the move); in another list the
   * list loads again when the job belongs there. Resolves with the keys that went back (a
   * job already there did not), or the error text.
   */
  async moveBack(
    back: readonly Unmove[],
    generation: number,
  ): Promise<{ moved: JobKey[] } | { error: string }> {
    const landed: JobKey[] = [];
    try {
      for (const place of new Set(back.map(({ job }) => job.place))) {
        const keys = back.filter(({ job }) => job.place === place).map(({ job }) => job.key);
        landed.push(...(await invoke('move_jobs', { keys, to: place })));
      }
    } catch (error) {
      void this.load(true);
      return { error: errorText(error) };
    }
    const done = new Set(landed.map(keyOf));
    const same = generation === this.generation;
    let missing = false;
    for (const { job, to, at } of [...back].sort((a, b) => a.at - b.at)) {
      if (!done.has(keyOf(job.key))) continue;
      if (this.rows.some((row) => sameKey(row.key, job.key)) || !same || at < 0) {
        this.patch(job.key, { place: job.place });
        missing ||= !this.rows.some((row) => sameKey(row.key, job.key)) && inFacet(job, this.facet);
        continue;
      }
      const gone = { ...job, place: to };
      const rows = [...this.rows];
      rows.splice(Math.min(at, rows.length), 0, job);
      this.rows = rows;
      this.counts = moved(this.counts, gone, job);
      if (this.overviewCounts !== null) this.overviewCounts = moved(this.overviewCounts, gone, job);
      this.recount(gone, job);
      if (this.detail && sameKey(this.detail.job.key, job.key)) {
        this.detail = { ...this.detail, job: { ...this.detail.job, place: job.place } };
      }
    }
    if (missing) void this.load(true);
    else void this.refreshCounts();
    return { moved: landed };
  }

  /** Archives a job or brings it back to the inbox (the reader's and the row's tool). */
  async archive(key: JobKey, archived: boolean): Promise<string | null> {
    const result = await this.move([key], archived ? 'archive' : 'inbox');
    return 'error' in result ? result.error : null;
  }

  /** The prompt for a deep analysis of a job in any AI chat. */
  async aiPrompt(key: JobKey): Promise<string> {
    return invoke('ai_prompt', { key });
  }

  /** One prompt that compares the best current matches (3 to 5) in any AI chat. */
  async aiPromptTop(limit: number): Promise<string> {
    return invoke('ai_prompt_top', { limit });
  }

  /** A listed row that no longer belongs to the facet leaves the list (`patch` has counted
   *  it out of the backend's list already). */
  private dropStray(key: JobKey): void {
    const row = this.rows.find((job) => sameKey(job.key, key));
    if (!row || inFacet(row, this.facet)) return;
    this.rows = this.rows.filter((job) => !sameKey(job.key, key));
  }

  /**
   * A held row changed (`after`) or was deleted (null): the backend's list of the query loses
   * or gains it, so its total follows, and so does the offset of the next page when the
   * backend served the row (every held row it served stands before that offset).
   */
  private recount(before: JobView, after: JobView | null): void {
    const change =
      Number(after !== null && inFacet(after, this.facet)) - Number(inFacet(before, this.facet));
    if (change === 0) return;
    this.total = Math.max(0, this.total + change);
    if (!this.#own.has(keyOf(before.key))) this.#served = Math.max(0, this.#served + change);
  }

  /** Change a job the page holds (a row, the reader) in place, moving the counts with it. */
  private patch(key: JobKey, change: Partial<JobView>): void {
    const row = this.rows.find((job) => sameKey(job.key, key)) ?? null;
    const shown = this.detail && sameKey(this.detail.job.key, key) ? this.detail.job : null;
    const before = row ?? shown;
    // A job the page does not hold (a row of the overview, an undo after the row left):
    // the counts still follow, from the backend.
    if (before === null) {
      this.countsSoon();
      return;
    }
    const after = { ...before, ...change };
    // A listed row belongs to the list's counts; every job belongs to the overall ones.
    if (row !== null) {
      this.counts = moved(this.counts, row, after);
      this.recount(row, after);
      this.rows = replaced(this.rows, key, () => after);
    } else {
      this.countsSoon();
    }
    if (this.overviewCounts !== null) {
      this.overviewCounts = moved(this.overviewCounts, before, after);
    }
    if (shown !== null && this.detail) {
      this.detail = { ...this.detail, job: { ...this.detail.job, ...change } };
    }
  }

  /**
   * `patch` for many jobs at once ("all read" and its undo): one pass over the rows and one
   * copy of them, the list's counts moved per listed row. The caller asks the backend for the
   * counts afterwards (it knows the jobs the page does not hold).
   */
  private patchAll(keys: readonly JobKey[], change: Partial<JobView>): void {
    const wanted = new Set(keys.map(keyOf));
    let counts = this.counts;
    let overall = this.overviewCounts;
    let changed = false;
    const rows = this.rows.map((row) => {
      if (!wanted.has(keyOf(row.key))) return row;
      const after = { ...row, ...change };
      counts = moved(counts, row, after);
      if (overall !== null) overall = moved(overall, row, after);
      this.recount(row, after);
      changed = true;
      return after;
    });
    if (changed) {
      this.rows = rows;
      this.counts = counts;
      this.overviewCounts = overall;
    }
    const shown = this.detail;
    if (shown !== null && wanted.has(keyOf(shown.job.key))) {
      this.detail = { ...shown, job: { ...shown.job, ...change } };
    }
  }

  private onRun(event: RunEvent): void {
    if (event.type === 'jobUpdated') this.upsert(event.job, event.fresh);
    else if (event.type === 'finished') void this.afterRun();
    else if (event.type === 'progress' && event.step === 'scan' && event.done === 0) {
      this.fresh.clear();
    }
  }

  /**
   * A job of the run changed. A listed row updates in place; a job new in this run comes
   * in at the top (under Neu only while unread, never into a search). A known job the page
   * does not list (further down, beyond the loaded page) stays out: it is no new row. The
   * counts follow from the backend.
   */
  private upsert(job: JobView, fresh: boolean): void {
    const index = this.rows.findIndex((row) => sameKey(row.key, job.key));
    if (index >= 0) {
      const before = this.rows[index]!;
      this.counts = moved(this.counts, before, job);
      if (this.overviewCounts !== null) {
        this.overviewCounts = moved(this.overviewCounts, before, job);
      }
      this.recount(before, job);
      this.rows = this.rows.with(index, job);
    } else if (
      (fresh || this.#served >= this.total) &&
      this.search.trim() === '' &&
      inFacet(job, this.facet)
    ) {
      // Under Neu an unread excluded job also shows (grey, behind the divider), uncounted.
      const at = isExcluded(job) ? this.rows.findIndex(isExcluded) : 0;
      const rows = [...this.rows];
      rows.splice(at < 0 ? rows.length : at, 0, job);
      this.#own.add(keyOf(job.key));
      this.rows = rows;
      this.total += 1;
      this.rendered += 1;
      this.window = Math.max(this.window, this.rendered);
      this.fresh.add(keyOf(job.key));
    }
    this.countsSoon();
    // The reader follows the job the list has selected (it may still show the one before).
    if (sameKey(this.selected, job.key)) void this.loadDetail(job.key);
  }

  private async afterRun(): Promise<void> {
    if (this.#countsTimer !== null) {
      clearTimeout(this.#countsTimer);
      this.#countsTimer = null;
    }
    await Promise.all([this.load(true), this.loadOverview()]);
    if (this.selected !== null) void this.loadDetail(this.selected);
  }
}

export const jobs = new JobsStore();
