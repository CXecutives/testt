<!--
  The job list: rows in windows of 60 (a sentinel at the end shows the next window), a new
  job of a run fades in where it lands (the rows below simply make room), and only a real
  re-sort moves rows: when the rows come back in another order (the sort switch, the re-sort
  at the end of a run) the rows on screen glide to their new place. Filtering, searching and
  live updates never move anything. Excluded jobs sit grey behind the divider
  "Ausgeschlossen n" (under Neu too, uncounted). Clicking the selected row again changes
  nothing (a native list keeps its selection). Every empty
  state has exactly one reason and at most one way out (secondary: the header holds the
  view's primary). Without a mailbox one note says how to connect one; a missing profile is
  said once, in the day overview.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Button from '$components/Button.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import JobRow from '$components/JobRow.svelte';
  import Notice from '$components/Notice.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import { nearEnd } from '$lib/actions/nearEnd';
  import { de } from '$lib/i18n/de';
  import type { JobView } from '$lib/ipc/types';
  import { play } from '$lib/motion/motion';
  import { rowIn } from '$lib/motion/transitions';
  import { app } from '$lib/state/app.svelte';
  import { isExcluded, jobs, keyOf, sameKey } from '$lib/state/jobs.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';

  const SKELETON_ROWS = [0, 1, 2, 3, 4, 5];

  const shown = $derived(jobs.shown);
  const active = $derived(shown.filter((job) => !isExcluded(job)));
  const excluded = $derived(shown.filter(isExcluded));
  const excludedCount = $derived(
    jobs.filter === null && jobs.facet === 'all'
      ? jobs.counts.excluded
      : jobs.visible.filter(isExcluded).length,
  );
  const searching = $derived(jobs.search.trim() !== '');
  const profileMissing = $derived(app.state !== null && !app.hasProfile);
  const mailboxMissing = $derived(app.state !== null && !app.hasMailbox);
  // Jobs without a match get one soon while a run goes or a rescore is pending.
  const pending = $derived(app.hasProfile && (run.active || (app.state?.matchPending ?? 0) > 0));

  function select(job: JobView): void {
    if (sameKey(jobs.selected, job.key)) return;
    void jobs.select(job, true);
  }

  /* ------------------------------------------------------------------ re-sort glide */

  let list = $state<HTMLElement | null>(null);
  /** Keys of the mounted rows as the DOM shows them now. */
  let order: string[] = [];
  /** Top edges of the rows before a re-sort, until the DOM has the new order. */
  let before: Map<string, number> | null = null;

  /** Some rows that were there before now stand in another order among themselves. */
  function resorted(previous: readonly string[], next: readonly string[]): boolean {
    const at = new Map(previous.map((key, index) => [key, index]));
    let last = -1;
    for (const key of next) {
      const index = at.get(key);
      if (index === undefined) continue;
      if (index < last) return true;
      last = index;
    }
    return false;
  }

  function rowsOf(root: HTMLElement): HTMLElement[] {
    return [...root.querySelectorAll<HTMLElement>('[data-key]')];
  }

  /** Every row on screen glides from where it stood (150 ms); rows far away are simply there. */
  function glide(root: HTMLElement, from: Map<string, number>): void {
    const height = window.innerHeight;
    for (const row of rowsOf(root)) {
      const was = from.get(row.dataset.key ?? '');
      if (was === undefined) continue;
      const box = row.getBoundingClientRect();
      const shift = Math.round(was - box.top);
      const seen = (top: number): boolean => top < height && top + box.height > 0;
      if (shift === 0 || (!seen(was) && !seen(box.top))) continue;
      const offset = Math.max(-height, Math.min(height, shift));
      const motion = play(row, [{ transform: `translateY(${offset}px)` }, { transform: 'none' }], {
        duration: 'base',
      });
      motion?.addEventListener('finish', () => motion.cancel());
    }
  }

  // Before the DOM changes: note where the rows stand when the new rows are a re-sort.
  $effect.pre(() => {
    const next = shown.map((job) => keyOf(job.key));
    untrack(() => {
      if (list !== null && resorted(order, next)) {
        before = new Map(
          rowsOf(list).map((row) => [row.dataset.key ?? '', row.getBoundingClientRect().top]),
        );
      }
      order = next;
    });
  });

  // After the DOM has the new order: glide from the noted places.
  $effect(() => {
    void shown;
    untrack(() => {
      if (list !== null && before !== null) glide(list, before);
      before = null;
    });
  });

  function pin(job: JobView): void {
    void jobs.pin(job.key, !job.pinned);
  }
</script>

<div
  class="list"
  bind:this={list}
  data-testid="job-list"
  aria-label={de.list.label}
  aria-busy={jobs.status === 'loading'}
>
  {#if mailboxMissing}
    <div class="note">
      <Notice
        tone="info"
        text={de.list.noMailbox}
        action={{ label: de.list.connectMailbox, onclick: () => navigation.go('settings') }}
        testid="no-mailbox"
      />
    </div>
  {/if}
  {#if jobs.filter !== null}
    <div class="filter" data-testid="filter">
      <span class="filter-label">{de.list.filter[jobs.filter]}</span>
      <Button
        variant="ghost"
        size="sm"
        icon="x"
        iconOnly
        label={de.list.clearFilter}
        testid="clear-filter"
        onclick={() => jobs.setFilter(null)}
      />
    </div>
  {/if}

  {#if jobs.status === 'error'}
    <div class="empty">
      <EmptyState
        icon="triangle-alert"
        tone="danger"
        text={jobs.error ?? de.list.loadFailed}
        secondary={{ label: de.common.retry, icon: 'rotate-ccw', onclick: () => void jobs.load() }}
        testid="list-error"
      />
    </div>
  {:else if jobs.rows.length === 0 && jobs.status !== 'ready'}
    {#if jobs.slow || app.slow}
      <div class="skeletons" data-testid="list-skeleton">
        {#each SKELETON_ROWS as index (index)}
          <div class="skeleton-row">
            <Skeleton shape="circle" size="sm" />
            <span class="lines">
              <Skeleton width={70} />
              <Skeleton width={45} />
            </span>
          </div>
        {/each}
      </div>
    {/if}
  {:else if jobs.visible.length === 0 && jobs.status === 'ready'}
    <div class="empty">
      {#if searching}
        <EmptyState
          icon="search"
          tone="neutral"
          text={de.list.noHit(jobs.search.trim())}
          secondary={{ label: de.field.clear, icon: 'x', onclick: () => jobs.setSearch('') }}
          testid="empty-search"
        />
      {:else if jobs.filter !== null}
        <EmptyState
          icon="inbox"
          tone="neutral"
          text={de.list.emptyFilter}
          secondary={{ label: de.list.clearFilter, onclick: () => jobs.setFilter(null) }}
          testid="empty-filter"
        />
      {:else if jobs.facet === 'new' && jobs.counts.all > 0}
        <EmptyState
          icon="check"
          tone="success"
          text={de.list.emptyNew}
          secondary={{ label: de.list.showAll, onclick: () => jobs.setFacet('all') }}
          testid="empty-new"
        />
      {:else}
        <EmptyState
          icon="inbox"
          tone="neutral"
          text={app.state?.lastRun ? de.list.emptyAfterRun : de.list.emptyAll}
          testid="empty-all"
        />
      {/if}
    </div>
  {:else}
    {#snippet row(job: JobView)}
      <JobRow
        {job}
        ring={!profileMissing}
        pending={pending && job.match === null}
        selected={sameKey(jobs.selected, job.key)}
        onselect={select}
        onpin={pin}
      />
    {/snippet}
    {#snippet group(items: JobView[], offset: number)}
      {#each items as job, index (keyOf(job.key))}
        {@const key = keyOf(job.key)}
        <div
          class="item"
          data-key={key}
          in:rowIn={{ index: offset + index, fresh: jobs.fresh.has(key) }}
        >
          {@render row(job)}
        </div>
      {/each}
    {/snippet}
    <div class="rows" data-testid="job-rows">
      {@render group(active, 0)}
    </div>
    {#if excluded.length > 0}
      <div class="divider" data-testid="excluded-divider">
        <span>{de.list.excluded(excludedCount)}</span>
      </div>
      <div class="rows" data-testid="excluded-rows">
        {@render group(excluded, active.length)}
      </div>
    {/if}
    {#if jobs.more}
      {#key shown.length}
        <div class="sentinel" use:nearEnd={() => void jobs.grow()}>
          <Skeleton width={60} />
        </div>
      {/key}
    {/if}
  {/if}
</div>

<style>
  .list {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }

  .note {
    padding: var(--pane-padding);
    border-bottom: var(--border-width) solid var(--border);
  }

  .filter {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    align-self: flex-start;
    margin: var(--pane-padding) var(--pane-padding) var(--space-4);
    padding-left: var(--space-12);
    border-radius: var(--radius-full);
    background-color: var(--surface-muted);
    color: var(--text);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .rows {
    display: flex;
    flex-direction: column;
  }

  .divider {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-24) var(--pane-padding) var(--space-8);
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .divider::after {
    flex: 1;
    height: var(--border-width);
    background-color: var(--border);
    content: '';
  }

  .empty {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    padding: var(--space-32) var(--pane-padding);
  }

  .skeletons {
    display: flex;
    flex-direction: column;
  }

  .skeleton-row {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    height: var(--row-height);
    padding: 0 var(--pane-padding);
    border-bottom: var(--border-width) solid var(--border);
  }

  .lines {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-8);
  }

  .sentinel {
    padding: var(--pane-padding);
  }
</style>
