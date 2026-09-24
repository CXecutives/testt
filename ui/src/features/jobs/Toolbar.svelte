<!--
  The 64 px toolbar of the Jobs view: "Abrufen" (the one primary of the view; during a run
  "Abbrechen" with a mini progress), Neu | Alle, Beste Passung | Neueste (only with a
  profile) and the search.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Meter from '$components/Meter.svelte';
  import Segmented from '$components/Segmented.svelte';
  import TextField from '$components/TextField.svelte';
  import { de } from '$lib/i18n/de';
  import type { JobFacet, JobSort } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { run } from '$lib/state/run.svelte';

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
  <div class="run">
    {#if run.active}
      <Button
        variant="secondary"
        size="lg"
        icon="square"
        label={de.toolbar.cancel}
        loading={run.cancelling}
        testid="cancel-run"
        onclick={() => void run.cancel()}
      />
      <span class="mini">
        <Meter value={run.fraction} size="sm" label={de.toolbar.progress} testid="mini-progress" />
      </span>
    {:else}
      <Button
        variant="primary"
        size="lg"
        icon="refresh-cw"
        label={de.toolbar.fetch}
        disabled={!app.hasMailbox}
        disabledReason={de.toolbar.needsMailbox}
        testid="fetch"
        onclick={() => void run.start({ kind: 'fetch' })}
      />
    {/if}
  </div>
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
    align-items: center;
    gap: var(--space-16);
    min-height: var(--toolbar-height);
    padding: var(--space-8) var(--space-24);
    border-bottom: var(--border-width) solid var(--border);
    background-color: var(--surface);
  }

  .run {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-12);
  }

  .mini {
    width: var(--space-64);
  }

  .search {
    flex: 1;
    min-width: var(--stat-min);
    max-width: var(--list-min);
    margin-left: auto;
  }

  @media (width < 900px) {
    .toolbar {
      flex-wrap: wrap;
      padding: var(--space-12) var(--space-16);
    }

    .search {
      flex-basis: 100%;
      max-width: none;
    }
  }
</style>
