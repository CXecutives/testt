<!--
  The run panel on top of the list (flat on the sheet, a hairline below), shown only while a
  run is going or right after it (or when the run status in the sidebar is clicked); it
  collapses to its header line (the chevron turns, the rest fades in when it opens and is
  gone at once when it closes) and closes.
  running: the header line (the spinner, the status naming the portal it is about, which
  cross-fades when it changes, and the countdown of a pause as a soft navy pill), the navy
  progress bar right below it, the steps of the kind side by side (a fetch: Postfach,
  Details, Bewertung; a details run: Details, Bewertung) with a navy dot for the current
  one, a check that draws itself when a step finishes while the card is on screen, and
  counters that roll; then every limit or pause with its reason and end.
  finished (the header cross-fades from the running one): the outcome, its time and, for a
  fetch, the pills "n neu" and "n passen gut" (the run's own numbers from the backend;
  nothing when there are none, the note says it), a details run what it got, a rescore only
  that it is done; then what went wrong with a fitting action, a file the export could not
  write (once), the history with copy. The overview file and the folder have their one
  place in the day overview. A rescore shows here only when it failed or could not write
  the files.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Badge from '$components/Badge.svelte';
  import Button from '$components/Button.svelte';
  import Disclosure from '$components/Disclosure.svelte';
  import Icon from '$components/Icon.svelte';
  import Meter from '$components/Meter.svelte';
  import Notice from '$components/Notice.svelte';
  import Spinner from '$components/Spinner.svelte';
  import { t } from '$lib/i18n/t';
  import { formatMoment, formatNumber, formatTime } from '$lib/i18n/format';
  import { DETAIL_WARNS, errorText, healthAdvice } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { OpenTarget, Portal, PortalHealth, Step } from '$lib/ipc/types';
  import { fade, roll } from '$lib/motion/transitions';
  import { app } from '$lib/state/app.svelte';
  import { fileManager } from '$lib/platform';
  import {
    exportError,
    exportText,
    failureAction,
    isFetch,
    needsAction,
    outcomeText,
    run,
  } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
  import { copyText } from './prompt';

  // The run the card followed, else the last fetch (after a restart).
  const summary = $derived(run.result ?? app.state?.lastRun ?? null);
  const open = $derived(run.panel === 'open');
  const fetchRun = $derived(summary !== null && isFetch(summary.kind));
  const sum = (key: 'fetched' | 'failed' | 'gone' | 'skipped'): number =>
    summary?.perPortal.reduce((total, p) => total + p[key], 0) ?? 0;
  // A fetch counts what it brought (new, not excluded) and how many of those fit well.
  const newJobs = $derived(summary?.newJobs?.count ?? 0);
  const topJobs = $derived(summary?.newJobs?.high ?? 0);
  const skipped = $derived(sum('skipped'));
  const failure = $derived(summary?.outcome.kind === 'failed' ? summary.outcome.error : null);
  const files = $derived(summary ? exportError(summary) : null);
  const filesText = $derived(exportText(files));
  // The old program's Excel file the export renamed before it wrote its own: by its name.
  const renamed = $derived(summary?.export?.backup?.split(/[\\/]/).pop() ?? null);
  // A text file problem is said once: by the export error when it names the text files.
  const txtFailed = $derived(
    files !== null && (files.params.target === 'txt' || files.params.target === 'txtFolder')
      ? 0
      : (summary?.export?.txtFailed ?? 0),
  );
  const pauses = $derived(
    (Object.entries(run.health) as [Portal, PortalHealth][]).filter(([, h]) => h.kind !== 'ok'),
  );
  const title = $derived(summary ? outcomeText(summary) : t.run.done);
  let actionError = $state<string | null>(null);

  function openTarget(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }

  async function copy(): Promise<void> {
    const lines = run.history.map(
      (line) => `${formatTime(new Date(line.at).toISOString())} ${line.text}`,
    );
    actionError = null;
    if (await copyText(lines.join('\n'))) toasts.show(t.toast.copied);
    else actionError = t.run.historyNotCopied;
  }

  function toggle(): void {
    run.panel = open ? 'collapsed' : 'open';
  }

  // A step that finishes while the card is on screen draws its check once; a card that
  // mounts with steps already done shows plain checks.
  const drawn = $state<Partial<Record<Step, true>>>({});
  let states: Partial<Record<Step, string>> = {};
  $effect.pre(() => {
    const now = run.steps.map(
      (step) => [step, run.fetching ? run.stepState(step) : 'idle'] as const,
    );
    untrack(() => {
      for (const [step, state] of now) {
        if (state === 'done' && states[step] === 'current') drawn[step] = true;
        if (state !== 'done') delete drawn[step];
        states[step] = state;
      }
    });
  });

  /** A fitting action for a failed run (a failed fetch: Abrufen right above does the same,
   *  no second button for it). */
  const failureFix = $derived(
    failure === null ? null : failureAction(summary, failure, () => openTarget({ kind: 'logDir' })),
  );
