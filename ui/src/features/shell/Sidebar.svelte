<!--
  The sidebar (full window height, 232 px; the 64 px icon rail below 1100 px): the app mark
  and name (on macOS below the traffic lights), "Abrufen" as the window's primary (during a
  run "Abbrechen" with a mini progress), the views with icons and the unread count, and at
  the bottom the compact run status that opens the run in the Jobs view.
  Per-OS markup lives only in the shell (this file and TitleBar) and WindowControls.
-->
<script lang="ts">
  import BrandMark from '$components/BrandMark.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Icon from '$components/Icon.svelte';
  import Meter from '$components/Meter.svelte';
  import SideNav, { type SideNavItem } from '$components/SideNav.svelte';
  import Spinner from '$components/Spinner.svelte';
  import { tooltip } from '$lib/actions/tooltip';
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
  // The first-run page and the settings form carry their own primary; the sidebar then steps back.
  const primary = $derived(app.hasMailbox && !shell.firstRun);

  function openRun(): void {
    navigation.go('jobs');
    run.panel = 'open';
  }
</script>

<aside class="sidebar {os}" class:rail={viewport.rail} data-testid="sidebar" data-tauri-drag-region>
  <div class="brand" data-testid="brand" data-tauri-drag-region>
    <BrandMark size="md" />
    {#if !viewport.rail}<span class="name" in:fade>{de.app.name}</span>{/if}
  </div>

  <div class="fetch">
    {#if run.active}
      <Button
        variant="secondary"
        wide
        icon="square"
        iconOnly={viewport.rail}
        label={de.toolbar.cancel}
        loading={run.cancelling}
        testid="cancel-run"
        onclick={() => void run.cancel()}
      />
      <Meter value={run.fraction} size="sm" label={de.toolbar.progress} testid="mini-progress" />
    {:else}
      <Button
        variant={primary ? 'primary' : 'secondary'}
        wide
        icon="refresh-cw"
        iconOnly={viewport.rail}
        label={de.toolbar.fetch}
        disabled={!app.hasMailbox}
        disabledReason={de.toolbar.needsMailbox}
        testid="fetch"
        onclick={() => void run.start({ kind: 'fetch' })}
      />
    {/if}
  </div>

  <SideNav
    {items}
    active={navigation.current}
    label={de.nav.label}
    collapsed={viewport.rail}
    onselect={(id) => navigation.go(id)}
  />

  <div class="status" use:tooltip={viewport.rail ? status : null}>
    <Card
      variant="interactive"
      padding="none"
      label={de.shell.showRun}
      onclick={openRun}
      testid="run-status"
    >
      <span class="status-body" class:failed>
        {#if run.active}
          <span class="line">
            <Spinner size="sm" label={null} />
            {#if !viewport.rail}<span class="text">{status}</span>{/if}
          </span>
          {#if !viewport.rail}
            <Meter value={run.fraction} size="sm" label={de.toolbar.progress} />
          {/if}
        {:else}
          <span class="line">
            <Icon name={failed ? 'triangle-alert' : 'clock'} size="sm" />
            {#if !viewport.rail}<span class="text">{status}</span>{/if}
          </span>
        {/if}
      </span>
    </Card>
  </div>
</aside>

<style>
  .sidebar {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: var(--space-16);
    width: var(--sidebar-width);
    height: 100%;
    padding: 0 var(--space-12) var(--space-12);
    border-right: var(--border-width) solid var(--border);
    background-color: var(--surface-sidebar);
  }

  .rail {
    align-items: center;
    width: var(--rail-width);
    padding: 0 var(--space-8) var(--space-12);
  }

  .brand {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-8);
    height: var(--titlebar-height);
    margin-bottom: calc(var(--space-8) - var(--space-16));
    padding: 0 var(--space-4);
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
    font: var(--type-md);
    font-weight: var(--weight-semibold);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .fetch {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    width: 100%;
  }

  .rail .fetch {
    align-items: center;
  }

  .status {
    width: 100%;
    margin-top: auto;
  }

  .status-body {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    padding: var(--space-8) var(--space-12);
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .rail .status-body {
    align-items: center;
    padding: var(--space-8) 0;
  }

  .status-body.failed {
    color: var(--danger-strong);
  }

  .line {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
