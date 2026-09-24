<!--
  The Jobs view on the white sheet of the shell: left the list column (360-460 px) with its
  header (search, "Abrufen", filters), the run panel and the list; a hairline; right the
  reader, or with nothing selected its empty state, the day overview. Nothing floats: no
  cards, no shadows. Both columns start at the same line. Below 900 px one column: the list,
  or the reader with a back button.

  The right pane is a stage with its own scroll position. A job opens once its details are
  there: until then the pane keeps what it shows (the overview or the previous job), so it
  never goes blank. The new stage rises in over a still picture of the old one, which keeps
  its own scroll position and fades: the new job starts at the top and the old text never
  jumps. The close button in the reader head goes back to the day overview.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import DragBand from '$components/DragBand.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import { de } from '$lib/i18n/de';
  import { play } from '$lib/motion/motion';
  import { fade, rise } from '$lib/motion/transitions';
  import { dragBands } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { jobs, keyOf } from '$lib/state/jobs.svelte';
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
  let last = OVERVIEW;
  const stage = $derived.by((): string => {
    const selected = jobs.selected !== null;
    if (selected && jobs.detailStatus === 'error') last = ERROR;
    else if (jobs.detail !== null) last = keyOf(jobs.detail.job.key);
    else if (selected && jobs.detailSlow) last = WAITING;
    else if (!selected) last = OVERVIEW;
    return last;
  });
  const reading = $derived(stage !== OVERVIEW);

  function close(): void {
    jobs.clearSelection();
  }

  /** A job rises in (4 px, 150 ms); the overview and the placeholders only fade (100 ms). */
  function enter(node: Element, job: boolean): ReturnType<typeof fade> {
    return job
      ? rise(node, { distance: 'sm', duration: 'base', easing: 'out' })
      : fade(node, { duration: 'fast' });
  }

  /**
   * The stage that leaves stays behind as a still picture: a copy without test ids, inert
   * and hidden from assistive technology, at the old scroll position. It fades while the
   * next stage rises over it. Nothing to leave behind when the pane is not on screen (the
   * one-column layout switches columns instead).
   */
  function leave(node: HTMLElement): Record<string, never> {
    if (viewport.narrow || node.getClientRects().length === 0) return {};
    const still = node.cloneNode(true) as HTMLElement;
    for (const element of [still, ...still.querySelectorAll('[data-testid]')]) {
      element.removeAttribute('data-testid');
    }
    still.inert = true;
    still.setAttribute('aria-hidden', 'true');
    node.after(still);
    still.scrollTop = node.scrollTop;
    const fading = play(still, [{ opacity: 1 }, { opacity: 0 }], {
      duration: 'base',
      easing: 'in',
      crossfade: true,
    });
    if (fading === null) still.remove();
    else fading.addEventListener('finish', () => still.remove());
    return {};
  }
</script>

<div class="jobs" class:reading data-testid="jobs">
  <div class="body">
    <aside class="left">
      <ListHeader />
      <div class="scroll" data-testid="list-scroll">
        {#if shell.runCard}
          <div class="run" in:rise={{ distance: 'md' }}><RunCard /></div>
        {/if}
        <JobList />
      </div>
    </aside>
    <section class="right" data-testid="reader-pane">
      {#key stage}
        <div class="stage" data-testid="stage" in:enter={stage !== OVERVIEW} out:leave>
          {#if dragBands()}<DragBand sheet />{/if}
          <div class="column">
            {#if stage === OVERVIEW}
              <DayOverview />
            {:else}
              <div class="back">
                <Button
                  variant="ghost"
                  size="sm"
                  icon="chevron-left"
                  label={de.common.back}
                  testid="back"
                  onclick={close}
                />
              </div>
              {#if stage === ERROR}
                <EmptyState
                  icon="triangle-alert"
                  tone="danger"
                  text={jobs.detailError ?? de.reader.loadFailed}
                  secondary={{
                    label: de.common.retry,
                    onclick: () => jobs.selected && void jobs.loadDetail(jobs.selected),
                  }}
                  testid="reader-error"
                />
              {:else if stage === WAITING}
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
    width: clamp(var(--list-min), 40%, var(--list-max));
    container-type: inline-size;
    min-height: 0;
    border-right: var(--border-width) solid var(--border);
  }

  .run {
    flex: none;
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

    .right {
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