</script>

{#snippet head(text: string, extra: string | null)}
  <div class="head">
    {#key text}<span class="title" in:fade>{text}</span>{/key}
    {#if extra}<span class="pill" data-testid="countdown">{extra}</span>{/if}
    <span class="tools">
      <!-- The close button comes first, so the chevron keeps the right edge in both states. -->
      {#if !run.fetching}
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="x"
          label={t.common.hide}
          testid="run-close"
          onclick={() => run.hide()}
        />
      {/if}
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon="chevron-down"
        turned={open}
        label={open ? t.run.collapse : t.run.expand}
        testid="run-toggle"
        onclick={toggle}
      />
    </span>
  </div>
{/snippet}

<section class="panel" data-testid="run-card" data-kind={run.fetching ? run.kind : summary?.kind}>
  {#if run.fetching}
    <div class="running" data-testid="run-running" in:fade>
      <div class="lead">
        <Spinner size="sm" label={null} />
        {@render head(
          run.status
            ? t.run.statusOf(run.status.code, run.status.portal)
            : t.run.kind[run.kind ?? 'fetch'],
          run.waitLeft !== null ? t.run.resumesIn(run.waitLeft) : null,
        )}
      </div>
      <Meter value={run.fraction} size="sm" label={t.toolbar.progress} />
      {#if open}
        <div class="more" in:fade>
          <ol class="steps">
            {#each run.steps as step (step)}
              {@const state = run.stepState(step)}
              {@const progress = run.progress[step]}
              <li class="step {state}" data-testid="step-{step}">
                <span class="step-head">
                  <span class="mark" class:drawn={drawn[step]}>
                    {#if state === 'done'}
                      <Icon name="circle-check" size="sm" />
                    {:else}
                      <span class="dot" aria-hidden="true"></span>
                    {/if}
                  </span>
                  <span class="name">{t.run.step[step]}</span>
                </span>
                <span class="count">
                  {#if progress && progress.total > 0}
                    {#key progress.done}<span class="value" in:roll={{ up: true }}
                        >{formatNumber(progress.done)}</span
                      >{/key}
                    {t.run.ofTotal(progress.total)}
                  {/if}
                </span>
              </li>
            {/each}
          </ol>
          {#each pauses as [portal, health] (portal)}
            <Notice
              tone={needsAction(health) ? 'warning' : 'info'}
              variant="inline"
              heading={t.portal[portal]}
              text={healthAdvice(health) ?? ''}
              testid="pause-{portal}"
            />
          {/each}
          {#if run.loginNeeded}
            <Notice tone="info" variant="inline" text={t.settings.signInWaiting} />
          {/if}
        </div>
      {/if}
    </div>
  {:else if summary}
    <div class="finished" data-testid="run-finished" in:fade>
      <div
        class="lead"
        class:failed={failure !== null}
        class:warned={failure === null && filesText !== null}
      >
        <Icon name={failure || filesText ? 'triangle-alert' : 'circle-check'} size="sm" />
        {@render head(title, null)}
      </div>
      {#if open}
        <div class="more" in:fade>
          <p class="facts">
            <span class="time">{formatMoment(summary.finishedAt)}</span>
            {#if fetchRun && newJobs > 0}
              <span data-testid="last-new"
                ><Badge label={t.run.newPill(newJobs)} tone="coral" /></span
              >
              {#if app.hasProfile && topJobs > 0}
                <span data-testid="last-top"
                  ><Badge label={t.run.topPill(topJobs)} tone="success" /></span
                >
              {/if}
            {/if}
          </p>
          {#if failure}
            <Notice
              tone="danger"
              variant="inline"
              text={t.error.text(failure.kind, failure.params)}
              action={failureFix}
              testid="run-failed"
            />
          {:else if fetchRun && summary.outcome.kind === 'completed' && newJobs === 0}
            <Notice tone="info" variant="inline" text={t.run.nothingNew} testid="nothing-new" />
          {/if}
          <!-- Ads that did not come warn like their rows (texts.ts DETAIL_WARNS). -->
          {#if summary.kind === 'details' && sum('failed') > 0}
            <Notice
              tone={DETAIL_WARNS.failed ? 'warning' : 'info'}
              variant="inline"
              text={t.run.details.failedAds(sum('failed'))}
              testid="details-failed"
            />
          {/if}
          {#if summary.kind === 'details' && sum('gone') > 0}
            <Notice
              tone={DETAIL_WARNS.gone ? 'warning' : 'info'}
              variant="inline"
              text={t.run.details.goneAds(sum('gone'))}
              testid="details-gone"
            />
          {/if}
          {#if skipped > 0}
            <Notice tone="info" variant="inline" text={t.run.skipped(skipped)} />
          {/if}
          {#if filesText}
            <Notice
              tone="warning"
              variant="inline"
              text={filesText}
              action={failure || run.active
                ? null
                : { label: t.common.retry, icon: 'refresh-cw', onclick: () => run.rewriteFiles() }}
              testid="export-failed"
            />
          {/if}
          {#if txtFailed > 0}
            <Notice tone="warning" variant="inline" text={t.run.filesFailed(txtFailed)} />
          {/if}
          {#if renamed}
            <Notice
              tone="info"
              variant="inline"
              text={t.run.excelRenamed(renamed)}
              action={{
                label: t.common.showInFolder[fileManager()],
                onclick: () => openTarget({ kind: 'excelBackupInFolder', name: renamed }),
              }}
              testid="excel-renamed"
            />
          {/if}
          {#if run.history.length > 0}
            <Disclosure label={t.run.history} testid="run-history">
              <ol class="history" data-copy>
                {#each run.history as line, index (index)}
                  <li>
                    <span class="stamp">{formatTime(new Date(line.at).toISOString())}</span>
                    {line.text}
                  </li>
                {/each}
              </ol>
              <span class="copy">
                <Button
                  variant="ghost"
                  size="sm"
                  icon="copy"
                  label={t.common.copy}
                  testid="history-copy"
                  onclick={copy}
                />
              </span>
            </Disclosure>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
  {#if run.startError}
    <Notice tone="danger" variant="inline" text={run.startError} testid="start-error" />
  {/if}
  {#if actionError}
    <Notice tone="danger" variant="inline" text={actionError} />
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    padding: var(--space-12) var(--pane-padding) var(--pane-padding);
    border-bottom: var(--border-width) solid var(--border);
  }

  .running,
  .finished,
  .more {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }

  .lead {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    color: var(--success-strong);
  }

  .running .lead {
    color: var(--meter-fill);
  }

  .lead.failed {
    color: var(--danger-strong);
  }

  .lead.warned {
    color: var(--warning-strong);
  }

  .head {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
  }

  .title {
    overflow: hidden;
    color: var(--text-heading);
    font: var(--type-md);
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The countdown of a pause: a soft navy pill; tabular digits, so the ticking stays still. */
  .pill {
    display: inline-flex;
    flex: none;
    align-items: center;
    height: var(--badge-height);
    padding: 0 var(--space-8);
    border-radius: var(--radius-full);
    background-color: var(--active-surface);
    color: var(--active-text);
    font: var(--type-xs);
    font-weight: var(--weight-medium);
    font-variant-numeric: var(--numeric);
    white-space: nowrap;
  }

  .tools {
    display: flex;
    margin-left: auto;
  }

  /* The steps side by side, as many equal columns as the kind has steps: marker and name,
     below them the counter. */
  .steps {
    display: grid;
    grid-auto-columns: minmax(0, 1fr);
    grid-auto-flow: column;
    gap: var(--space-8);
  }

  .step {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .step-head {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    min-width: 0;
  }

  .step.current,
  .step.done {
    color: var(--text);
  }

  .step.current .name {
    font-weight: var(--weight-medium);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* 16 px markers: a check when done, a filled navy dot while current, a small grey dot
     ahead. No outlined circles: next to each other they read as radio buttons. */
  .mark {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--icon-sm);
    height: var(--icon-sm);
    color: var(--text-subtle);
  }

  .dot {
    width: var(--dot);
    height: var(--dot);
    border-radius: var(--radius-full);
    background-color: var(--border-strong);
  }

  .current .dot {
    background-color: var(--meter-fill);
  }

  .done .mark {
    color: var(--success-strong);
  }

  .current .mark {
    color: var(--meter-fill);
  }

  /* The check of a step that just finished draws itself once (180 ms). */
  .drawn :global(svg path) {
    stroke-dasharray: var(--draw-length);
    animation: draw var(--dur-slow) var(--ease-out) both;
  }

  .count {
    min-height: var(--leading-xs);
    padding-left: calc(var(--icon-sm) + var(--space-6));
    color: var(--text-subtle);
    font: var(--type-xs);
    font-variant-numeric: var(--numeric);
  }

  .value {
    display: inline-block;
  }

  .facts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-6);
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .facts > * + * {
    display: inline-flex;
  }

  .time {
    margin-right: var(--space-2);
  }

  /* At most 16 lines, then it scrolls. */
  .history {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-height: calc(16 * (var(--leading-xs) + var(--space-4)));
    margin-bottom: var(--space-8);
    overflow: auto;
    color: var(--text-muted);
    font: var(--type-xs);
  }

  /* A quiet button: its icon starts on the edge of the card, like the lines above (its
     padding and border hang out). */
  .copy {
    display: flex;
    align-self: flex-start;
    margin-left: calc(-1 * var(--ghost-inset));
  }

  /* The space after the time is a real one, so a selection copies like "Kopieren". */
  .stamp {
    margin-right: var(--space-4);
    font-variant-numeric: var(--numeric);
  }
</style>
