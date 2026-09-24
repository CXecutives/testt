<!--
  The job list: rows in windows of 60 (a sentinel at the end shows the next window), a new
  job of a run fades in where it lands (the rows below simply make room). Rows move only for
  the user's own change and for the re-sort at the end of a run: after the sort switch, Neu |
  Alle or a filter the rows on screen glide to their new place (150 ms); rows off screen and
  new rows are simply there. A search and live updates never move anything. Excluded jobs
  sit grey behind the divider "Ausgeschlossen" with a soft count (under Neu or a filter too,
  there without the count: the rows below are only a part of the excluded jobs). A page that
  fails to load while scrolling says so at the end of the list, with a retry. Clicking the
  selected row again closes it (back to the day overview). At the end of Alle a divider
  leads to the archived jobs. An empty list says where jobs come from (an alert on each
  portal, older mails). Every empty
  state has exactly one reason and at most one way out (secondary: the header holds the
  view's primary). Without a mailbox one note says how to connect one; a missing profile is
  said once, in the day overview.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Button from '$components/Button.svelte';
  import Count from '$components/Count.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import JobRow from '$components/JobRow.svelte';
  import Notice from '$components/Notice.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import { nearEnd } from '$lib/actions/nearEnd';
  import { t } from '$lib/i18n/t';
  import { invoke } from '$lib/ipc/api';
  import type { JobView, Portal } from '$lib/ipc/types';
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

  /** A click on the selected row closes it again: back to the day overview. */
  function select(job: JobView): void {
    if (sameKey(jobs.selected, job.key)) jobs.clearSelection();
    else void jobs.select(job, true);
  }
  const hiddenCount = $derived((jobs.overviewCounts ?? jobs.counts).archived);
  const PORTALS = $derived((app.state?.portals ?? []).filter((p) => p.enabled));
  function openPortal(portal: Portal): void {
    invoke('open_target', { target: { kind: 'portalHome', portal } }).catch(() => undefined);
  }

  /* --------------------------------------------------------------------- glides */

  let list = $state<HTMLElement | null>(null);
  /** The next new rows come from the user's own change (sort, facet, filter) or from the
   *  re-sort at the end of a run: the rows on screen glide to their new place. */
  let armed = false;
  /** Top edges of the rows before such a change, until the DOM has the new rows. */
  let before: Map<string, number> | null = null;
  let last = untrack(() => ({
    sort: jobs.sortChoice,
    facet: jobs.facet,
    filter: jobs.filter,
    active: run.active,
  }));

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

  // What changed the list: the user's sort, facet or filter (never while a run streams new
  // rows in), or the end of a run. A search and live updates arm nothing.
  $effect.pre(() => {
    const now = {
      sort: jobs.sortChoice,
      facet: jobs.facet,
      filter: jobs.filter,
      active: run.active,
    };
    untrack(() => {
      const chosen =
        now.sort !== last.sort || now.facet !== last.facet || now.filter !== last.filter;
      if ((chosen && !now.active) || (last.active && !now.active)) armed = true;
      last = now;
    });
  });

  // Before the DOM changes: note where the rows stand. The glide stays armed until the
  // change has loaded (a filter shows its rows at once and again once every page is in).
  $effect.pre(() => {
    void shown;
    untrack(() => {
      if (!armed || list === null) return;
      before = new Map(
        rowsOf(list).map((row) => [row.dataset.key ?? '', row.getBoundingClientRect().top]),
      );
      if (jobs.status !== 'loading') armed = false;
    });
  });

  // After the DOM has the new rows: glide from the noted places.
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
  aria-label={t.list.label}
  aria-busy={jobs.status === 'loading'}
>
  {#if mailboxMissing}
    <div class="note">
      <Notice
        tone="info"
        text={t.list.noMailbox}
        action={{ label: t.list.connectMailbox, onclick: () => navigation.go('settings') }}
        testid="no-mailbox"
      />
    </div>
  {/if}

  {#if jobs.status === 'error'}
    <div class="empty">
      <EmptyState
        icon="triangle-alert"
        tone="danger"
        text={jobs.error ?? t.list.loadFailed}
        secondary={{ label: t.common.retry, icon: 'rotate-ccw', onclick: () => void jobs.load() }}
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
          text={t.list.noHit(jobs.search.trim())}
          secondary={{ label: t.field.clear, icon: 'x', onclick: () => jobs.setSearch('') }}
          testid="empty-search"
        />
      {:else if jobs.filter !== null}
        <EmptyState
          icon="inbox"
          tone="neutral"
          text={t.list.emptyFilter}
          secondary={{ label: t.list.clearFilter, onclick: () => jobs.setFilter(null) }}
          testid="empty-filter"
        />
      {:else if jobs.facet === 'applications'}
        <EmptyState
          icon="inbox"
          tone="neutral"
          text={t.list.emptyApplications}
          testid="empty-applications"
        />
      {:else if jobs.facet === 'archived'}
        <EmptyState
          icon="inbox"
          tone="neutral"
          text={t.list.emptyHidden}
          secondary={{ label: t.list.showAll, onclick: () => jobs.setFacet('all') }}
          testid="empty-hidden"
        />
      {:else if jobs.facet === 'new' && jobs.counts.all > 0}
        <EmptyState
          icon="check"
          tone="success"
          text={t.list.emptyNew}
          secondary={{ label: t.list.showAll, onclick: () => jobs.setFacet('all') }}
          testid="empty-new"
        />
      {:else}
        <div class="sources">
          <EmptyState
            icon="inbox"
            tone="neutral"
            text={mailRead ? t.list.emptyAfterRun : t.list.emptyAll}
            testid="empty-all"
          />
          <p class="sources-text">{t.list.emptySources}</p>
          <div class="sources-actions">
            {#each PORTALS as portal (portal.portal)}
              <Button
                variant="ghost"
                size="sm"
                icon="external-link"
                label={t.list.createAlert(t.portal[portal.portal])}
                testid="alert-{portal.portal}"
                onclick={() => openPortal(portal.portal)}
              />
            {/each}
            {#if app.hasMailbox}
              <Button
                variant="ghost"
                size="sm"
                icon="mail"
                label={t.list.readOlder}
                disabled={run.active}
                disabledReason={run.busyText}
                testid="read-older"
                onclick={() => void run.start({ kind: 'fullMailbox' })}
              />
            {/if}
          </div>
        </div>
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
        <span class="divider-label">{t.list.excluded}</span>
        {#if excludedCount !== null}<Count value={excludedCount} testid="excluded-count" />{/if}
      </div>
      <div class="rows" data-testid="excluded-rows">
        {@render group(excluded, active.length)}
      </div>
    {/if}
    {#if jobs.facet === 'all' && jobs.filter === null && hiddenCount > 0 && !jobs.more}
      <div class="divider" data-testid="hidden-divider">
        <span class="divider-label">{t.list.hidden}</span>
        <Count value={hiddenCount} tone="plain" />
        <span class="divider-link">
          <Button
            variant="link"
            size="sm"
            label={t.list.showHidden}
            testid="show-hidden"
            onclick={() => jobs.setFacet('archived')}
          />
        </span>
      </div>
    {/if}
    {#if jobs.pageError}
      <div class="page-error">
        <Notice
          tone="warning"
          variant="row"
          text={t.list.pageFailed}
          action={{ label: t.common.retry, onclick: () => void jobs.grow() }}
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

  .rows {
    display: flex;
    flex-direction: column;
  }

  /* A navy sub-label with its soft count, then the hairline. */
  .divider {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    padding: var(--space-24) var(--pane-padding) var(--space-8);
    color: var(--text-label);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .divider::after {
    margin-left: var(--space-4);
    flex: 1;
    height: var(--border-width);
    background-color: var(--border);
    content: '';
  }

  .divider-link {
    display: flex;
    order: 2;
  }

  /* The empty list says where jobs come from: the portals' alerts, older mails. */
  .sources {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-12);
    max-width: var(--list-min);
  }

  .sources-text {
    color: var(--text-muted);
    font: var(--type-sm);
    text-align: center;
  }

  .sources-actions {
    display: flex;
    flex-direction: column;
    align-items: center;
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
