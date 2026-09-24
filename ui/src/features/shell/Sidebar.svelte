<!--
  The calm sidebar (196 px, icons only below 1100 px) on the cream: no surface of its own,
  the white sheet of the content is the divider. App icon and name live in the native title
  bar of the OS, so the sidebar starts with the views (on macOS below the traffic lights,
  whose 52 px band moves the window): the first sits on the line of the list's search field
  on Windows, each with its icon and the unread count, and
  at the foot a quiet run status that opens the run in the Jobs view. It shows only while
  there is a run to open (before the first fetch the first-run page says it all), and it is
  said once: while the run card is on screen it steps aside. "Abrufen" lives in the list
  header.
-->
<script lang="ts">
  import DragBand from '$components/DragBand.svelte';
  import SideNav, { type SideNavItem } from '$components/SideNav.svelte';
  import StatusLine from '$components/StatusLine.svelte';
  import { t } from '$lib/i18n/t';
  import { settled } from '$lib/motion/settled.svelte';
  import { fade } from '$lib/motion/transitions';
  import { dragBands } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { navigation, type ViewId } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import { shell } from '$lib/state/shell.svelte';
  import { viewport } from '$lib/state/viewport.svelte';

  // New jobs over everything (the overview's unfiltered counts), whatever the list shows.
  const unread = $derived(jobs.overviewCounts?.new ?? app.state?.counts.new ?? 0);
  const items = $derived<SideNavItem<ViewId>[]>([
    { id: 'jobs', label: t.nav.jobs, icon: 'briefcase', count: unread, testid: 'nav-jobs' },
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
    if (failed) return t.shell.runFailed;
    return last ? t.shell.last(last.finishedAt) : t.run.never;
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

  function openRun(): void {
    navigation.go('jobs');
    run.panel = 'open';
    // In one column an open job hides the list and its run card: back to the list.
    if (viewport.narrow) jobs.clearSelection();
  }
</script>

<aside class="sidebar" class:rail={viewport.rail} data-testid="sidebar">
  {#if dragBands()}<span class="lights"><DragBand /></span>{/if}
  <!-- Before the setup there is nowhere to go yet: the views wait (inert, faded). -->
  <div class="nav" class:waiting={setup} inert={setup}>
    <SideNav
      {items}
      active={navigation.current}
      label={t.nav.label}
      collapsed={viewport.rail}
      onselect={(id) => navigation.go(id)}
    />
  </div>

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
