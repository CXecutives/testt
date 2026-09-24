// The job list, the selection and the reader.
//
// - List and counts come from one `list_jobs` call (pages of 500); the list renders in
//   windows of 60 rows that grow while scrolling, so 2000 jobs never block a frame.
// - Rows are plain objects (`$state.raw`): a change replaces the row, so only that row
//   renders again, and no proxy sits between the template and 2000 jobs.
// - Every number comes from the backend (one truth): the counts of the list (with the
//   search) and the counts over every job (tiles, sidebar, new jobs per portal, saved).
//   A change the page makes itself (read, a stage) or a run update of a listed row moves
//   them at once; during a run a counts-only query follows every update (throttled), so
//   they stay exact for rows the page does not hold.
// - During a run the new jobs of the run are inserted at the top (they fade in) and listed
//   rows update in place (rings fill live); a job further down the list stays where the
//   next load puts it. The list re-sorts once, when the run finishes, and keeps the
//   selection.
// - `mark_read` only on a real click on a row (select(..., true)).
// - The user's marks: one pipeline of stages (saved, the star, then the application) with a
//   follow-up day, a note, "fits anyway" and the archive. An archived job is in no list but
//   the archive and in no count but its own; archiving or listing it again takes the row out
//   of a list it no longer belongs to. A stage change keeps the row where it is until the
//   next load (the list does not jump under the pointer). Deleting for good removes the row.
// - "Neu" holds the unread jobs of the last 14 days (store::new_since): older unread ones
//   stay under "Alle".

