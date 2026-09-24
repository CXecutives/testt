<!-- Placeholder until phase 3 builds the screen. -->
<script lang="ts">
  import EmptyState from '$components/EmptyState.svelte';
  import { de } from '$lib/i18n/de';
</script>

<div class="page">
  <EmptyState icon="sparkles" heading={de.views.jobs.heading} text={de.views.jobs.text} />
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
