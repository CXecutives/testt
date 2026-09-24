<!--
  The shell: title bar and the three views. A view switch fades the old view out quickly
  and lets the new one rise 8 px into place; both share one grid cell, so nothing jumps.
-->
<script lang="ts">
  import Tooltip from '$components/Tooltip.svelte';
  import { viewIn, viewOut } from '$lib/motion/transitions';
  import { navigation } from '$lib/state/navigation.svelte';
  import JobsView from './features/jobs/JobsView.svelte';
  import ProfileView from './features/profile/ProfileView.svelte';
  import SettingsView from './features/settings/SettingsView.svelte';
  import TitleBar from './features/shell/TitleBar.svelte';
</script>

<div class="shell" data-testid="shell">
  <TitleBar />
  <main class="views">
    {#if navigation.current === 'jobs'}
      <section class="view" data-testid="view-jobs" in:viewIn out:viewOut>
        <JobsView />
      </section>
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
</style>
