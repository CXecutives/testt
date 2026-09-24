<!--
  The header of the list column: the search over the full width, below it the two filter
  groups, Neu | Alle on the left and (with a profile) Beste Passung | Neueste on the right.
  In a narrow column the two groups stack and each takes the full width.
-->
<script lang="ts">
  import Segmented from '$components/Segmented.svelte';
  import TextField from '$components/TextField.svelte';
  import { de } from '$lib/i18n/de';
  import type { JobFacet, JobSort } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';

  const facets = $derived([
    { id: 'new' as JobFacet, label: de.toolbar.facetNew, count: jobs.counts.new },
    { id: 'all' as JobFacet, label: de.toolbar.facetAll, count: jobs.counts.all },
  ]);
  const sorts: { id: JobSort; label: string }[] = [
    { id: 'match', label: de.toolbar.sortMatch },
    { id: 'newest', label: de.toolbar.sortNewest },
  ];
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
  <div class="filters" class:single={!app.hasProfile}>
    <Segmented
      options={facets}
      value={jobs.facet}
      label={de.toolbar.facet}
      size="sm"
      testid="facet"
      onchange={(id) => jobs.setFacet(id)}
    />
    {#if app.hasProfile}
      <Segmented
        options={sorts}
        value={jobs.sortChoice}
        label={de.toolbar.sort}
        size="sm"
        testid="sort"
        onchange={(id) => jobs.setSort(id)}
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
    display: grid;
    grid-template-columns: auto auto;
    justify-content: space-between;
    gap: var(--space-8);
  }

  .filters.single {
    grid-template-columns: auto;
    justify-content: start;
  }

  /* Too narrow for both groups side by side: one group per line, each over the full width. */
  @container (width < 392px) {
    .filters,
    .filters.single {
      grid-template-columns: 1fr;
      justify-content: stretch;
    }
  }
</style>
