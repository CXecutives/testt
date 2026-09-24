<!--
  The shell below the native title bar of the OS: the sidebar and the white sheet with the
  three views. A view switch is quick: the old view leaves at once and the new one fades in
  rising 4 px (150 ms). On start the app shows useful content at once: the first-run page
  while nothing was ever fetched, otherwise the Jobs view with the last results. Closing
  while a fetch runs keeps the window until the run has stopped; a calm note says so.
-->
<script lang="ts">
  import DragBand from '$components/DragBand.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import Spinner from '$components/Spinner.svelte';
  import Toast from '$components/Toast.svelte';
  import Tooltip from '$components/Tooltip.svelte';
  import { de } from '$lib/i18n/de';
  import { onClosing } from '$lib/ipc/api';
  import { fade, viewIn } from '$lib/motion/transitions';
  import { dragBands } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import { shell } from '$lib/state/shell.svelte';
  import FirstRunView from './features/first-run/FirstRunView.svelte';
  import JobsView from './features/jobs/JobsView.svelte';
  import ProfileView from './features/profile/ProfileView.svelte';
  import SettingsView from './features/settings/SettingsView.svelte';
  import Sidebar from './features/shell/Sidebar.svelte';

  run.install();
  jobs.install();
  navigation.install();
  void app.load().then((state) => run.attach(state?.running ?? null));

  const firstRun = $derived(shell.firstRun);
  /** macOS: the views keep the toolbar row free (the Jobs view uses it for its list row). */
  const band = dragBands();
  let closing = $state(false);
  $effect(() => onClosing(() => (closing = true)));
</script>

<div class="shell" data-testid="shell">
  <div class="body">
    <Sidebar />
    <main class="views">
      {#if app.error !== null && app.state === null}
        <section class="view center" data-testid="view-error">
          <EmptyState
            icon="triangle-alert"
            tone="danger"
            text={de.shell.loadFailed}
            action={{ label: de.common.retry, icon: 'rotate-ccw', onclick: () => void app.load() }}
          />
        </section>
      {:else if app.state === null}
        <!-- Until the state is known nothing is guessed (no jobs view flashing before the first run). -->
        <section class="view center" data-testid="view-loading">
          {#if app.slow}<Spinner size="lg" />{/if}
        </section>
      {:else if navigation.current === 'jobs'}
        {#if firstRun}
          <section class="view" data-testid="view-first-run" in:viewIn>
            {#if band}<DragBand sheet />{/if}
            <FirstRunView />
          </section>
        {:else}
          <section class="view fixed" data-testid="view-jobs" in:viewIn>
            <JobsView />
          </section>
        {/if}
      {:else if navigation.current === 'profile'}
        <section class="view" data-testid="view-profile" in:viewIn>
          {#if band}<DragBand sheet />{/if}
          <ProfileView />
        </section>
      {:else}
        <section class="view" data-testid="view-settings" in:viewIn>
          {#if band}<DragBand sheet />{/if}
          <SettingsView />
        </section>
      {/if}
    </main>
    {#if closing}
      <div class="closing" data-testid="closing" role="status" transition:fade>
        <p class="closing-note"><Spinner size="sm" label={null} />{de.shell.closing}</p>
      </div>
    {/if}
  </div>
  <Toast />
  <Tooltip />
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    background-color: var(--bg);
  }

  .body {
    position: relative;
    display: flex;
    flex: 1;
    min-height: 0;
  }

  /* Closing during a fetch: the window waits until the run has stopped. */
  .closing {
    position: absolute;
    z-index: var(--z-overlay);
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: var(--scrim);
  }

  .closing-note {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-16) var(--space-20);
    border-radius: var(--radius-dialog);
    background-color: var(--surface);
    box-shadow: var(--sh-pop);
    color: var(--text);
    font: var(--type-md);
  }

  /* One white sheet for every view: the sidebar stays on the cream, the sheet's hairline and
     rounded corner are the only divider between them. */
  .views {
    display: grid;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border-top: var(--sheet-top-edge) solid var(--border);
    border-left: var(--border-width) solid var(--border);
    border-top-left-radius: var(--sheet-corner);
    background-color: var(--surface);
  }

  .view {
    grid-area: 1 / 1;
    min-width: 0;
    min-height: 0;
    overflow: auto;
  }

  .fixed {
    overflow: hidden;
  }

  .center {
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
