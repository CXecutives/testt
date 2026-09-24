<!-- Placeholder until phase 3 builds the screen. -->
<script lang="ts">
  import EmptyState from '$components/EmptyState.svelte';
  import { de } from '$lib/i18n/de';
  import { invoke, reportUiError } from '$lib/ipc/api';

  function pick(): void {
    invoke('pick_profile').catch((error: unknown) => reportUiError(String(error), null, null));
  }
</script>

<div class="page">
  <EmptyState
    icon="file-text"
    text={de.views.profile.text}
    action={{ label: de.views.profile.action, icon: 'file-up', onclick: pick }}
    testid="placeholder-profile"
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
