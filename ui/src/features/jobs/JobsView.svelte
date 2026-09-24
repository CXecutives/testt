<!--
  The Jobs view on the white sheet of the shell: left the list column (360-460 px) with its
  header (search, "Abrufen", filters), the run panel and the list; a hairline; right the
  reader, or with nothing selected its empty state, the day overview. Nothing floats: no
  cards, no shadows. Both columns start at the same line; the handle between them resizes
  the list (the width is kept). Below 900 px one column: the list,
  or the reader with a back button. The run card rises in above the list and fades out when
  it is closed (the list moves up without animation).

  The right pane is a stage with its own scroll position. A job opens once its details are
  there: until then the pane keeps what it shows (the overview or the previous job), so it
  never goes blank. The new stage rises in over the old one, which keeps its own scroll
  position and fades: the new job starts at the top and the old text never jumps. The old
  stage is the real one on its way out (nothing is copied or laid out again); it answers no
  pointer and drops its test ids. The close button in the reader head, a second click on the
  selected row, Esc and a search that no longer finds the job go back to the day overview.
  The keys of a mail app (lib/input/input.ts): ArrowUp/ArrowDown open the previous/next job,
  Home/End the first/last, Ctrl+F (Cmd+F on macOS) goes to the search.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Button from '$components/Button.svelte';
  import DragBand from '$components/DragBand.svelte';
  import Splitter from '$components/Splitter.svelte';
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
  import { jobs, keyOf, sameKey } from '$lib/state/jobs.svelte';
  import { shell } from '$lib/state/shell.svelte';
  import { viewport } from '$lib/state/viewport.svelte';
  import DayOverview from './DayOverview.svelte';
  import JobList from './JobList.svelte';
  import Reader from './Reader.svelte';
  import ListHeader from './ListHeader.svelte';
  import RunCard from './RunCard.svelte';

  const OVERVIEW = 'overview';
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
    if (selected && jobs.detailStatus === 'error') next = ERROR;
    else if (jobs.detail !== null) next = keyOf(jobs.detail.job.key);
    else if (selected && jobs.detailSlow) next = WAITING;
    else if (!selected) next = OVERVIEW;
    if (next !== shown) {
      shown = next;
      turns += 1;
    }
    return { what: shown, turn: turns };
  });
  const reading = $derived(stage.what !== OVERVIEW);
  /** The list is scrolled away from its top (the header shows its hairline). */
  let scrolled = $state(false);
  /** The width of the list column (the splitter keeps it per user). */
  let listWidth = $state<number | undefined>(undefined);
  /** The first width (and the one a double click on the handle restores): 40 % of the
   *  window beside the sidebar, which the splitter keeps between 360 and 460 px. */
  const LIST_SHARE = 0.4;
  const firstWidth = Math.round((window.innerWidth - tokenPx('--sidebar-width')) * LIST_SHARE);

  function close(): void {
    jobs.clearSelection();
  }

  let header = $state<ListHeader | null>(null);
  let list = $state<JobList | null>(null);

  // A search that no longer finds the open job closes it (the list shows what it found).
  $effect(() => {
    const selected = jobs.selected;
    if (selected === null || jobs.search.trim() === '' || jobs.status !== 'ready') return;
    if (!jobs.visible.some((row) => sameKey(row.key, selected))) untrack(close);
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
    close,
    find: () => header?.find(),
  }}
>
  <div class="body">
    <aside class="left" use:cssVars={listWidth ? { 'list-width': `${listWidth}px` } : {}}>
      <ListHeader bind:this={header} {scrolled} />
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
        initial={firstWidth}
        storageKey="jobs-list-width"
        testid="list-splitter"
      /></span
    >
    <section class="right" data-testid="reader-pane">
      {#key stage.turn}
        <div class="stage" data-testid="stage" in:enter={stage.what !== OVERVIEW} out:leave>
          {#if dragBands()}<DragBand sheet />{/if}
          <div class="column">
            {#if stage.what === OVERVIEW}
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
    width: var(--list-width, clamp(var(--list-min), 40%, var(--list-max)));
    container-type: inline-size;
    min-height: 0;
    border-right: var(--border-width) solid var(--border);
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

  .scroll {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    overflow: auto;
  }

  /* One cell: a leaving stage and the next one lie on top of each other. */
  .right {
    display: grid;
    flex: 1;
    grid-template: minmax(0, 1fr) / minmax(0, 1fr);
    min-width: 0;
    overflow: hidden;
  }

  /* Each stage scrolls on its own; the sheet colour lets the next one cover the last. */
  .stage {
    grid-area: 1 / 1;
    min-height: 0;
    overflow: auto;
    background-color: var(--surface);
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    max-width: var(--reader-width);
    margin: 0 auto;
    padding: var(--pane-padding) var(--reader-padding) var(--space-64);
  }

  /* The chevron lines up with the title below it. */
  .back {
    display: none;
    margin-left: calc(-1 * var(--space-12));
  }

  .skeleton {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
  }

  @media (width < 900px) {
    /* One column scrolls as a whole, so the run card never squeezes the list. */
    .left {
      width: 100%;
      overflow: auto;
      border-right: 0;
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
