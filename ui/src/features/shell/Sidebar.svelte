<!--
  The calm sidebar (196 px, icons only below 1100 px) on the cream: no surface of its own,
  the white sheet of the content is the divider. App icon and name live in the native title
  bar of the OS, so the sidebar starts with the views (on macOS below the traffic lights,
  whose 52 px band moves the window): the first sits on the line of the list's search field
  on Windows, each with its icon and no count (the list says how many are new); under
  Jobs (the inbox) the two other places of the jobs, Archiv and Papierkorb, quieter (a
  click on Jobs from there goes back to the inbox). An arrow at the end of the Jobs row hides and shows them
  (kept; while one of them is open they stay); in the rail it is a slim row under the Jobs
  icon. At the foot a quiet run status that opens the run in the Jobs view. It shows only
  while there is a run to open (before the first fetch the first-run page says it all), and
  it is said once: while the run card is on screen it steps aside. "Abrufen" lives in the
  list header.
  Below 1100 px it folds to its icons by the window width alone; there is no manual fold.
-->
<script lang="ts">
  import DragBand from '$components/DragBand.svelte';
  import SideNav, { type SideNavFold, type SideNavItem } from '$components/SideNav.svelte';
  import StatusLine from '$components/StatusLine.svelte';
  import { t } from '$lib/i18n/t';
  import { settled } from '$lib/motion/settled.svelte';
  import { fade } from '$lib/motion/transitions';
  import { dragBands } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { clock } from '$lib/state/clock.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { navigation, type ViewId } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import { shell } from '$lib/state/shell.svelte';
  import { viewport } from '$lib/state/viewport.svelte';

  const SIDEBAR_ID = 'sidebar';

  /** The views, and under Jobs its places (the inbox is Jobs itself). */
  type NavId = ViewId | 'archive' | 'trash';
  const items = $derived<SideNavItem<NavId>[]>([
    {
      id: 'jobs',
      label: t.nav.jobs,
      icon: 'briefcase',
      testid: 'nav-jobs',
      children: [
        { id: 'archive', label: t.place.archive, icon: 'archive', testid: 'nav-archive' },
        { id: 'trash', label: t.place.trash, icon: 'trash-2', testid: 'nav-trash' },
      ],
    },
    { id: 'profile', label: t.nav.profile, icon: 'user-round', testid: 'nav-profile' },
    { id: 'settings', label: t.nav.settings, icon: 'sliders-horizontal', testid: 'nav-settings' },
  ]);
  // The last fetch: a rescore of this session is no fetch.
  const fetched = $derived(run.summary?.kind === 'rescore' ? null : run.summary);
  const last = $derived(fetched ?? app.state?.lastRun ?? null);
  const failed = $derived(!run.active && last?.outcome.kind === 'failed');
  const status = $derived.by(() => {
    if (run.active) {
      if (run.status) return t.run.statusOf(run.status.code, run.status.portal);
      return run.step ? t.run.step[run.step] : t.run.kind[run.kind ?? 'fetch'];
    }
    if (last === null) return t.run.never;
    // The time moves on ("08:30" gains its date after midnight): read the shared clock.
    void clock.now;
    // What happened last and when, in the same short form either way (one line).
    return failed ? t.shell.runFailed(last.finishedAt) : t.shell.last(last.finishedAt);
  });
  const setup = $derived(navigation.current === 'jobs' && shell.firstRun);
  // A click opens the run card: without a run to open the status would be a dead button.
  // The run card says the same while it is on screen.
  // The status that arrives with the first data is simply there (no fade at start).
  const motion = settled();
  const statusShown = $derived(
    (run.active || last !== null) &&
      !(navigation.current === 'jobs' && !shell.firstRun && shell.runCard),
  );

  const active = $derived.by((): NavId => {
    if (navigation.current !== 'jobs') return navigation.current;
    if (jobs.facet === 'archived') return 'archive';
    return jobs.facet === 'trash' ? 'trash' : 'jobs';
  });

  /** The arrow on Jobs: Archiv and Papierkorb hide and show (they stay while one is open). */
  const fold = $derived<SideNavFold>({
    open: navigation.placesShown,
    hide: t.nav.hidePlaces,
    show: t.nav.showPlaces,
    locked: active === 'trash' ? t.nav.placesStay.trash : t.nav.placesStay.archive,
    testid: 'places-toggle',
    ontoggle: () => navigation.togglePlaces(),
  });

  /**
   * The view first: an unsaved Profil may keep it and ask. The place changes only with the
   * switch (at once, or once the question is answered), and a click on the place that is
   * already open changes nothing (no reload, like Jobs).
   */
  function choose(id: NavId): void {
    const from = navigation.current;
    const view: ViewId = id === 'archive' || id === 'trash' ? 'jobs' : id;
    navigation.go(view, false, () => arrive(id, from));
  }

  function arrive(id: NavId, from: ViewId): void {
    if (id === 'archive' || id === 'trash') {
      const facet = id === 'archive' ? 'archived' : 'trash';
      if (jobs.facet !== facet) jobs.setFacet(facet);
      return;
    }
    if (id !== 'jobs') return;
    // Jobs from the archive or the trash: back to the inbox, on its last tab.
    if (jobs.facet === 'archived' || jobs.facet === 'trash') jobs.setFacet(jobs.inboxFacet);
    // Back from another view: Neu is entered again (the jobs read meanwhile leave it).
    else if (from !== 'jobs' && jobs.facet === 'new') void jobs.load(true);
  }

  function openRun(): void {
    navigation.go('jobs');
    run.panel = 'open';
    // In one column an open job hides the list and its run card: back to the list.
    if (viewport.narrow) jobs.clearSelection();
  }

  // Ctrl+B (Cmd+B on macOS) and the macOS menu fold and unfold the sidebar.
