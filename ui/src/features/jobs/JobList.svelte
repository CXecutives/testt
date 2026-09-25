<!--
  The job list: rows in windows of 60 (a sentinel at the end shows the next window), a new
  job of a run fades in where it lands (the rows below simply make room). Rows move only for
  the user's own change and for the re-sort at the end of a run: after the sort switch, Neu |
  Alle or a filter the rows on screen glide to their new place (150 ms); rows off screen and
  new rows are simply there. A search and live updates never move anything. Excluded jobs
  sit grey behind the divider "Ausgeschlossen" with a soft count (under Neu or a filter too,
  there without the count: the rows below are only a part of the excluded jobs). A page that
  fails to load while scrolling says so at the end of the list, with a retry. A search looks
  in the list's place; under its hits a quiet link names each other place with hits ("Auch
  im Archiv (2)"), which keeps the search. Each row's tools are the job's actions where it
  is (Archivieren, Löschen, the star; in the Papierkorb Wiederherstellen, Endgültig
  löschen); a row the user moves out folds away. Rows are chosen like in a mail app: a
  click opens one, Ctrl+click (Cmd on macOS) takes one in or out, Shift+click a range; the
  highlight shows what is chosen, and in one column choosing never opens a job. One coral
  bar marks the open job's row and slides from row to row (RowBar); the other chosen rows
  mark themselves. The list is one Tab stop: the open row (else the row last focused, else
  the first) takes Tab, the arrows move from there; the row tools are for the pointer.
  Back in the Jobs view, the open
  job's row is in view again. A row move that fails says so in the list header. An
  empty inbox says where jobs come from (an alert on each portal, older mails; reading the
  whole mailbox asks first, as in Einstellungen). Every empty
  state has exactly one reason and at most one way out (secondary: the header holds the
  view's primary). Without a mailbox one slim note at the top says how to connect one;
  without a usable profile one says that there is no fit without it and leads to the Profil
  view (the rings stay, empty).
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import Button from '$components/Button.svelte';
  import Count from '$components/Count.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import Dialog from '$components/Dialog.svelte';
  import JobRow, { type RowTool, type SelectHow } from '$components/JobRow.svelte';
  import Notice from '$components/Notice.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import { nearEnd } from '$lib/actions/nearEnd';
  import { t } from '$lib/i18n/t';
  import { invoke } from '$lib/ipc/api';
  import { errorText } from '$lib/i18n/texts';
  import type { JobView, Place, Portal } from '$lib/ipc/types';
  import { play, staggerLimit } from '$lib/motion/motion';
  import { rowCollapse, rowEnter } from '$lib/motion/transitions';
  import { app } from '$lib/state/app.svelte';
  import { isExcluded, jobs, keyOf, placeOf, sameKey } from '$lib/state/jobs.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { editor } from '$lib/state/profile.svelte';
  import { run } from '$lib/state/run.svelte';
  import { viewport } from '$lib/state/viewport.svelte';
  import { actionsOf, disarm, guarded, hasStar, move, moving, purge, toggleStar } from './actions';
  import RowBar from './RowBar.svelte';
  import { selection } from './selection.svelte';

  const SKELETON_ROWS = [0, 1, 2, 3, 4, 5];

  const shown = $derived(jobs.shown);
  const active = $derived(shown.filter((job) => !isExcluded(job)));
  const excluded = $derived(shown.filter(isExcluded));
  // The number only where the divider heads every excluded job of the list (its count, like
  // the facet's, follows the search).
  const excludedCount = $derived(jobs.facet === 'all' ? jobs.counts.excluded : null);
  // "No jobs in the alert mails" only after a fetch that read the mailbox.
  const lastFetch = $derived(run.summary ?? app.state?.lastRun ?? null);
  const mailRead = $derived(lastFetch?.outcome.kind === 'completed' && lastFetch.scan !== null);
  const searching = $derived(jobs.search.trim() !== '');
  const profileMissing = $derived(app.state !== null && !app.hasProfile);
  // No profile: the fit needs one. One that is there but cannot be used is named.
  const profileNote = $derived.by(() => {
    const profile = app.state?.profile ?? null;
    if (profile === null) {
      return { heading: null, text: t.list.noProfile, label: t.list.createProfile };
    }
    return {
      heading: profile.parseError ? t.list.profileUnreadable : t.list.profileEmpty,
      text: t.list.profileBrokenText,
      label: t.list.openProfile,
    };
  });

  function toProfile(): void {
    // No profile yet: straight into the empty form, one click.
    if (app.state?.profile == null) editor.create();
    navigation.go('profile');
  }
  const mailboxMissing = $derived(app.state !== null && !app.hasMailbox);
  // Jobs without a match get one soon while a run goes or a rescore is pending.
  const pending = $derived(app.hasProfile && (run.active || (app.state?.matchPending ?? 0) > 0));

  /** The rows in the order they stand: the active ones, then the excluded ones. */
  const order = $derived([...active, ...excluded]);
  /** The rows the list shows (a row on the page that is not among them is leaving). */
  const listed = $derived(new Set(shown.map((job) => keyOf(job.key))));
  const openKey = $derived(jobs.selected ? keyOf(jobs.selected) : null);
  /** The open job's row while it shows as selected: the list's one bar marks it (the other
   *  rows of a choice mark themselves). */
  const marked = $derived(
    openKey !== null && (selection.size === 0 || selection.keys.includes(openKey)) ? openKey : null,
  );
  const markedExcluded = $derived(
    marked !== null && excluded.some((job) => keyOf(job.key) === marked),
  );

  /**
   * Open the job at `target` of the whole list (the keyboard), also one below the rows
   * mounted or loaded so far: the pages up to it load, its row mounts, then it scrolls into
   * view and takes the focus.
   */
  async function openAt(target: number | 'last'): Promise<void> {
    const job = await jobs.reach(target, true);
    if (job === null) return;
    selection.only(job);
    if (!sameKey(jobs.selected, job.key)) void jobs.select(job, true);
  }

  /** ArrowUp / ArrowDown (lib/input/input.ts): the previous or next job opens; with none
   *  open, the first (down) or the last of the list (up). */
  export function step(by: -1 | 1): void {
    const at = jobs.visible.findIndex((job) => sameKey(jobs.selected, job.key));
    void openAt(at === -1 ? (by === 1 ? 0 : 'last') : Math.max(0, at + by));
  }

  /** Home / End: the first or the last job of the list. */
  export function edge(last: boolean): void {
    void openAt(last ? 'last' : 0);
  }

  /** The row focused last (the list's one Tab stop when no job is open). */
  let focused = $state<string | null>(null);
  /** The row Tab stops at: the open job's, else the one focused last, else the first. */
  const tabStop = $derived.by(() => {
    if (openKey !== null && listed.has(openKey)) return openKey;
    if (focused !== null && listed.has(focused)) return focused;
    const first = order[0];
    return first ? keyOf(first.key) : null;
  });

  function onfocusin(event: FocusEvent): void {
    const item = (event.target as Element | null)?.closest<HTMLElement>('[data-key]');
    if (item?.dataset['key']) focused = item.dataset['key'];
  }

  // Back in the Jobs view (the list is built anew): what the header said about an action in
  // another list goes, and the open job's row comes into view.
  onMount(() => {
    jobs.quiet();
    const open = jobs.selected;
    if (open !== null && jobs.visible.some((job) => sameKey(job.key, open))) {
      void jobs.reach(open, false);
    }
  });

  // A job the keys opened, or the open job after a re-sort: once its row is mounted it
  // scrolls into view (and takes the focus when the keys opened it).
  $effect(() => {
    const want = jobs.reveal;
    void shown;
    if (want === null) return;
    untrack(() => {
      const row = list?.querySelector<HTMLElement>(`[data-key="${CSS.escape(want.key)}"] .row`);
      if (!row) return;
      jobs.reveal = null;
      if (want.focus) row.focus({ preventScroll: true });
      row.scrollIntoView({ block: 'nearest' });
    });
  });

  /** A click opens the job (the open one stays open); with Ctrl/Cmd or Shift it chooses. */
  function select(job: JobView, how: SelectHow): void {
    if (!how.range && !how.toggle) {
      selection.only(job);
      if (!sameKey(jobs.selected, job.key)) void jobs.select(job, true);
      return;
    }
    if (how.range) selection.range(job, order);
    else selection.toggle(job);
    settle(true);
  }

  /**
   * One chosen row is no selection: that job simply opens (like a mail app), and a Ctrl+click
   * that took the open job out of the choice closes it. In one column choosing never opens a
   * job: the list stays, and its header's bar acts on the chosen rows. `click`: the user's
   * click led here (the job then counts as read), not a row that left the list.
   */
  function settle(click: boolean): void {
    if (viewport.narrow) return;
    const [only, ...more] = selection.jobs(order);
    if (more.length > 0) return;
    if (only === undefined) {
      if (click && selection.size === 0) jobs.clearSelection();
      return;
    }
    selection.only(only);
    if (!sameKey(jobs.selected, only.key)) void jobs.select(only, click);
  }

  // Rows that leave the list (a move, a reload) leave the choice too.
  $effect(() => {
    const listed = new Set(jobs.rows.map((row) => keyOf(row.key)));
    untrack(() => {
      if (selection.prune(listed)) settle(false);
    });
  });

  // Another list, another search or order: the choice starts anew, and a click right away
  // counts (the guard after a move is for rows that slid under the pointer).
  $effect(() => {
    void jobs.facet;
    void jobs.search;
    void jobs.sortChoice;
    untrack(() => {
      selection.clear();
      disarm();
    });
  });

  // A search looks in the list's place; the other places with hits are named under them (the
  // favourites are of the inbox and the archive, so only the trash is elsewhere).
  const place = $derived(placeOf(jobs.facet));
  const PLACES: readonly Place[] = ['inbox', 'archive', 'trash'];
  const COUNT_OF: Record<Place, 'inbox' | 'archive' | 'trash'> = {
    inbox: 'inbox',
    archive: 'archive',
    trash: 'trash',
  };
  const FACET_OF: Record<Place, 'all' | 'archived' | 'trash'> = {
    inbox: 'all',
    archive: 'archived',
    trash: 'trash',
  };
  const elsewhere = $derived(
    searching
      ? PLACES.filter((other) =>
          jobs.facet === 'favourites' ? other === 'trash' : other !== place,
        )
          .map((other) => ({ place: other, count: jobs.counts[COUNT_OF[other]] }))
          .filter((hit) => hit.count > 0)
      : [],
  );
  const PORTALS = $derived((app.state?.portals ?? []).filter((p) => p.enabled));
  /** A portal's page that did not open (said under the links). */
  let portalError = $state<string | null>(null);
  function openPortal(portal: Portal): void {
    portalError = null;
    invoke('open_target', { target: { kind: 'portalHome', portal } }).catch((error: unknown) => {
      portalError = errorText(error);
    });
  }
  /** "Ältere Mails lesen" asks first, like Einstellungen: it fetches more portal pages. */
  let confirmOlder = $state(false);

  /* --------------------------------------------------------------------- glides */

  let list = $state<HTMLElement | null>(null);
  /** The rows of the list (both groups and the divider), which the bar follows. */
  let groups = $state<HTMLElement | null>(null);
  let rowBar = $state<RowBar | null>(null);
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

  /** Every row on screen glides from where it stood (150 ms); rows far away are simply there.
   *  Every box is read before the first move starts: a read after a started animation lays
   *  the whole list out again, once per row. */
  function glide(root: HTMLElement, from: Map<string, number>): void {
    const height = window.innerHeight;
    const moves: [HTMLElement, number][] = [];
    for (const row of rowsOf(root)) {
      const was = from.get(row.dataset.key ?? '');
      if (was === undefined) continue;
      const box = row.getBoundingClientRect();
      const shift = Math.round(was - box.top);
      const seen = (top: number): boolean => top < height && top + box.height > 0;
      if (shift === 0 || (!seen(was) && !seen(box.top))) continue;
      moves.push([row, Math.max(-height, Math.min(height, shift))]);
    }
    for (const [row, offset] of moves) {
      const motion = play(row, [{ transform: `translateY(${offset}px)` }, { transform: 'none' }], {
        duration: 'base',
      });
      motion?.addEventListener('finish', () => motion.cancel());
      if (row.hasAttribute('data-open')) rowBar?.shift(offset);
    }
  }

  /** New jobs of a run that have entered already (each one fades in once; not reactive). */
  let entered: Record<string, true> = {};

  /** A job new in this run fades in where it lands, among the first --stagger-max rows. */
  function enterFresh(root: HTMLElement): void {
    if (jobs.fresh.size === 0) {
      entered = {};
      return;
    }
    for (const job of order.slice(0, staggerLimit())) {
      const key = keyOf(job.key);
      if (!jobs.fresh.has(key) || key in entered) continue;
      entered[key] = true;
      const row = root.querySelector<HTMLElement>(`[data-key="${CSS.escape(key)}"]`);
      if (row) rowEnter(row);
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

  // After the DOM has the new rows: glide from the noted places, at the start of the next
  // frame (before it is drawn): the new rows are laid out once, there, and not a second time
  // inside the task that built them. A new job of a run fades in.
  $effect(() => {
    void shown;
    untrack(() => {
      const root = list;
      const from = before;
      before = null;
      if (root === null) return;
      if (from !== null) requestAnimationFrame(() => glide(root, from));
      enterFresh(root);
    });
  });

  function pin(job: JobView): void {
    if (!guarded()) toggleStar([job]);
  }

  /** Ends with "Endgültig löschen" of a row: the dialog asks first. */
  let purging = $state<JobView | null>(null);
  let purgeBusy = $state(false);
  let purgeError = $state<string | null>(null);

  /** The row's tools: the job's actions where it is (deleting for good waits for a run: the
   *  backend refuses meanwhile). */
  function toolsOf(job: JobView): RowTool[] {
    return actionsOf(job.place).map((action) => ({
      id: action.id,
      icon: action.icon,
      label: action.label,
      disabled: action.id === 'purge' && run.active,
      disabledReason: run.busyText,
      onclick: () => {
        if (action.id === 'purge') {
          purgeError = null;
          purging = job;
        } else {
          void move([job], action.id).then((error) => (jobs.actionError = error));
        }
      },
    }));
  }

  async function purgeRow(): Promise<void> {
    if (purging === null) return;
    purgeBusy = true;
    purgeError = await purge([purging]);
    purgeBusy = false;
    if (purgeError === null) purging = null;
  }
</script>

{#snippet alsoIn()}
  <div class="also" data-testid="also-in">
    {#each elsewhere as hit (hit.place)}
      <Button
        variant="link"
        size="sm"
        label={t.place.alsoIn[hit.place](hit.count)}
        testid="also-{hit.place}"
        onclick={() => jobs.setFacet(FACET_OF[hit.place])}
      />
    {/each}
  </div>
{/snippet}

<div
  class="list"
  bind:this={list}
  data-testid="job-list"
  aria-label={t.list.label}
  aria-busy={jobs.status === 'loading'}
  {onfocusin}
>
  {#if mailboxMissing}
    <div class="note">
      <Notice
        tone="info"
        variant="row"
        text={t.list.noMailbox}
        action={{ label: t.list.connectMailbox, onclick: () => navigation.go('settings') }}
        testid="no-mailbox"
      />
    </div>
  {/if}

  {#if profileMissing}
    <div class="note">
      <Notice
        tone="info"
        variant="row"
        heading={profileNote.heading}
        text={profileNote.text}
        action={{ label: profileNote.label, onclick: toProfile }}
        testid="no-profile"
      />
    </div>
  {/if}

  {#if jobs.status === 'error'}
    <div class="empty">
      <EmptyState
        icon="triangle-alert"
        tone="danger"
        text={jobs.error ?? t.list.loadFailed}
        secondary={{
          label: t.common.retry,
          icon: 'rotate-ccw',
          onclick: () => {
            void jobs.load();
            void jobs.loadOverview();
          },
        }}
        testid="list-error"
      />
    </div>
  {:else if jobs.rows.length === 0 && jobs.status !== 'ready'}
    <!-- Only once the list has taken a while (jobs.slow): then at once, never blank rows. -->
    {#if jobs.slow}
      <div class="skeletons" data-testid="list-skeleton">
        {#each SKELETON_ROWS as index (index)}
          <div class="skeleton-row">
            <Skeleton shape="circle" size="sm" />
            <span class="lines">
              <span class="line title"><Skeleton width={70} /></span>
              <span class="line"><Skeleton width={45} /></span>
            </span>
          </div>
        {/each}
      </div>
    {/if}
  {:else if jobs.visible.length === 0 && jobs.status === 'ready'}
    <div class="empty">
      {#if searching}
        <div class="stack">
          <!-- Under Neu or Favoriten a search that Alle would find says so and goes there. -->
          {#if (jobs.facet === 'new' || jobs.facet === 'favourites') && jobs.counts.inbox > 0}
            <EmptyState
              icon="search"
              tone="neutral"
              text={t.list.noHitIn[jobs.facet](jobs.search.trim())}
              secondary={{ label: t.list.searchAll, onclick: () => jobs.setFacet('all') }}
              testid="empty-search"
            />
          {:else}
            <EmptyState
              icon="search"
              tone="neutral"
              text={t.list.noHit(jobs.search.trim())}
              secondary={{ label: t.field.clear, icon: 'x', onclick: () => jobs.setSearch('') }}
              testid="empty-search"
            />
          {/if}
          {#if elsewhere.length > 0}{@render alsoIn()}{/if}
        </div>
      {:else if place !== 'inbox'}
        <EmptyState
          icon={place === 'trash' ? 'trash-2' : 'archive'}
          tone="neutral"
          text={t.place.empty[place]}
          testid="empty-place-{place}"
        />
      {:else if jobs.facet === 'favourites'}
        <EmptyState
          icon="star"
          tone="neutral"
          text={t.list.emptyFavourites}
          testid="empty-favourites"
        />
      {:else if jobs.facet === 'new' && jobs.counts.inbox > 0}
        <EmptyState
          icon="check"
          tone="success"
          text={t.list.emptyNew}
          secondary={{ label: t.list.showAll, onclick: () => jobs.setFacet('all') }}
          testid="empty-new"
        />
      {:else if run.active || !mailRead}
        <!-- A fetch that goes, or none yet: only what comes (no setup links). -->
        <EmptyState
          icon="inbox"
          tone="neutral"
          text={run.active ? t.list.emptyWhileRun : t.list.emptyAll}
          testid="empty-all"
        />
      {:else}
        <div class="sources">
          <EmptyState icon="inbox" tone="neutral" text={t.list.emptyAfterRun} testid="empty-all" />
          <p class="sources-text">{t.list.emptySources}</p>
          <div class="sources-actions">
            {#each PORTALS as portal (portal.portal)}
              <Button
                variant="ghost"
                size="sm"
                icon="external-link"
                external
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
                disabled={run.fetchBlocked !== null}
                disabledReason={run.fetchBlocked}
                testid="read-older"
                onclick={() => (confirmOlder = true)}
              />
            {/if}
          </div>
          {#if portalError}
            <Notice tone="danger" variant="inline" text={portalError} testid="portal-error" />
          {/if}
        </div>
      {/if}
    </div>
  {:else}
    {#snippet row(job: JobView, open: boolean)}
      <!-- While rows are chosen the highlight shows exactly them (what the header's bar
           counts and a Ctrl+click takes out); else the open job. The open job's row is
           marked by the list's one bar, every other chosen row by its own. -->
      <JobRow
        {job}
        ring={!profileMissing}
        pending={pending && job.match === null}
        selected={selection.size > 0 ? selection.has(job) : open}
        bar={!open}
        tabbable={keyOf(job.key) === tabStop}
        onselect={select}
        onpin={hasStar(job.place) ? pin : null}
        tools={toolsOf(job)}
      />
    {/snippet}
    {#snippet group(items: JobView[])}
      {#each items as job (keyOf(job.key))}
        {@const key = keyOf(job.key)}
        {@const open = key === openKey}
        <div
          class="item"
          data-key={key}
          data-open={open ? '' : undefined}
          out:rowCollapse={{ on: moving.has(key) }}
        >
          {@render row(job, open)}
        </div>
      {/each}
    {/snippet}
    <div class="groups" bind:this={groups}>
      <!-- Another list is built anew: its old rows leave as one piece, not row by row. -->
      {#key jobs.generation}
        <div class="rows" data-testid="job-rows">
          {@render group(active)}
        </div>
        {#if excluded.length > 0}
          <div class="divider" data-testid="excluded-divider">
            <span class="divider-label">{t.list.excluded}</span>
            {#if excludedCount !== null}<Count
                value={excludedCount}
                tone="plain"
                testid="excluded-count"
              />{/if}
          </div>
          <div class="rows" data-testid="excluded-rows">
            {@render group(excluded)}
          </div>
        {/if}
      {/key}
    </div>
    {#if elsewhere.length > 0 && !jobs.more}{@render alsoIn()}{/if}
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
          <Skeleton width={60} late />
        </div>
      {/key}
    {/if}
  {/if}
  <RowBar
    bind:this={rowBar}
    rows={groups}
    open={marked}
    muted={markedExcluded}
    {listed}
    folding={moving}
    several={selection.size > 0}
    generation={jobs.generation}
  />
</div>

<Dialog
  bind:open={confirmOlder}
  heading={t.settings.fullMailboxHeading}
  text={t.settings.fullMailboxText}
  confirmLabel={t.settings.fullMailboxAction}
  testid="dialog-read-older"
  onconfirm={() => {
    confirmOlder = false;
    void run.start({ kind: 'fullMailbox' });
  }}
/>

<Dialog
  open={purging !== null}
  variant="danger"
  heading={t.actions.purgeHeading(1)}
  text={t.actions.purgeText}
  confirmLabel={t.actions.purge}
  busy={purgeBusy}
  error={purgeError}
  testid="dialog-purge"
  onconfirm={() => void purgeRow()}
  oncancel={() => (purging = null)}
/>

<style>
  /* The containing block of the rows' one selection bar (RowBar). */
  .list {
    position: relative;
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }

  /* A slim line at the top of the list (no mailbox, no profile). */
  .note {
    padding: var(--space-12) var(--pane-padding);
    border-bottom: var(--border-width) solid var(--border);
  }

  /* Search hits in the other places, quiet links under the hits of this one. */
  .also {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4) var(--space-16);
    padding: var(--space-12) var(--pane-padding);
  }

  .page-error {
    padding: var(--space-12) var(--pane-padding);
  }

  /* Plain block flow: a flex column adds nothing here and costs a little more each time the
     rows are laid out again. */
  .groups,
  .rows {
    display: block;
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

  .stack {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    width: 100%;
  }

  /* Under a centred empty state the links are centred too (under rows they start left). */
  .stack > .also {
    justify-content: center;
    padding-inline: 0;
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

  /* The block is centred, its links start on one line (their icons on one axis). */
  .sources-actions {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
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

  /* Laid out like a row (ListRow, JobRow): the ring and the title at its top, the next line
     under it, so nothing moves when the rows arrive. */
  .skeleton-row {
    display: flex;
    align-items: flex-start;
    gap: var(--space-12);
    height: var(--row-height);
    padding: var(--space-12) var(--pane-padding);
    border-bottom: var(--border-width) solid var(--border);
  }

  .lines {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
  }

  .line {
    display: flex;
    align-items: center;
    height: var(--leading-sm);
  }

  .line.title {
    height: var(--leading-title);
  }

  .sentinel {
    padding: var(--pane-padding);
  }
</style>
