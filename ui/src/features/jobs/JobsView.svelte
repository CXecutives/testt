<!-- Placeholder until phase 3 builds the screen. -->
<script lang="ts">
  import EmptyState from '$components/EmptyState.svelte';
  import { de } from '$lib/i18n/de';
  import { navigation } from '$lib/state/navigation.svelte';
</script>

<div class="page">
  <EmptyState
    text={de.views.jobs.text}
    action={{ label: de.views.jobs.action, icon: 'mail', onclick: () => navigation.go('settings') }}
    testid="placeholder-jobs"
  />
</div>

<style>
  .page {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: var(--space-32);
  }
</style>
