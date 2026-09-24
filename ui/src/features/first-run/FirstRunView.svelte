<!-- Placeholder until phase 3 builds the screen. -->
<script lang="ts">
  import EmptyState from '$components/EmptyState.svelte';
  import { de } from '$lib/i18n/de';
</script>

<div class="page">
  <EmptyState icon="check" heading={de.views.firstRun.heading} text={de.views.firstRun.text} />
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
