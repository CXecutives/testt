<!--
  The job list: rows in windows of 60 (a sentinel at the end shows the next window), a new
  job of a run fades in, FLIP when sort or filter reorders a list of up to 100 rows, excluded
  jobs grey behind the divider "Ausgeschlossen n" (under Neu or a filter too, there without
  a number: the rows below are only a part of the excluded jobs). A page that fails to load
  while scrolling says so at the end of the list, with a retry. Every empty
  state has exactly one reason and at most one way out (secondary: the header holds the
  view's primary). Without a mailbox one note says how to connect one; a missing profile is
  said once, in the day overview.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import JobRow from '$components/JobRow.svelte';
  import Notice from '$components/Notice.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import { nearEnd } from '$lib/actions/nearEnd';
  import { de } from '$lib/i18n/de';
  import type { JobView } from '$lib/ipc/types';
  import { FLIP_LIMIT, flip, rowIn } from '$lib/motion/transitions';
  import { app } from '$lib/state/app.svelte';
  import { isExcluded, jobs, keyOf, sameKey } from '$lib/state/jobs.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';

  const SKELETON_ROWS = [0, 1, 2, 3, 4, 5];

  const shown = $derived(jobs.shown);
  // The whole list decides about FLIP, not the rows mounted so far.
  const total = $derived(jobs.visible.length);
  const animated = $derived(total <= FLIP_LIMIT);
  const active = $derived(shown.filter((job) => !isExcluded(job)));
  const excluded = $derived(shown.filter(isExcluded));
  // The number only where the divider heads every excluded job of the list (its count, like
  // the facet's, follows the search).
  const excludedCount = $derived(
    jobs.filter === null && jobs.facet === 'all' ? jobs.counts.excluded : null,
  );
  // "No jobs in the alert mails" only after a fetch that read the mailbox.
  const lastFetch = $derived(run.summary ?? app.state?.lastRun ?? null);
  const mailRead = $derived(lastFetch?.outcome.kind === 'completed' && lastFetch.scan !== null);
  const searching = $derived(jobs.search.trim() !== '');
  const profileMissing = $derived(app.state !== null && !app.hasProfile);
  const mailboxMissing = $derived(app.state !== null && !app.hasMailbox);
  // Jobs without a match get one soon while a run goes or a rescore is pending.
  const pending = $derived(app.hasProfile && (run.active || (app.state?.matchPending ?? 0) > 0));

  function select(job: JobView): void {
    void jobs.select(job, true);
  }

  function pin(job: JobView): void {
    void jobs.pin(job.key, !job.pinned);
  }
</script>

<div
  class="list"
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
          text={mailRead ? de.list.emptyAfterRun : de.list.emptyAll}
          testid="empty-all"
        />
      {/if}
    </div>
  {:else}
    <!-- A list of up to FLIP_LIMIT rows moves with FLIP when sort or filter reorders it; a
         longer one has no animate directive at all (Svelte would measure every mounted row
         in every frame while the window fills). -->
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
      {#if animated}
        {#each items as job, index (keyOf(job.key))}
          <div
            class="item"
            animate:flip={{ count: total }}
            in:rowIn={{ index: offset + index, fresh: jobs.fresh.has(keyOf(job.key)) }}
          >
            {@render row(job)}
          </div>
        {/each}
      {:else}
        {#each items as job, index (keyOf(job.key))}
          <div
            class="item"
            in:rowIn={{ index: offset + index, fresh: jobs.fresh.has(keyOf(job.key)) }}
          >
            {@render row(job)}
          </div>
        {/each}
      {/if}
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
    {#if jobs.pageError}
      <div class="page-error">
        <Notice
          tone="warning"
          variant="row"
          text={de.list.pageFailed}
          action={{ label: de.common.retry, onclick: () => void jobs.grow() }}
          testid="page-error"
        />
      </div>
    {:else if jobs.more}
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

  .page-error {
    padding: var(--space-12) var(--pane-padding);
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
