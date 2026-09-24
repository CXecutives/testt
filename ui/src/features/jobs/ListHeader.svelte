<!--
  The header of the list column, every list control in one place.
  Row 1: the search and next to it "Abrufen", the one primary of the Jobs view, which fills
  this list ("Abbrechen" in its place while a fetch or details run goes; locked while the
  app scores the jobs anew, and without a mailbox, saying why). The action slot is as wide as the wider of the two and both fill it, so the
  search never jumps when a run starts; the one that comes fades in, the one that goes is
  gone at once.
  On macOS this row is the list's part of the toolbar row, centred on the traffic lights,
  and its empty parts move the window.
  Row 2: Neu | Alle, then the filter a tile or a portal chip set (a soft navy pill that
  pops in and fades out, with its x), and at the right edge (with a profile) the sort as
  one quiet icon button whose glyph stands half a turn for "newest first".
  The bottom hairline shows only once the list below is scrolled.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Segmented from '$components/Segmented.svelte';
  import TextField from '$components/TextField.svelte';
  import { de } from '$lib/i18n/de';
  import type { JobFacet } from '$lib/ipc/types';
  import { fade, pop } from '$lib/motion/transitions';
  import { dragBands } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { run } from '$lib/state/run.svelte';

  interface Props {
    /** The list below is scrolled away from its top. */
    scrolled?: boolean;
  }
  let { scrolled = false }: Props = $props();

  const facets = $derived([
    { id: 'new' as JobFacet, label: de.toolbar.facetNew, count: jobs.counts.new },
    { id: 'all' as JobFacet, label: de.toolbar.facetAll, count: jobs.counts.all },
  ]);
</script>

{#snippet fetchButton(live: boolean)}
  <Button
    variant={app.hasMailbox ? 'primary' : 'secondary'}
    icon="refresh-cw"
    label={de.toolbar.fetch}
    disabled={!app.hasMailbox || run.active}
    disabledReason={run.active ? run.busyText : de.toolbar.needsMailbox}
    wide
    testid={live ? 'fetch' : null}
    onclick={() => void run.start({ kind: 'fetch' })}
  />
{/snippet}

{#snippet cancelButton(live: boolean)}
  <Button
    variant="secondary"
    icon="circle-stop"
    label={de.toolbar.cancel}
    loading={live && run.cancelling}
    wide
    testid={live ? 'cancel-run' : null}
    onclick={() => void run.cancel()}
  />
{/snippet}

<div class="header" class:scrolled data-testid="list-header">
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
    <!-- The other button stands invisible in the same cell and only keeps the width. -->
    <span class="action">
      {#if run.fetching}
        <span class="live" in:fade>{@render cancelButton(true)}</span>
        <span class="spare" aria-hidden="true" inert>{@render fetchButton(false)}</span>
      {:else}
        <span class="live" in:fade>{@render fetchButton(true)}</span>
        <span class="spare" aria-hidden="true" inert>{@render cancelButton(false)}</span>
      {/if}
    </span>
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
    {#if jobs.filter !== null}
      <span class="filter" data-testid="filter" in:pop out:fade>
        <span class="filter-label">{de.list.filter[jobs.filter]}</span>
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="x"
          label={de.list.clearFilter}
          testid="clear-filter"
          onclick={() => jobs.setFilter(null)}
        />
      </span>
    {/if}
    {#if app.hasProfile}
      <span class="sort">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="arrow-up-down"
          label={de.toolbar.sortedBy[jobs.sortChoice]}
          turned={jobs.sortChoice === 'newest'}
          testid="sort"
          onclick={() => jobs.setSort(jobs.sortChoice === 'match' ? 'newest' : 'match')}
        />
      </span>
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
    border-bottom: var(--border-width) solid transparent;
    transition: border-color var(--dur-fast) var(--ease-standard);
  }

  .scrolled {
    border-bottom-color: var(--border);
  }

  .top {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-height: var(--list-toolbar);
  }

  .search {
    display: flex;
    flex: 1;
    min-width: 0;
  }

  /* Both buttons in one cell: the slot is as wide as the wider one. */
  .action {
    display: grid;
    flex: none;
  }

  .live,
  .spare {
    display: flex;
    grid-area: 1 / 1;
  }

  .spare {
    visibility: hidden;
  }

  .filters {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
  }

  /* The filter a tile or chip set: a soft navy pill with its x. */
  .filter {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    height: var(--control-sm);
    padding-left: var(--space-12);
    border-radius: var(--radius-full);
    background-color: var(--active-surface);
    color: var(--active-text);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .filter-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sort {
    display: flex;
    margin-left: auto;
  }
</style>
