<!--
  The calm sidebar (196 px, icons only below 1100 px) on the cream: no surface of its own,
  the white sheet of the content is the divider. App icon and name live in the title bar
  (Windows) or the native title bar (macOS), so the sidebar starts with the views: the first
  sits on the line of the list's search field, each with its icon and the unread count, and
  at the foot a quiet run status that opens the run in the Jobs view. The status is said
  once: while the run card is on screen it steps aside. "Abrufen" lives in the list header.
-->
<script lang="ts">
  import SideNav, { type SideNavItem } from '$components/SideNav.svelte';
  import StatusLine from '$components/StatusLine.svelte';
  import { de } from '$lib/i18n/de';
  import { fade } from '$lib/motion/transitions';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { navigation, type ViewId } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import { shell } from '$lib/state/shell.svelte';
  import { viewport } from '$lib/state/viewport.svelte';

  // New jobs over everything (the overview's unfiltered counts), whatever the list shows.
  const unread = $derived(jobs.overviewCounts?.new ?? app.state?.counts.new ?? 0);
  const items = $derived<SideNavItem<ViewId>[]>([
    { id: 'jobs', label: de.nav.jobs, icon: 'briefcase', count: unread, testid: 'nav-jobs' },
    { id: 'profile', label: de.nav.profile, icon: 'user-round', testid: 'nav-profile' },
    { id: 'settings', label: de.nav.settings, icon: 'sliders-horizontal', testid: 'nav-settings' },
  ]);
  // The last fetch: a rescore of this session is no fetch.
  const fetched = $derived(run.summary?.kind === 'rescore' ? null : run.summary);
  const last = $derived(fetched ?? app.state?.lastRun ?? null);
  const failed = $derived(!run.active && last?.outcome.kind === 'failed');
  const status = $derived.by(() => {
    if (run.active) {
      if (run.status) return de.run.statusOf(run.status.code, run.status.portal);
      return run.step ? de.run.step[run.step] : de.run.kind[run.kind ?? 'fetch'];
    }
    if (failed) return de.shell.runFailed;
    return last ? de.shell.last(last.finishedAt) : de.run.never;
  });
  const setup = $derived(navigation.current === 'jobs' && shell.firstRun);
  // The run card says the same while it is on screen.
  const statusShown = $derived(
    !(navigation.current === 'jobs' && !shell.firstRun && shell.runCard),
  );

  function openRun(): void {
    navigation.go('jobs');
    run.panel = 'open';
  }
</script>

<aside class="sidebar" class:rail={viewport.rail} data-testid="sidebar">
  <!-- Before the setup there is nowhere to go yet: the views wait (inert, faded). -->
  <div class="nav" class:waiting={setup} inert={setup}>
    <SideNav
      {items}
      active={navigation.current}
      label={de.nav.label}
      collapsed={viewport.rail}
      onselect={(id) => navigation.go(id)}
    />
  </div>

  {#if statusShown}
    <div class="status" transition:fade>
      <StatusLine
        text={status}
        label={de.shell.showRun}
        icon={failed ? 'triangle-alert' : 'clock'}
        tone={failed ? 'danger' : 'neutral'}
        busy={run.active}
        progress={run.active && !viewport.rail ? run.fraction : undefined}
        progressLabel={de.toolbar.progress}
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
