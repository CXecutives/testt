<!--
  The header of the list column: the search and next to it "Abrufen", the one primary of
  the Jobs view, which fills this list ("Abbrechen" in its place while a run goes; without
  a mailbox it stays locked and says why). On macOS this first row is the list's part of the
  toolbar row, centred on the traffic lights, and its empty parts move the window. Below, Neu | Alle on the left and (with a
  profile) the sort as one quiet icon button on the right. Its tooltip says the current
  order; a click switches between best match and newest first.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Segmented from '$components/Segmented.svelte';
  import TextField from '$components/TextField.svelte';
  import { de } from '$lib/i18n/de';
  import type { JobFacet } from '$lib/ipc/types';
  import { dragBands } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { run } from '$lib/state/run.svelte';

  const facets = $derived([
    { id: 'new' as JobFacet, label: de.toolbar.facetNew, count: jobs.counts.new },
    { id: 'all' as JobFacet, label: de.toolbar.facetAll, count: jobs.counts.all },
  ]);
</script>

<div class="header" data-testid="list-header">
  <div class="top" data-tauri-drag-region={dragBands() ? '' : undefined}>
    <span class="search">
      <TextField
        kind="search"
        value={jobs.search}
        label={de.toolbar.searchLabel}
        placeholder={de.toolbar.search}
        testid="search"
        oninput={(value) => jobs.setSearch(value)}
      />
    </span>
    {#if run.active}
      <Button
        variant="secondary"
        icon="circle-stop"
        label={de.toolbar.cancel}
        loading={run.cancelling}
        testid="cancel-run"
        onclick={() => void run.cancel()}
      />
    {:else}
      <Button
        variant={app.hasMailbox ? 'primary' : 'secondary'}
        icon="refresh-cw"
        label={de.toolbar.fetch}
        disabled={!app.hasMailbox}
        disabledReason={de.toolbar.needsMailbox}
        testid="fetch"
        onclick={() => void run.start({ kind: 'fetch' })}
      />
    {/if}
  </div>
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
    padding: var(--list-header-top) var(--pane-padding) var(--pane-padding);
    border-bottom: var(--border-width) solid var(--border);
  }

  /* The row spans the header's side padding too, so on macOS its empty ends move the window
     like the rest of the toolbar row. */
  .top {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-height: var(--list-toolbar);
    margin: 0 calc(-1 * var(--pane-padding));
    padding: 0 var(--pane-padding);
  }

  .search {
    display: flex;
    flex: 1;
    min-width: 0;
  }

  .filters {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-8);
  }
</style>
