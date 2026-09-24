<!-- Placeholder until phase 3 builds the first run (a soft coral hero, no second hue). -->
<script lang="ts">
  import EmptyState from '$components/EmptyState.svelte';
  import { de } from '$lib/i18n/de';
  import { navigation } from '$lib/state/navigation.svelte';
</script>

<div class="hero">
  <EmptyState
    heading={de.views.firstRun.heading}
    text={de.views.firstRun.text}
    action={{ label: de.views.firstRun.action, onclick: () => navigation.go('settings') }}
  />
</div>

<style>
  .hero {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: var(--space-32);
    background: var(--grad-hero);
  }
</style>
