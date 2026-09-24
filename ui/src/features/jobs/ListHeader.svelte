<!--
  The header of the list column, every list control in one place.
  Row 1: the search and next to it "Abrufen", the one primary of the Jobs view, which fills
  this list ("Abbrechen" in its place while a fetch or details run goes; locked while the
  app scores the jobs anew, and without a mailbox, saying why). The action slot is as wide as the wider of the two and both fill it, so the
  search never jumps when a run starts; the one that comes fades in, the one that goes is
  gone at once.
  On macOS this row is the list's part of the toolbar row, centred on the traffic lights,
  and its empty parts move the window.
  Row 2: the one place for filters, Neu · Alle · Gemerkt · Bewerbungen with their counts
  (always there, also while the reader is open); the archive (archived jobs), reached from the end of
  Alle, show as a pill with its x instead.
  Row 3 (with a profile): the order in words ("Beste Passung", "Neueste"), a quiet button
  whose glyph stands half a turn for newest first; a click switches it.
  The bottom hairline shows only once the list below is scrolled.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Segmented from '$components/Segmented.svelte';
  import TextField from '$components/TextField.svelte';
  import { t } from '$lib/i18n/t';
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

  /** Gemerkt is Alle with the pinned filter (until the backend has a list of its own). */
  type View = JobFacet | 'pinned';
  const counts = $derived(jobs.overviewCounts ?? jobs.counts);
  const views = $derived([
    { id: 'new' as View, label: t.toolbar.facetNew, count: jobs.counts.new },
    { id: 'all' as View, label: t.toolbar.facetAll, count: jobs.counts.all },
    // An empty list of the user's own shows no zero (the row stays narrow).
    { id: 'pinned' as View, label: t.toolbar.facetPinned, count: counts.saved || null },
    {
      id: 'applications' as View,
      label: t.toolbar.facetApplications,
      count: counts.applications || null,
    },
  ]);
  const view = $derived<View>(jobs.filter === 'pinned' ? 'pinned' : jobs.facet);

  function choose(id: View): void {
    if (id === 'pinned') jobs.setFilter('pinned');
    else jobs.setFacet(id);
  }
  /** A filter the segments do not name (a portal, a tile). */
  const otherFilter = $derived(
    jobs.filter !== null && jobs.filter !== 'pinned' ? jobs.filter : null,
  );
</script>

{#snippet fetchButton(live: boolean)}
  <Button
    variant={app.hasMailbox ? 'primary' : 'secondary'}
    icon="refresh-cw"
    label={t.toolbar.fetch}
    disabled={!app.hasMailbox || run.active}
    disabledReason={run.active ? run.busyText : t.toolbar.needsMailbox}
    wide
    testid={live ? 'fetch' : null}
    onclick={() => void run.start({ kind: 'fetch' })}
  />
{/snippet}

{#snippet cancelButton(live: boolean)}
  <Button
    variant="secondary"
    icon="circle-stop"
    label={t.toolbar.cancel}
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
        label={t.toolbar.searchLabel}
        placeholder={t.toolbar.search}
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
    {#if jobs.facet === 'archived'}
      <span class="filter" data-testid="filter" in:pop out:fade>
        <span class="filter-label">{t.list.hidden}</span>
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="x"
          label={t.list.clearFilter}
          testid="clear-filter"
          onclick={() => jobs.setFacet('all')}
        />
      </span>
    {:else}
      <Segmented
        options={views}
        value={view}
        label={t.toolbar.facet}
        size="sm"
        testid="facet"
        onchange={choose}
      />
    {/if}
    {#if otherFilter !== null}
      <span class="filter" data-testid="filter" in:pop out:fade>
        <span class="filter-label">{t.list.filter[otherFilter]}</span>
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="x"
          label={t.list.clearFilter}
          testid="clear-filter"
          onclick={() => jobs.setFilter(null)}
        />
      </span>
    {/if}
  </div>
  {#if app.hasProfile}
    <span class="sort">
      <Button
        variant="ghost"
        size="sm"
        icon="arrow-up-down"
        label={t.toolbar.sortLabel[jobs.sortChoice]}
        turned={jobs.sortChoice === 'newest'}
        testid="sort"
        onclick={() => jobs.setSort(jobs.sortChoice === 'match' ? 'newest' : 'match')}
      />
    </span>
  {/if}
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

  /* A narrow column keeps the chosen segment's count only. */
  @container (width < 440px) {
    .filters :global(.count.plain) {
      display: none;
    }
  }

  /* The order in words; its glyph starts on the edge of the column. */
  .sort {
    display: flex;
    margin: calc(-1 * var(--space-4)) 0 calc(-1 * var(--space-4)) calc(-1 * var(--space-12));
  }
</style>
