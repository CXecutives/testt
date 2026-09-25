<!--
  The header of the list column, two rows.
  Row 1: the search, whose placeholder names what it searches ("Jobs durchsuchen", "Archiv
  durchsuchen", "Papierkorb durchsuchen"), and next to it "Abrufen", the one primary of the
  Jobs view, which fills the inbox ("Abbrechen" in its place while a fetch or details run
  goes; locked while the app scores the jobs anew, and without a mailbox, saying why). The
  action slot is as wide as the wider of the two and both fill it, so the search never jumps
  when a run starts; the one that comes fades in, the one that goes is gone at once. On
  macOS this row is the list's part of the toolbar row, centred on the traffic lights, and
  its empty parts move the window.
  Row 2 in the inbox: Neu · Alle · Favoriten with their counts, and at its end "Alle als
  gelesen markieren" (while there are unread jobs; the toast takes it back) and the order, a
  quiet button that opens the OS's own menu (Nach Passung, Nach Datum; one choice for every
  list, kept; without a usable profile by date, saying why). In the Archiv and the
  Papierkorb: how many jobs lie there, the order, and in the Papierkorb "Papierkorb leeren"
  (asks first). While two or more jobs are chosen, the selection bar takes this row: how
  many, the place's actions, "Auswahl aufheben" (Esc too). The row keeps one height in every
  state; where the column is narrow its tools wrap under the segments. The bottom hairline
  shows only once the list below is scrolled.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Dialog from '$components/Dialog.svelte';
  import MenuButton from '$components/MenuButton.svelte';
  import Notice from '$components/Notice.svelte';
  import Segmented from '$components/Segmented.svelte';
  import SelectionBar, { type SelectionAction } from '$components/SelectionBar.svelte';
  import TextField from '$components/TextField.svelte';
  import { t } from '$lib/i18n/t';
  import type { JobSort } from '$lib/ipc/types';
  import { fade } from '$lib/motion/transitions';
  import { dragBands } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { isExcluded, jobs, placeOf, type JobFacet } from '$lib/state/jobs.svelte';
  import { run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
  import { actionsFor, hasStar, move, purge, toggleStar, trashEmptied } from './actions';
  import { selection } from './selection.svelte';

  interface Props {
    /** The list below is scrolled away from its top. */
    scrolled?: boolean;
  }
  let { scrolled = false }: Props = $props();

  const place = $derived(placeOf(jobs.facet));
  const inInbox = $derived(place === 'inbox');

  // Each segment counts its list (they follow the search): the unread ones always in the warm
  // pill, the others plain, whichever is chosen, so the control keeps its width; no zero.
  const views = $derived([
    {
      id: 'new' as JobFacet,
      label: t.toolbar.facetNew,
      count: jobs.counts.unread || null,
      tone: 'soft' as const,
    },
    {
      id: 'all' as JobFacet,
      label: t.toolbar.facetAll,
      count: jobs.counts.inbox || null,
      tone: 'plain' as const,
    },
    {
      id: 'favourites' as JobFacet,
      label: t.toolbar.facetSaved,
      count: jobs.counts.favourites || null,
      tone: 'plain' as const,
    },
  ]);
  /** The jobs of this place (with the search), for the count and whether to order. */
  const inPlace = $derived(
    jobs.facet === 'favourites' ? jobs.counts.favourites : jobs.counts[place],
  );

  let searchBox = $state<HTMLElement | null>(null);

  /** Ctrl+F (Cmd+F on macOS, lib/input/input.ts): into the search, its text selected. */
  export function find(): void {
    const input = searchBox?.querySelector('input');
    input?.focus();
    input?.select();
  }

  const SORTS: readonly JobSort[] = ['match', 'newest'];
  const sorts = $derived(SORTS.map((sort) => ({ id: sort, label: t.toolbar.sortLabel[sort] })));

  let error = $state<string | null>(null);

  /** "Alle als gelesen markieren": every unread job of the inbox; the toast takes it back. */
  async function markAllRead(): Promise<void> {
    error = null;
    const result = await jobs.markAllRead();
    if ('error' in result) {
      error = result.error;
      return;
    }
    void jobs.loadOverview();
    toasts.show(t.toast.allRead, 'success', {
      label: t.common.undo,
      onclick: () => {
        void jobs.markUnread(result.keys).then(() => jobs.loadOverview());
      },
    });
  }

  /* ------------------------------------------------------------------- selection */

  const rows = $derived([
    ...jobs.shown.filter((job) => !isExcluded(job)),
    ...jobs.shown.filter(isExcluded),
  ]);
  const chosen = $derived(selection.jobs(rows));
  let confirmPurge = $state(false);
  let purging = $state(false);

  const barActions = $derived.by((): SelectionAction[] => {
    // What fits every chosen job (Favoriten may hold jobs of the inbox and the archive).
    const out: SelectionAction[] = actionsFor(chosen).map((action) => ({
      icon: action.icon,
      label: action.label,
      testid: `selection-${action.id}`,
      // Deleting for good waits for a run (the backend refuses meanwhile).
      disabled: action.id === 'purge' && run.active,
      disabledReason: run.busyText,
      onclick: () => {
        if (action.id === 'purge') {
          purgeError = null;
          confirmPurge = true;
          return;
        }
        const list = chosen;
        selection.clear();
        void move(list, action.id).then((failed) => (error = failed));
      },
    }));
    if (chosen.every((job) => hasStar(job.place))) {
      const on = chosen.some((job) => !job.pinned);
      out.push({
        icon: 'star',
        label: on ? t.reader.pin : t.reader.unpin,
        testid: 'selection-star',
        onclick: () => toggleStar(chosen),
      });
    }
    return out;
  });

  let purgeError = $state<string | null>(null);

  async function purgeChosen(): Promise<void> {
    purging = true;
    purgeError = await purge(chosen);
    purging = false;
    if (purgeError === null) {
      confirmPurge = false;
      selection.clear();
    }
  }

  /* ----------------------------------------------------------------------- trash */

  let confirmEmpty = $state(false);
  let emptying = $state(false);
  let emptyError = $state<string | null>(null);
  /** Every job of the trash, whatever the search: emptying it deletes them all. */
  const inTrash = $derived(jobs.overviewCounts?.trash ?? jobs.counts.trash);
  const searching = $derived(jobs.search.trim() !== '');

  async function emptyTrash(): Promise<void> {
    emptying = true;
    emptyError = null;
    const result = await jobs.emptyTrash();
    emptying = false;
    if ('error' in result) {
      emptyError = result.error;
      return;
    }
    confirmEmpty = false;
    trashEmptied();
    toasts.show(t.toast.trashEmptied);
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
        label={t.place.search[place]}
        placeholder={t.place.search[place]}
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
  <div class="second">
    {#if chosen.length >= 2}
      <SelectionBar
        count={chosen.length}
        actions={barActions}
        onclear={() => selection.clear()}
        testid="selection-bar"
      />
    {:else}
      {#if inInbox}
        <Segmented
          options={views}
          value={jobs.facet}
          label={t.toolbar.facet}
          size="sm"
          testid="facet"
          onchange={(id) => jobs.setFacet(id)}
        />
      {:else}
        <!-- An empty place says so in the list; no "0 Jobs" above it. -->
        {#if inPlace > 0}
          <span class="place-count" data-testid="place-count">{t.place.count[place](inPlace)}</span>
        {/if}
      {/if}
      <span class="tools">
        {#if inInbox && jobs.facet !== 'favourites' && !searching && jobs.counts.unread > 0}
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            icon="check-check"
            label={t.actions.markAllRead}
            testid="mark-all-read"
            onclick={() => void markAllRead()}
          />
        {/if}
        {#if inPlace > 0}
          <MenuButton
            options={sorts}
            value={app.hasProfile ? jobs.sortChoice : 'newest'}
            disabled={!app.hasProfile}
            disabledReason={t.toolbar.sortNoProfile}
            testid="sort"
            onchange={(sort) => jobs.setSort(sort)}
          />
        {/if}
        {#if place === 'trash' && !searching && inTrash > 0}
          <Button
            variant="ghost"
            size="sm"
            icon="trash-2"
            label={t.actions.emptyTrash}
            disabled={run.active}
            disabledReason={run.busyText}
            testid="empty-trash"
            onclick={() => {
              emptyError = null;
              confirmEmpty = true;
            }}
          />
        {/if}
      </span>
    {/if}
  </div>
  {#if error}
    <Notice tone="danger" variant="inline" text={error} testid="header-error" />
  {/if}
</div>

<Dialog
  bind:open={confirmEmpty}
  variant="danger"
  heading={t.actions.emptyTrashHeading}
  text={t.actions.emptyTrashText(inTrash)}
  confirmLabel={t.actions.emptyTrash}
  busy={emptying}
  error={emptyError}
  testid="dialog-empty-trash"
  onconfirm={() => void emptyTrash()}
/>

<Dialog
  bind:open={confirmPurge}
  variant="danger"
  heading={t.actions.purgeHeading(chosen.length)}
  text={t.actions.purgeText}
  confirmLabel={t.actions.purge}
  busy={purging}
  error={purgeError}
  testid="dialog-purge-chosen"
  onconfirm={() => void purgeChosen()}
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

  /* The second row: one height in every state; the tools wrap under a narrow column's
     segments, at its end. */
  .second {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
    min-height: var(--control-sm);
  }

  .place-count {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .tools {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-left: auto;
    margin-right: calc(-1 * var(--space-12));
  }
</style>
