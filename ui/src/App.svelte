<!--
  The shell: title bar and the three views. A view switch fades the old view out quickly
  and lets the new one rise 8 px into place; both share one grid cell, so nothing jumps.
  On start the app shows useful content at once: the first-run page while no mailbox is
  connected (or nothing was ever fetched), otherwise the Jobs view with the last results.
-->
<script lang="ts">
  import EmptyState from '$components/EmptyState.svelte';
  import Spinner from '$components/Spinner.svelte';
  import Tooltip from '$components/Tooltip.svelte';
  import { de } from '$lib/i18n/de';
  import { viewIn, viewOut } from '$lib/motion/transitions';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import FirstRunView from './features/first-run/FirstRunView.svelte';
  import JobsView from './features/jobs/JobsView.svelte';
  import ProfileView from './features/profile/ProfileView.svelte';
  import SettingsView from './features/settings/SettingsView.svelte';
  import TitleBar from './features/shell/TitleBar.svelte';

  run.install();
  jobs.install();
  void app.load().then((state) => run.attach(state?.running ?? null));

  const firstRun = $derived(
    app.state !== null &&
      (!app.hasMailbox || (app.state.firstRun && !run.active && run.summary === null)),
  );
</script>

<div class="shell" data-testid="shell">
  <TitleBar />
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
        <section class="view" data-testid="view-first-run" in:viewIn out:viewOut>
          <FirstRunView />
        </section>
      {:else}
        <section class="view fixed" data-testid="view-jobs" in:viewIn out:viewOut>
          <JobsView />
        </section>
      {/if}
    {:else if navigation.current === 'profile'}
      <section class="view" data-testid="view-profile" in:viewIn out:viewOut>
        <ProfileView />
      </section>
    {:else}
      <section class="view" data-testid="view-settings" in:viewIn out:viewOut>
        <SettingsView />
      </section>
    {/if}
  </main>
  <Tooltip />
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    background-color: var(--bg);
  }

  .views {
    display: grid;
    flex: 1;
    min-height: 0;
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