</script>

<aside class="sidebar" class:rail={viewport.rail} id={SIDEBAR_ID} data-testid="sidebar">
  {#if dragBands()}<span class="lights"><DragBand /></span>{/if}
  <!-- Until the state is known nothing is guessed (like the views): the entries come with it,
       as they are, instead of changing their colours in front of the user. Before the setup
       there is nowhere to go yet: the views wait (inert, faded). -->
  {#if app.state !== null}
    <div class="nav" class:waiting={setup} inert={setup}>
      <!-- The setup page is no view of the list: nothing is marked current while it shows. -->
      <SideNav
        {items}
        active={setup ? null : active}
        label={t.nav.label}
        collapsed={viewport.rail}
        {fold}
        onselect={choose}
      />
    </div>
  {/if}

  {#if statusShown}
    <div class="status" transition:fade={{ on: motion.ready }}>
      <StatusLine
        text={status}
        label={t.shell.showRun}
        icon={failed ? 'triangle-alert' : 'clock'}
        tone={failed ? 'danger' : 'neutral'}
        busy={run.active}
        progress={run.active && !viewport.rail ? run.fraction : undefined}
        progressLabel={t.toolbar.progress}
        collapsed={viewport.rail}
        testid="run-status"
        onclick={openRun}
      />
    </div>
  {/if}
</aside>

<style>
  .sidebar {
    position: relative;
    display: flex;
    flex: none;
    flex-direction: column;
    width: var(--sidebar-width);
    height: 100%;
    padding: 0 var(--space-12) var(--space-12);
  }

  .rail {
    align-items: center;
    width: var(--rail-width);
  }

  /* The traffic lights' band spans the whole width of the sidebar. */
  .lights {
    display: flex;
    flex-direction: column;
    align-self: stretch;
    margin: 0 calc(-1 * var(--space-12));
  }

  /* The first view starts on the line of the list header's search field. */
  .nav {
    margin-top: calc(var(--pane-padding) + var(--border-width));
  }

  .waiting {
    opacity: var(--opacity-disabled);
  }

  .status {
    display: flex;
    justify-content: center;
    width: 100%;
    margin-top: auto;
  }
</style>
