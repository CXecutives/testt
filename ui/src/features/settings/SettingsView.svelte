<!-- Placeholder until phase 3 builds the screen. -->
<script lang="ts">
  import EmptyState from '$components/EmptyState.svelte';
  import { de } from '$lib/i18n/de';
  import { navigation } from '$lib/state/navigation.svelte';
</script>

<div class="page">
  <EmptyState
    icon="mail"
    text={de.views.settings.text}
    action={{ label: de.views.settings.action, onclick: () => navigation.go('jobs') }}
    testid="placeholder-settings"
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
