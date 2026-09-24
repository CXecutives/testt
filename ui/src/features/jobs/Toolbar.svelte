<!--
  The 56 px toolbar of the Jobs view: only the filters (Neu | Alle, Beste Passung | Neueste
  when there is a profile) and the search. "Abrufen" lives in the sidebar.
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

<div class="toolbar" data-testid="toolbar">
  <Segmented
    options={facets}
    value={jobs.facet}
    label={de.toolbar.facet}
    testid="facet"
    onchange={(id) => jobs.setFacet(id)}
  />
  {#if app.hasProfile}
    <Segmented
      options={sorts}
      value={jobs.sortChoice}
      label={de.toolbar.sort}
      testid="sort"
      onchange={(id) => jobs.setSort(id)}
    />
  {/if}
  <div class="search">
    <TextField
      kind="search"
      value={jobs.search}
      label={de.toolbar.searchLabel}
      placeholder={de.toolbar.search}
      testid="search"
      oninput={(value) => jobs.setSearch(value)}
    />
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-12) var(--space-16);
    min-height: var(--toolbar-height);
    padding: var(--space-8) var(--space-24);
    border-bottom: var(--border-width) solid var(--border);
  }

  .search {
    flex: 1;
    min-width: var(--stat-min);
    max-width: var(--list-min);
    margin-left: auto;
  }

  @media (width < 900px) {
    .toolbar {
      padding: var(--space-8) var(--space-16);
    }
  }
</style>
