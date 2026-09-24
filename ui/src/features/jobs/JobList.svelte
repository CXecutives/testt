<!--
  The job list: rows in windows of 60 (a sentinel at the end shows the next window),
  staggered entry, FLIP when sort or filter reorders up to 100 rows, excluded jobs grey
  behind the divider "Ausgeschlossen n". Every empty state has exactly one reason and at
  most one way out (secondary: the toolbar holds the view's primary).
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import JobRow from '$components/JobRow.svelte';
  import Notice from '$components/Notice.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import { nearEnd } from '$lib/actions/nearEnd';
  import { de } from '$lib/i18n/de';
  import { errorText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { JobView } from '$lib/ipc/types';
  import { flip, rowIn } from '$lib/motion/transitions';
  import { app } from '$lib/state/app.svelte';
  import { jobs, keyOf, sameKey } from '$lib/state/jobs.svelte';
  import { run } from '$lib/state/run.svelte';

  const SKELETON_ROWS = [0, 1, 2, 3, 4, 5];

  const shown = $derived(jobs.shown);
  const active = $derived(shown.filter((job) => job.match?.status !== 'excluded'));
  const excluded = $derived(shown.filter((job) => job.match?.status === 'excluded'));
  const excludedCount = $derived(
    jobs.filter === null && jobs.facet === 'all'
      ? jobs.counts.excluded
      : jobs.visible.filter((job) => job.match?.status === 'excluded').length,
  );
  const searching = $derived(jobs.search.trim() !== '');
  const profileMissing = $derived(app.state !== null && !app.hasProfile);
  // Jobs without a match get one soon while a run goes or a rescore is pending.
  const pending = $derived(app.hasProfile && (run.active || (app.state?.matchPending ?? 0) > 0));
  let pickError = $state<string | null>(null);

  function pick(): void {
    pickError = null;
    invoke('pick_profile')
      .then((profile) => {
        if (profile !== null) void app.load().then(() => jobs.load());
      })
      .catch((error: unknown) => (pickError = errorText(error)));
  }

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
  {#if profileMissing && app.state?.profile === null}
    <div class="note">
      <Notice
        tone="info"
        text={pickError ?? de.list.noProfile}
        action={{ label: de.list.pickProfile, onclick: pick }}
        testid="no-profile"
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
            <Skeleton shape="circle" size="md" />
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
    <div class="rows" data-testid="job-rows">
      {#each active as job, index (keyOf(job.key))}
        <div
          class="item"
          animate:flip={{ count: shown.length }}
          in:rowIn={{ index: index, count: shown.length }}
        >
          <JobRow
            {job}
            ring={!profileMissing}
            pending={pending && job.match === null}
            fresh={jobs.fresh.has(keyOf(job.key))}
            selected={sameKey(jobs.selected, job.key)}
            onselect={select}
            onpin={pin}
          />
        </div>
      {/each}
    </div>
    {#if excluded.length > 0}
      <div class="divider" data-testid="excluded-divider">
        <span>{de.list.excluded(excludedCount)}</span>
      </div>
      <div class="rows" data-testid="excluded-rows">
        {#each excluded as job, index (keyOf(job.key))}
          <div
            class="item"
            animate:flip={{ count: shown.length }}
            in:rowIn={{ index: active.length + index, count: shown.length }}
          >
            <JobRow
              {job}
              ring={!profileMissing}
              pending={pending && job.match === null}
              fresh={jobs.fresh.has(keyOf(job.key))}
              selected={sameKey(jobs.selected, job.key)}
              onselect={select}
              onpin={pin}
            />
          </div>
        {/each}
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
    padding: var(--pane-padding) var(--pane-padding) 0;
  }

  .filter {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    align-self: flex-start;
    margin: var(--pane-padding) var(--pane-padding) var(--space-4);
    padding-left: var(--space-12);
    border-radius: var(--radius-full);
    background-color: var(--surface-selected);
    color: var(--accent-text);
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
    font: var(--type-xs);
    font-weight: var(--weight-semibold);
    letter-spacing: var(--tracking-caps);
    text-transform: uppercase;
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