import { SvelteSet } from 'svelte/reactivity';
import { errorText } from '../i18n/texts';
import { invoke } from '../ipc/api';
import type {
  AppStatus,
  Deleted,
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
/** At most one counts query per this many ms while a run updates jobs. */
const COUNTS_EVERY = 400;

/** A tile of the day overview, or a portal (its new jobs). */
export type JobFilter = 'high' | 'noDetail' | 'excluded' | 'pinned' | Portal;
type Status = 'idle' | 'loading' | 'ready' | 'error';

const ZERO: JobCounts = {
  new: 0,
  all: 0,
  excluded: 0,
  high: 0,
  noDetail: 0,
  saved: 0,
  applications: 0,
  archived: 0,
  newByPortal: [],
};

/** "Neu" holds the unread jobs of this many days (store::NEW_DAYS). */
const NEW_DAYS = 14;
const DAY = 86_400_000;

/** Is the job recent enough for "Neu" (by the date of its alert mail; store::new_since)? */
export function isRecent(job: JobView, now = Date.now()): boolean {
  const since = Math.floor(now / DAY) * DAY - NEW_DAYS * DAY;
  return Date.parse(job.mailDate ?? job.firstSeenAt) >= since;
}

/** A stage of an application (everything after "saved"). */
export const isApplication = (status: AppStatus | null): boolean =>
  status !== null && status !== 'saved';

/** Stages that wait for an answer: only they keep a follow-up day. */
const awaitsAnswer = (status: AppStatus | null): boolean =>
  status === 'applied' || status === 'interview';

export function keyOf(key: JobKey): string {
  return `${key.portal}:${key.id}`;
}

export function sameKey(a: JobKey | null, b: JobKey | null): boolean {
  return a !== null && b !== null && a.portal === b.portal && a.id === b.id;
}

export const isExcluded = (job: JobView): boolean => job.match?.status === 'excluded';

/** Does a job belong to the list of a facet (the backend's rule, store::ListFacet)? */
export function inFacet(job: JobView, facet: JobFacet): boolean {
  switch (facet) {
    case 'new':
      return !job.archived && job.unread && isRecent(job);
    case 'all':
      return !job.archived;
    case 'saved':
      return !job.archived && job.appStatus === 'saved';
    case 'applications':
      return !job.archived && isApplication(job.appStatus);
    case 'archived':
      return job.archived;
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
 * What one job adds to the counts (the backend's definitions, store::job_page): an archived
 * job only to "archived".
 */
function add(counts: JobCounts, job: JobView | null, sign: 1 | -1): JobCounts {
  if (job === null) return counts;
  const shown = job.archived ? 0 : sign;
  const out = isExcluded(job);
  const isNew = job.unread && !out && isRecent(job) ? shown : 0;
  const high = job.match?.status === 'scored' && job.match.score >= HIGH;
  return {
    all: counts.all + shown,
    new: counts.new + isNew,
    excluded: counts.excluded + (out ? shown : 0),
    high: counts.high + (high ? shown : 0),
    noDetail: counts.noDetail + (job.detail.kind !== 'ok' ? shown : 0),
    saved: counts.saved + (job.appStatus === 'saved' ? shown : 0),
    applications: counts.applications + (isApplication(job.appStatus) ? shown : 0),
    archived: counts.archived + (job.archived ? sign : 0),
    newByPortal: counts.newByPortal.map((line) =>
      line.portal === job.portal ? { ...line, new: line.new + isNew } : line,
    ),
  };
}

/** `counts` after `before` became `after` (the same job). */
function moved(counts: JobCounts, before: JobView, after: JobView): JobCounts {
  return add(add(counts, before, -1), after, 1);
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
  /** The counts of the list: with the search, whatever the facet. */
  counts = $state<JobCounts>(ZERO);
  /** Rows the server has for the current query (for paging). */
  total = $state(0);
  status = $state<Status>('idle');
  slow = $state(false);
  error = $state<string | null>(null);
  /** The next page did not load (the list stays, the end of it offers a retry). */
  pageError = $state<string | null>(null);
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
      (this.window < this.visible.length || this.rows.length < this.total),
  );

  /** Jobs the user saved (the star), over every job. */
  get pinned(): number {
    return this.overviewCounts?.saved ?? 0;
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
    // The app state knows the counts already: no zeros while the first page loads.
    if (counts) {
      this.counts = counts;
      this.overviewCounts = counts;
    }
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
    this.pageError = null;
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
    switch (this.facet) {
      case 'new':
        return Number.MAX_SAFE_INTEGER;
      case 'all':
        return counts.all;
      case 'saved':
        return counts.saved;
      case 'applications':
        return counts.applications;
      case 'archived':
        return counts.archived;
    }
  }

  private query(offset: number, limit = PAGE): JobQuery {
    return {
      facet: this.facet,
      sort: this.sort,
      search: this.search.trim() === '' ? null : this.search.trim(),
      limit,
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
        query: { facet: 'all', sort: 'newest', search: null, limit: 0, offset: 0 },
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

  /**
   * The star is the stage "saved": it never overwrites a later stage, and taking it off
   * clears only "saved" (store::set_pinned).
   */
  async pin(key: JobKey, on: boolean): Promise<void> {
    const before = this.held(key);
    if (before === null) return;
    const stage = before.appStatus;
    if (on ? stage !== null : stage !== 'saved') return;
    this.patch(key, this.stageChange(on ? 'saved' : null));
    try {
      await invoke('set_pinned', { key, on });
    } catch {
      this.patch(key, { pinned: before.pinned, appStatus: stage, statusAt: before.statusAt });
    }
  }

  /** The fields a new stage changes in a row (the backend's rule, store::set_app_status). */
  private stageChange(status: AppStatus | null): Partial<JobView> {
    return {
      appStatus: status,
      pinned: status === 'saved',
      statusAt: status === null ? null : new Date().toISOString(),
      ...(awaitsAnswer(status) ? {} : { followUpOn: null }),
    };
  }

  /** The job as the page holds it (a row or the reader). */
  private held(key: JobKey): JobView | null {
    const row = this.rows.find((job) => sameKey(job.key, key));
    if (row) return row;
    return this.detail && sameKey(this.detail.job.key, key) ? this.detail.job : null;
  }

  /**
   * The stage of a job (`null` = none). Moves at once and back on an error; resolves with
   * the error text, or null.
   */
  async setAppStatus(key: JobKey, status: AppStatus | null): Promise<string | null> {
    const before = this.held(key);
    if (before === null || before.appStatus === status) return null;
    const { pinned, appStatus, statusAt, followUpOn } = before;
    this.patch(key, this.stageChange(status));
    try {
      await invoke('set_app_status', { key, status });
      return null;
    } catch (error) {
      this.patch(key, { pinned, appStatus, statusAt, followUpOn });
      return errorText(error);
    }
  }

  /**
   * The day to follow up an application (`YYYY-MM-DD`, `null` clears it; only while applied
   * or in talks). Resolves with the error text, or null.
   */
  async setFollowUp(key: JobKey, on: string | null): Promise<string | null> {
    const before = this.held(key);
    if (before === null || !awaitsAnswer(before.appStatus)) return null;
    this.patch(key, { followUpOn: on });
    try {
      await invoke('set_follow_up', { key, on });
      return null;
    } catch (error) {
      this.patch(key, { followUpOn: before.followUpOn });
      return errorText(error);
    }
  }

  /**
   * "All read": every unread job of the current facet. Resolves with the keys for the undo
   * (`markUnread`), or the error text.
   */
  async markAllRead(): Promise<{ keys: JobKey[] } | { error: string }> {
    try {
      const keys = await invoke('mark_all_read', { facet: this.facet });
      for (const key of keys) this.patch(key, { unread: false });
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
      for (const key of keys) this.patch(key, { unread: true });
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
   * Deletes jobs for good (rows, text files and Excel rows; a later scan never brings them
   * back). Resolves with what the backend did, or the error text.
   */
  async deleteJobs(keys: JobKey[]): Promise<Deleted | { error: string }> {
    return this.forget(() => invoke('delete_jobs', { keys }), keys);
  }

  /** Deletes every archived job for good. */
  async emptyArchive(): Promise<Deleted | { error: string }> {
    const archived = this.rows.filter((job) => job.archived).map((job) => job.key);
    return this.forget(() => invoke('empty_archive'), archived);
  }

  private async forget(
    command: () => Promise<Deleted>,
    keys: JobKey[],
  ): Promise<Deleted | { error: string }> {
    try {
      const deleted = await command();
      const gone = new Set(keys.map(keyOf));
      const rows = this.rows.filter((job) => !gone.has(keyOf(job.key)));
      this.total = Math.max(0, this.total - (this.rows.length - rows.length));
      this.rows = rows;
      if (this.selected !== null && gone.has(keyOf(this.selected))) this.clearSelection();
      await this.refreshCounts();
      return deleted;
    } catch (error) {
      return { error: errorText(error) };
    }
  }

  /** Stores the note of a job (blank = none); resolves with the error text, or null. */
  async setNote(key: JobKey, note: string): Promise<string | null> {
    try {
      await invoke('set_note', { key, note });
      this.patchDetail(key, { note: note.trim() === '' ? null : note });
      return null;
    } catch (error) {
      return errorText(error);
    }
  }

  /**
   * Archives a job or lists it again. The row leaves a list it no longer belongs to; on an
   * error the list loads again. Resolves with the error text, or null.
   */
  async archive(key: JobKey, archived: boolean): Promise<string | null> {
    this.patch(key, { archived });
    this.dropStray(key);
    try {
      await invoke('set_archived', { key, archived });
      return null;
    } catch (error) {
      this.patch(key, { archived: !archived });
      void this.load(true);
      return errorText(error);
    }
  }

  /** The prompt for a deep analysis of a job in any AI chat. */
  async aiPrompt(key: JobKey): Promise<string> {
    return invoke('ai_prompt', { key });
  }

  /** One prompt that compares the best current matches (3 to 5) in any AI chat. */
  async aiPromptTop(limit: number): Promise<string> {
    return invoke('ai_prompt_top', { limit });
  }

  /** A listed row that no longer belongs to the facet leaves the list. */
  private dropStray(key: JobKey): void {
    const row = this.rows.find((job) => sameKey(job.key, key));
    if (!row || inFacet(row, this.facet)) return;
    this.rows = this.rows.filter((job) => !sameKey(job.key, key));
    this.total = Math.max(0, this.total - 1);
  }

  private detailOf(key: JobKey): JobDetail | null {
    return this.detail && sameKey(this.detail.job.key, key) ? this.detail : null;
  }

  /** Change the reader's own fields (the note) of a job it shows. */
  private patchDetail(key: JobKey, change: Partial<Pick<JobDetail, 'note'>>): void {
    const detail = this.detailOf(key);
    if (detail !== null) this.detail = { ...detail, ...change };
  }

  /** Change a job the page holds (a row, the reader) in place, moving the counts with it. */
  private patch(key: JobKey, change: Partial<JobView>): void {
    const row = this.rows.find((job) => sameKey(job.key, key)) ?? null;
    const shown = this.detail && sameKey(this.detail.job.key, key) ? this.detail.job : null;
    const before = row ?? shown;
    if (before === null) return;
    const after = { ...before, ...change };
    // A listed row belongs to the list's counts; every job belongs to the overall ones.
    if (row !== null) {
      this.counts = moved(this.counts, row, after);
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
      this.rows = this.rows.with(index, job);
    } else if (
      (fresh || this.rows.length >= this.total) &&
      this.search.trim() === '' &&
      inFacet(job, this.facet)
    ) {
      // Under Neu an unread excluded job also shows (grey, behind the divider), uncounted.
      const at = isExcluded(job) ? this.rows.findIndex(isExcluded) : 0;
      const rows = [...this.rows];
      rows.splice(at < 0 ? rows.length : at, 0, job);
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
