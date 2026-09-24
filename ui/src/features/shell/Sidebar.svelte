<!--
  The calm sidebar (196 px, icons only below 1100 px): no surface of its own, only a hairline
  to the content. On top the app mark and name (on macOS below the traffic lights), then the
  views with icons and the unread count, and at the foot a quiet run status that opens the
  run in the Jobs view (during a run the step and a slim meter). "Abrufen" lives in the top
  strip of the content (TitleBar).
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

  function openRun(): void {
    navigation.go('jobs');
    run.panel = 'open';
  }
</script>

<aside class="sidebar {os}" class:rail={viewport.rail} data-testid="sidebar" data-tauri-drag-region>
  <div class="brand" data-testid="brand" data-tauri-drag-region>
    <BrandMark size="sm" />
    {#if !viewport.rail}<span class="name" in:fade>{de.app.name}</span>{/if}
  </div>

  <SideNav
    {items}
    active={navigation.current}
    label={de.nav.label}
    collapsed={viewport.rail}
    onselect={(id) => navigation.go(id)}
  />

  <div class="status">
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
</aside>

<style>
  .sidebar {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: var(--space-12);
    width: var(--sidebar-width);
    height: 100%;
    padding: 0 var(--space-12) var(--space-12);
    border-right: var(--border-width) solid var(--border);
  }

  .rail {
    align-items: center;
    width: var(--rail-width);
    padding: 0 var(--space-12) var(--space-12);
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

  /* macOS: the traffic lights sit at the top left; the brand goes below them, and the rail
     is as wide as the lights. */
  .macos .brand {
    margin-top: var(--titlebar-height);
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

  .status {
    display: flex;
    justify-content: center;
    width: 100%;
    margin-top: auto;
  }
</style>
