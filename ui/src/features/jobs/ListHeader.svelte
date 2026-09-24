<!--
  The header of the list column: the search over the full width, below it Neu | Alle on the
  left and (with a profile) the sort as one quiet icon button on the right. Its tooltip says
  the current order; a click switches between best match and newest first.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Segmented from '$components/Segmented.svelte';
  import TextField from '$components/TextField.svelte';
  import { de } from '$lib/i18n/de';
  import type { JobFacet } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';

  const facets = $derived([
    { id: 'new' as JobFacet, label: de.toolbar.facetNew, count: jobs.counts.new },
    { id: 'all' as JobFacet, label: de.toolbar.facetAll, count: jobs.counts.all },
  ]);
</script>

<div class="header" data-testid="list-header">
  <TextField
    kind="search"
    value={jobs.search}
    label={de.toolbar.searchLabel}
    placeholder={de.toolbar.search}
    testid="search"
    oninput={(value) => jobs.setSearch(value)}
  />
  <div class="filters">
    <Segmented
      options={facets}
      value={jobs.facet}
      label={de.toolbar.facet}
      size="sm"
      testid="facet"
      onchange={(id) => jobs.setFacet(id)}
    />
    {#if app.hasProfile}
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon="arrow-up-down"
        label={de.toolbar.sortedBy[jobs.sortChoice]}
        testid="sort"
        onclick={() => jobs.setSort(jobs.sortChoice === 'match' ? 'newest' : 'match')}
      />
    {/if}
  </div>
</div>

<style>
  .header {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: var(--space-12);
    padding: var(--pane-padding);
    border-bottom: var(--border-width) solid var(--border);
  }

  .filters {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-8);
  }
</style>
