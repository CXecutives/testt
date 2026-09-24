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
  Row 3: the order, a quiet button with its name and a chevron ("Nach Passung", "Nach
  Datum") that opens the OS's own menu with both, the current one ticked; one choice for
  every list, kept. Without a usable profile it says "Nach Datum" and cannot open (why, in
  its tooltip). Under Favoriten the favourites can be copied as one prompt for any AI chat.
  The bottom hairline shows only once the list below is scrolled.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Count from '$components/Count.svelte';
  import Dialog from '$components/Dialog.svelte';
  import Segmented from '$components/Segmented.svelte';
  import Notice from '$components/Notice.svelte';
  import TextField from '$components/TextField.svelte';
  import { t } from '$lib/i18n/t';
  import type { JobFacet } from '$lib/ipc/types';
  import { fade, pop } from '$lib/motion/transitions';
  import { dragBands } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { popupChoiceMenu } from '$lib/ipc/api';
  import type { JobSort } from '$lib/ipc/types';
  import { jobs } from '$lib/state/jobs.svelte';
  import { run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
  import { tooltip } from '$lib/actions/tooltip';
  import { copyTopPrompt } from './prompt';

  interface Props {
    /** The list below is scrolled away from its top. */
    scrolled?: boolean;
  }
  let { scrolled = false }: Props = $props();

  // Every count follows the search, the same way for every segment.
  const views = $derived([
    { id: 'new' as JobFacet, label: t.toolbar.facetNew, count: jobs.counts.new },
    { id: 'all' as JobFacet, label: t.toolbar.facetAll, count: jobs.counts.all },
    // An empty list of the user's own shows no zero (the row stays narrow).
    { id: 'saved' as JobFacet, label: t.toolbar.facetSaved, count: jobs.counts.saved || null },
  ]);

  let searchBox = $state<HTMLElement | null>(null);

  /** Ctrl+F (Cmd+F on macOS, lib/input/input.ts): into the search, its text selected. */
  export function find(): void {
    const input = searchBox?.querySelector('input');
    input?.focus();
    input?.select();
  }

  const SORTS: readonly JobSort[] = ['match', 'newest'];

  /** The order menu opens under its button, the current order ticked. */
  function chooseSort(event: MouseEvent): void {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    void popupChoiceMenu(
      SORTS.map((sort) => ({
        text: t.toolbar.sortLabel[sort],
        checked: jobs.sortChoice === sort,
        onselect: () => jobs.setSort(sort),
      })),
      { x: box.left, y: box.bottom },
    );
  }

  let promptError = $state<string | null>(null);
  let confirmEmpty = $state(false);
  let emptying = $state(false);
  let emptyError = $state<string | null>(null);

  /** A filter the segments do not name (a portal, a tile). */
  const otherFilter = $derived(jobs.filter);

  async function emptyArchive(): Promise<void> {
    emptying = true;
    emptyError = null;
    const result = await jobs.emptyArchive();
    emptying = false;
    if ('error' in result) {
      emptyError = result.error;
      return;
    }
    confirmEmpty = false;
    toasts.show(t.toast.deleted(result.count));
    void jobs.loadOverview();
  }
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
    <span class="search" bind:this={searchBox}>
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
        <span class="filter-label">{t.list.archive}</span>
        <Count value={jobs.counts.archived} tone="plain" />
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
        value={jobs.facet}
        label={t.toolbar.facet}
        size="sm"
        testid="facet"
        onchange={(id) => jobs.setFacet(id)}
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
  <span class="order">
    <span class="sort">
      <Button
        variant="ghost"
        size="sm"
        label={t.toolbar.sortLabel[app.hasProfile ? jobs.sortChoice : 'newest']}
        menu
        disabled={!app.hasProfile}
        disabledReason={t.toolbar.sortNoProfile}
        testid="sort"
        onclick={chooseSort}
      />
    </span>
    <span class="order-tools">
      {#if jobs.facet === 'saved' && jobs.counts.saved > 0}
        <span use:tooltip={t.reader.promptHint}>
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            icon="copy"
            label={t.overview.promptTop}
            testid="prompt-pinned"
            onclick={() => void copyTopPrompt().then((error) => (promptError = error))}
          />
        </span>
      {/if}
      {#if jobs.facet === 'archived'}
        {#if jobs.counts.archived > 0}
          <Button
            variant="ghost"
            size="sm"
            icon="trash-2"
            label={t.list.emptyArchive}
            testid="empty-archive"
            onclick={() => (confirmEmpty = true)}
          />
        {/if}
      {:else if jobs.counts.archived > 0}
        <!-- The archive, reachable from every list; its count follows the search. -->
        <Button
          variant="ghost"
          size="sm"
          icon="archive"
          label={t.list.archiveLink(jobs.counts.archived)}
          testid="show-archive"
          onclick={() => jobs.setFacet('archived')}
        />
      {/if}
    </span>
  </span>
  {#if promptError}
    <Notice tone="danger" variant="inline" text={promptError} />
  {/if}
</div>

<Dialog
  bind:open={confirmEmpty}
  variant="danger"
  heading={t.list.emptyArchiveHeading}
  text={t.list.emptyArchiveText}
  confirmLabel={t.list.emptyArchive}
  busy={emptying}
  error={emptyError}
  testid="dialog-empty-archive"
  onconfirm={() => void emptyArchive()}
/>

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
  /* The order in words and, under Gemerkt, the pinned jobs as one prompt. */
  .order {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-8);
    margin-right: calc(-1 * var(--space-12));
  }

  .sort {
    display: flex;
    margin: calc(-1 * var(--space-4)) 0 calc(-1 * var(--space-4)) calc(-1 * var(--space-12));
  }
</style>
