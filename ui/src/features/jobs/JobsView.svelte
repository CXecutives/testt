<!--
  The Jobs view on the white sheet of the shell: left the list column with its header
  (search, "Abrufen", filters), the run panel and the list; a hairline; right the reader, or
  with nothing selected its empty state, the day overview. Nothing floats: no cards, no
  shadows. Both columns start at the same line; the handle between them resizes the list
  (the width is kept) from 320 px up to 60 % of the content, as long as the reader keeps
  440 px; the limits follow the window and the sidebar. Below 900 px one column: the list
  under its pinned header (choosing rows keeps it, the header's bar acts on them), or the
  reader with a back button. The run card rises in above the list and fades out when it is
  closed (the list moves up without animation). Closing a job from inside the reader hands
  the focus to its row.

  The right pane is a stage with its own scroll position. A job opens once its details are
  there: until then the pane keeps what it shows (the overview or the previous job), so it
  never goes blank. The new stage rises in over the old one, which keeps its own scroll
  position and fades: the new job starts at the top and the old text never jumps. The old
  stage is the real one on its way out (nothing is copied or laid out again); it answers no
  pointer and drops its test ids. The close button in the reader head, Esc and a search
  that no longer finds the job go back to the day overview.
  The keys of a mail app (lib/input/input.ts): ArrowUp/ArrowDown open the previous/next job,
  Home/End the first/last, with Shift they choose from the open job on; Space on the open
  job's row pages through the reader, and after a click into the reader the arrows, Home and
  End scroll it; Ctrl+F (Cmd+F on macOS) goes to the search.
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import Button from '$components/Button.svelte';
  import DragBand from '$components/DragBand.svelte';
  import Splitter, { splitLimits } from '$components/Splitter.svelte';
  import { cssVars } from '$lib/actions/cssVars';
  import EmptyState from '$components/EmptyState.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import { t } from '$lib/i18n/t';
  import { fade, rise } from '$lib/motion/transitions';
  import { inView } from '$lib/actions/inView';
  import { listKeys } from '$lib/input/input';
  import { dragBands } from '$lib/platform';
  import { tokenPx } from '$lib/tokens';
  import { app } from '$lib/state/app.svelte';
  import { jobs, keyOf, placeOf, sameKey } from '$lib/state/jobs.svelte';
  import { shell } from '$lib/state/shell.svelte';
  import { viewport } from '$lib/state/viewport.svelte';
  import DayOverview from './DayOverview.svelte';
  import JobList from './JobList.svelte';
  import Reader from './Reader.svelte';
  import ListHeader from './ListHeader.svelte';
  import RunCard from './RunCard.svelte';
  import SelectionPane from './SelectionPane.svelte';
  import { bulk } from './bulk.svelte';

  const OVERVIEW = 'overview';
  const CHOSEN = 'chosen';
  const ERROR = 'error';
  const WAITING = 'waiting';

  // The list loads as soon as the app state is there (also after the first run page).
  $effect(() => {
    if (app.state !== null && jobs.status === 'idle') void jobs.start();
  });

  /**
   * What the right pane shows, as the key of its stage: a failed load of the selected job,
   * the loaded job (a job key never equals one of the words), a placeholder once loading
   * takes a while, else the day overview. While a job loads quickly the pane keeps what it
   * showed last.
   */
  let shown = OVERVIEW;
  /** Counts the changes: a stage that comes back while the old one still fades is new. */
  let turns = 0;
  const stage = $derived.by((): { what: string; turn: number } => {
    const selected = jobs.selected !== null;
    let next = shown;
    if (bulk.active) next = CHOSEN;
    else if (selected && jobs.detailStatus === 'error') next = ERROR;
    else if (jobs.detail !== null) next = keyOf(jobs.detail.job.key);
    else if (selected && jobs.detailSlow) next = WAITING;
    else if (!selected) next = OVERVIEW;
    if (next !== shown) {
      shown = next;
      turns += 1;
    }
    return { what: shown, turn: turns };
  });
  // One column shows the list or the reader. Choosing rows is no reading: the list stays and
  // its header's selection bar acts on the chosen rows.
  const reading = $derived(stage.what !== OVERVIEW && !(stage.what === CHOSEN && viewport.narrow));
  // One column: a job in place of the list hides the run card too (the sidebar says the run).
  $effect(() => {
    shell.listHidden = reading && viewport.narrow;
    return () => {
      shell.listHidden = false;
    };
  });
  const place = $derived(placeOf(jobs.facet));
  const trashDays = $derived(app.state?.autoEmptyTrashDays ?? 0);
  /** The place holds nothing (no search): the list says it, the reader adds no second tile. */
  const placeEmpty = $derived(
    jobs.status === 'ready' && jobs.total === 0 && jobs.search.trim() === '',
  );
  /** The list is scrolled away from its top (the header shows its hairline). */
  let scrolled = $state(false);
  /** The same for the whole column, which scrolls in the one-column layout. */
  let columnScrolled = $state(false);
  /** The width of the list column (the splitter keeps it per user). */
  let listWidth = $state<number | undefined>(undefined);
  /** The content beside the sidebar (and the sheet's hairline): the list's limits and its
   *  first width follow it when the window resizes (the sidebar turns to its rail too). */
  const content = $derived(
    viewport.width -
      tokenPx(viewport.rail ? '--rail-width' : '--sidebar-width') -
      tokenPx('--border-width'),
  );
  const limits = $derived(splitLimits(content));

  let right = $state<HTMLElement | null>(null);

  /**
   * Back to the day overview. A focus inside the reader (its close button, Esc on one of its
   * buttons) goes to the row of the job that was open, like Mail: the list keeps its place
   * and Tab goes on from there.
   */
  function close(): void {
    const open = jobs.selected;
    const focused = document.activeElement;
    const inReader = right !== null && focused !== null && right.contains(focused);
    // In one column the list comes back: at the open job's row, which gets the focus unless
    // the pointer was elsewhere (the keys stepped through jobs the list never showed).
    const lost = focused === null || focused === document.body;
    jobs.clearSelection();
    if (open === null || !(inReader || (viewport.narrow && lost))) return;
    if (jobs.shown.some((job) => sameKey(job.key, open))) {
      jobs.reveal = { key: keyOf(open), focus: true };
    }
  }

  let header = $state<ListHeader | null>(null);
  let list = $state<JobList | null>(null);

  // A search that no longer finds the open job closes it (the list shows what it found);
  // only a change of the search does, never a job opened from elsewhere (the overview).
  // Only a list that holds every hit can tell: a hit beyond the loaded rows stays open.
  let searched = untrack(() => jobs.search.trim());
  let searchChanged = false;
  $effect(() => {
    const search = jobs.search.trim();
    untrack(() => {
      if (search !== searched) searchChanged = true;
      searched = search;
    });
  });
  $effect(() => {
    const selected = jobs.selected;
    const ready = jobs.status === 'ready';
    const listed = jobs.visible;
    untrack(() => {
      if (!searchChanged || !ready) return;
      searchChanged = false;
      if (selected === null || searched === '') return;
      const complete = jobs.rows.length >= jobs.total;
      if (complete && !listed.some((row) => sameKey(row.key, selected))) close();
    });
  });

  // Back to the Jobs view: Neu is entered again, so the jobs read meanwhile leave it (like
  // Mail); the open one stays until another opens.
  $effect(() => {
    untrack(() => {
      if (jobs.status === 'ready' && jobs.facet === 'new') void jobs.load(true, false);
    });
  });

  /** A job rises in (4 px, 150 ms); the overview and the placeholders only fade (100 ms). */
  function enter(node: Element, job: boolean): ReturnType<typeof fade> {
    return job
      ? rise(node, { distance: 'sm', duration: 'base', easing: 'out' })
      : fade(node, { duration: 'fast' });
  }

  /**
   * The stage that leaves stays where it is, at its own scroll position, and fades (150 ms,
   * ease-in) while the next one rises over it. On its way out it is hidden from assistive
   * technology and drops its test ids, so nothing on the page exists twice. The one-column
   * layout switches columns instead: there the old stage simply goes.
   */
  function leave(node: HTMLElement): ReturnType<typeof fade> | Record<string, never> {
    if (viewport.narrow) return {};
    for (const element of [node, ...node.querySelectorAll('[data-testid]')]) {
      element.removeAttribute('data-testid');
    }
    node.setAttribute('aria-hidden', 'true');
    return fade(node, { duration: 'base', easing: 'in' });
  }
</script>

<div
  class="jobs"
  class:reading
  data-testid="jobs"
  use:listKeys={{
    step: (by) => list?.step(by),
    edge: (last) => list?.edge(last),
    extend: (to) => list?.extend(to),
    close,
    // In one column the search sits in the hidden list: the open job closes first.
    find: () => {
      if (viewport.narrow && jobs.selected !== null) {
        close();
        void tick().then(() => header?.find());
      } else header?.find();
    },
    // The stage on screen: the one on its way out has dropped its test ids.
    reader: () => right?.querySelector<HTMLElement>('[data-testid="stage"]') ?? null,
  }}
>
  <div class="body">
    <aside class="left" use:cssVars={listWidth ? { 'list-width': `${listWidth}px` } : {}}>
      <!-- One column scrolls the whole column under its pinned header: watched from its top. -->
      <span class="top" use:inView={(place) => (columnScrolled = place === 'above')}></span>
      <div class="head">
        <ListHeader bind:this={header} scrolled={scrolled || columnScrolled} />
      </div>
      <div class="scroll" data-testid="list-scroll">
        <span class="top" use:inView={(place) => (scrolled = place === 'above')}></span>
        {#if shell.runCard}
          <div class="run" in:rise={{ distance: 'md' }} out:fade>
            <RunCard />
          </div>
        {/if}
        <JobList bind:this={list} />
      </div>
    </aside>
    <span class="split"
      ><Splitter
        bind:size={listWidth}
        initial={limits.initial}
        min={limits.min}
        max={limits.max}
        storageKey="jobs-list-width"
        testid="list-splitter"
      /></span
    >
    <section class="right" data-testid="reader-pane" bind:this={right}>
      {#key stage.turn}
        <div class="stage" data-testid="stage" in:enter={stage.what !== OVERVIEW} out:leave>
          {#if dragBands()}<DragBand sheet />{/if}
          <div class="column">
            {#if stage.what === CHOSEN}
              <SelectionPane />
            {:else if stage.what === OVERVIEW && place !== 'inbox'}
              <!-- The archive and the trash have no day overview: what lies here, quietly.
                   Beside an empty list, which shows its own empty state, only the sentence. -->
              {@const text =
                place === 'trash' && trashDays > 0
                  ? t.place.trashFor(trashDays)
                  : t.place.reader[place]}
              <div class="place-reader">
                {#if placeEmpty}
                  <p class="place-note" data-testid="place-reader">{text}</p>
                {:else}
                  <EmptyState
                    icon={place === 'trash' ? 'trash-2' : 'archive'}
                    tone="neutral"
                    {text}
                    testid="place-reader"
                  />
                {/if}
              </div>
            {:else if stage.what === OVERVIEW}
              <DayOverview />
            {:else}
              <div class="back">
                <Button
                  variant="ghost"
                  size="sm"
                  icon="chevron-left"
                  label={t.common.back}
                  testid="back"
                  onclick={close}
                />
              </div>
              {#if stage.what === ERROR}
                <EmptyState
                  icon="triangle-alert"
                  tone="danger"
                  text={jobs.detailError ?? t.reader.loadFailed}
                  secondary={{
                    label: t.common.retry,
                    icon: 'refresh-cw',
                    onclick: () => jobs.selected && void jobs.loadDetail(jobs.selected),
                  }}
                  testid="reader-error"
                />
              {:else if stage.what === WAITING}
                <div class="skeleton" data-testid="reader-skeleton">
                  <Skeleton width={80} />
                  <Skeleton width={55} />
                  <Skeleton shape="circle" size="md" />
                  <Skeleton shape="block" />
                </div>
              {:else if jobs.detail}
                <Reader detail={jobs.detail} onclose={close} />
              {/if}
            {/if}
          </div>
        </div>
      {/key}
    </section>
  </div>
</div>

<style>
  .jobs {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .left {
    display: flex;
    flex: none;
    flex-direction: column;
    width: var(--list-width, clamp(var(--list-min), 40%, var(--list-first-max)));
    min-height: 0;
    border-right: var(--border-width) solid var(--border);
  }

  /* The header asks the column's width (its second row wraps in a narrow column): the query
     container is the header's box, as wide as the column, and not the column itself. Around
     the list a query container made every layout of the view half as long again (the
     reader's ring fill lays the view out in each of its frames).
     It keeps the room of the list's scrollbar beside it, empty (a scroller that never
     scrolls): its search, Abrufen and tools end where the rows' text ends, with the engine's
     own scrollbar width (Windows 8 px, overlay scrollbars none). */
  .head {
    flex: none;
    overflow-x: hidden;
    overflow-y: scroll;
    container-type: inline-size;
  }

  .run {
    flex: none;
  }

  /* The handle lies over the list's border and takes no room. */
  .split {
    display: flex;
    flex: none;
  }

  /* Watched: once it has scrolled away, the list header draws its bottom line. */
  .top {
    flex: none;
    height: 0;
  }

  /* The list and the reader keep their scrollbar's room whether they scroll or not, like the
     views of the shell (Windows: a transparent track, the thumb only under the pointer;
     macOS overlay scrollbars take none): the rows end on the header's line, and the reader
     does not move sideways between a short and a long job. */
  .scroll {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    overflow-x: auto;
    overflow-y: scroll;
  }

  /* One cell: a leaving stage and the next one lie on top of each other. */
  .right {
    display: grid;
    flex: 1;
    grid-template: minmax(0, 1fr) / minmax(0, 1fr);
    min-width: 0;
    overflow: hidden;
  }

  /* Each stage scrolls on its own; the sheet colour lets the next one cover the last. The
     keyboard focus stops below the macOS toolbar row and the reader's compact bar. */
  .stage {
    grid-area: 1 / 1;
    min-height: 0;
    overflow-x: auto;
    overflow-y: scroll;
    background-color: var(--surface);
    scroll-padding-top: calc(var(--window-top) + var(--compact-header));
  }

  /* Centred on a whole pixel (rounded down to the step of a hairline): an odd pane width
     would put the text on a half one. */
  .column {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    max-width: var(--reader-width);
    margin: 0 auto 0
      max(0%, round(down, calc((100% - var(--reader-width)) / 2), var(--border-width)));
    padding: var(--pane-padding) var(--reader-padding) var(--space-64);
  }

  /* The chevron's ink lines up with the title below it: the button's padding and border
     hang out, and so does the glyph's own inset in its box. */
  .back {
    display: none;
    margin-left: calc(-1 * (var(--ghost-inset) + var(--space-6)));
  }

  /* The reader of the archive and the trash with nothing open: centred across the pane,
     near its top. */
  .place-reader {
    display: flex;
    justify-content: center;
    padding-top: var(--space-48);
  }

  .place-note {
    max-width: var(--measure-intro);
    color: var(--text-muted);
    font: var(--type-body);
    text-align: center;
  }

  .skeleton {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
  }

  @media (width < 900px) {
    /* One column scrolls as a whole, so the run card never squeezes the list; the header
       (search, Abrufen, on macOS the toolbar row that moves the window) stays on top, and a
       row brought into view stops below it (the header at its tallest, two lines). */
    .left {
      width: 100%;
      overflow-x: auto;
      overflow-y: scroll;
      border-right: 0;
      scroll-padding-top: calc(
        var(--list-header-top) + var(--list-toolbar) + var(--space-12) + 2 * var(--control-sm) +
          var(--space-8) + var(--pane-padding)
      );
    }

    /* Inside the column's scroller: the scrollbar runs beside it already. */
    .head {
      position: sticky;
      top: 0;
      z-index: var(--z-sticky);
      overflow: visible;
      background-color: var(--surface);
    }

    .scroll {
      flex: none;
      overflow: visible;
    }

    .right,
    .split {
      display: none;
    }

    .reading .left {
      display: none;
    }

    .reading .right {
      display: grid;
    }

    .back {
      display: block;
    }

    .column {
      padding-bottom: var(--space-32);
    }
  }
</style>
