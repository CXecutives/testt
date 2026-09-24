<!--
  The calm sidebar (196 px, icons only below 1100 px) on the cream: no surface of its own,
  the white sheet of the content is the divider. On Windows the app mark and name share the
  height of the top strip; on macOS that band stays empty for the traffic lights (the Dock
  names the app). The first view sits on the line of the list's search field, then the views
  with icons and the unread count, and at the foot a quiet run status that opens the run in
  the Jobs view. The status is said once: while the run card is on screen it steps aside.
  "Abrufen" lives in the top strip of the content (TitleBar).
  Per-OS markup lives only in the shell (this file and TitleBar) and WindowControls.
-->
<script lang="ts">
  import BrandMark from '$components/BrandMark.svelte';
  import SideNav, { type SideNavItem } from '$components/SideNav.svelte';
  import StatusLine from '$components/StatusLine.svelte';
  import { de } from '$lib/i18n/de';
  import { fade } from '$lib/motion/transitions';
  import { platform } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { navigation, type ViewId } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import { shell } from '$lib/state/shell.svelte';
  import { viewport } from '$lib/state/viewport.svelte';

  const os = platform();

  const unread = $derived(
    jobs.status === 'ready' && jobs.search.trim() === '' && jobs.filter === null
      ? jobs.counts.new
      : (app.state?.counts.new ?? 0),
  );
  const items = $derived<SideNavItem<ViewId>[]>([
    { id: 'jobs', label: de.nav.jobs, icon: 'briefcase', count: unread, testid: 'nav-jobs' },
    { id: 'profile', label: de.nav.profile, icon: 'user-round', testid: 'nav-profile' },
    { id: 'settings', label: de.nav.settings, icon: 'sliders-horizontal', testid: 'nav-settings' },
  ]);
  const last = $derived(app.state?.lastRun ?? null);
  const failed = $derived(!run.active && (run.summary ?? last)?.outcome.kind === 'failed');
  const status = $derived.by(() => {
    if (run.active) {
      if (run.status) return de.run.status[run.status.code];
      return run.step ? de.run.step[run.step] : de.run.kind[run.kind ?? 'fetch'];
    }
    if (failed) return de.shell.runFailed;
    const finished = run.summary?.finishedAt ?? last?.finishedAt ?? null;
    return finished ? de.shell.last(finished) : de.run.never;
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

<aside class="sidebar {os}" class:rail={viewport.rail} data-testid="sidebar" data-tauri-drag-region>
  {#if os === 'windows'}
    <div class="brand" data-testid="brand" data-tauri-drag-region>
      <BrandMark size="sm" />
      {#if !viewport.rail}<span class="name" in:fade>{de.app.name}</span>{/if}
    </div>
  {/if}

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

  /* The brand row shares the height of the content's top strip, so both read as one band. */
  .brand {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-8);
    height: var(--titlebar-height);
    /* The mark's centre sits on the axis of the nav icons below. */
    padding-left: calc(var(--space-12) - var(--space-2));
  }

  .rail .brand {
    padding: 0;
  }

  /* macOS: the band of the top strip belongs to the traffic lights; the rail is as wide as
     the lights. */
  .macos {
    padding-top: var(--titlebar-height);
  }

  .macos.rail {
    width: var(--traffic-light-inset);
  }

  .brand > * {
    pointer-events: none;
  }

  .name {
    overflow: hidden;
    color: var(--text-heading);
    font: var(--type-tab);
    font-weight: var(--weight-semibold);
    text-overflow: ellipsis;
    white-space: nowrap;
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
