<!--
  The Jobs view: toolbar on top, left the run card and the list (360-440 px), right the
  reader or, with nothing selected, the day overview. Below 900 px one column: the list, or
  the reader with a back button.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import { de } from '$lib/i18n/de';
  import { app } from '$lib/state/app.svelte';
  import { fade, rise } from '$lib/motion/transitions';
  import { jobs, keyOf } from '$lib/state/jobs.svelte';
  import { run } from '$lib/state/run.svelte';
  import DayOverview from './DayOverview.svelte';
  import JobList from './JobList.svelte';
  import Reader from './Reader.svelte';
  import RunCard from './RunCard.svelte';
  import Toolbar from './Toolbar.svelte';

  let pane = $state<HTMLElement | null>(null);

  // The list loads as soon as the app state is there (also after the first run page).
  $effect(() => {
    if (app.state !== null && jobs.status === 'idle') void jobs.start();
  });

  // A new selection starts the reader at the top.
  $effect(() => {
    void jobs.selected;
    pane?.scrollTo({ top: 0 });
  });
</script>

<div class="jobs" class:reading={jobs.selected !== null} data-testid="jobs">
  <Toolbar />
  <div class="body">
    <aside class="left">
      {#if run.active || (run.panel !== 'hidden' && (run.summary ?? app.state?.lastRun))}
        <div class="run" in:rise={{ distance: 'md' }}><RunCard /></div>
      {/if}
      <div class="scroll" data-testid="list-scroll"><JobList /></div>
    </aside>
    <section class="right" bind:this={pane} data-testid="reader-pane">
      <div class="column">
        {#if jobs.selected !== null}
          <div class="back">
            <Button
              variant="ghost"
              size="sm"
              icon="chevron-left"
              label={de.common.back}
              testid="back"
              onclick={() => jobs.clearSelection()}
            />
          </div>
          {#if jobs.detail}
            {#key keyOf(jobs.detail.job.key)}
              <div in:fade={{ duration: 'base' }}><Reader detail={jobs.detail} /></div>
            {/key}
          {:else if jobs.detailStatus === 'error'}
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
          {:else if jobs.detailSlow}
            <Card padding="lg" testid="reader-skeleton">
              <div class="skeleton">
                <Skeleton shape="circle" size="lg" />
                <Skeleton width={80} />
                <Skeleton width={55} />
                <Skeleton shape="block" />
              </div>
            </Card>
          {/if}
        {:else}
          <div in:fade={{ duration: 'base' }}><DayOverview /></div>
        {/if}
      </div>
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
    width: clamp(var(--list-min), 38%, var(--list-max));
    container-type: inline-size;
    min-height: 0;
    border-right: var(--border-width) solid var(--border);
    background-color: var(--surface);
  }

  .run {
    flex: none;
    padding: var(--space-16);
    border-bottom: var(--border-width) solid var(--border);
    background-color: var(--bg);
  }

  .scroll {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    overflow: auto;
  }

  .right {
    flex: 1;
    min-width: 0;
    overflow: auto;
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    max-width: var(--reader-width);
    margin: 0 auto;
    padding: var(--space-32) var(--space-32) var(--space-64);
  }

  .back {
    display: none;
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
      display: block;
    }

    .back {
      display: block;
    }

    .column {
      padding: var(--space-16) var(--space-16) var(--space-32);
    }
  }
</style>
