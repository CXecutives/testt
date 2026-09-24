<!--
  The run panel on top of the list (flat on the sheet, a hairline below), shown only while a
  run is going or right after it (or when the run status in the sidebar is clicked); it
  collapses to its header line and closes.
  running: the header (the status, naming the portal it is about, and the countdown of a
  pause) and the progress bar right below it, the steps of the kind (a fetch: Postfach,
  Details, Bewertung; a details run: Details, Bewertung) with 16 px check / loader / circle,
  and every limit or pause with its reason and end.
  finished: a fetch says how many new and well-fitting jobs it brought (the run's own
  numbers from the backend), a details run what it got, a rescore only that it is done;
  then what went wrong with a fitting action, a file the export could not write (once),
  the overview and the folder, the history with copy. A rescore shows here only when it
  failed or could not write the files.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Disclosure from '$components/Disclosure.svelte';
  import Icon from '$components/Icon.svelte';
  import Meter from '$components/Meter.svelte';
  import Notice from '$components/Notice.svelte';
  import Spinner from '$components/Spinner.svelte';
  import { de } from '$lib/i18n/de';
  import { formatMoment, formatTime } from '$lib/i18n/format';
  import { errorText, healthText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { OpenTarget, Portal, PortalHealth } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { exportError, isFetch, outcomeText, run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';

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
  const filesText = $derived.by(() => {
    if (files === null) return null;
    const texts = de.run.exportFailed;
    switch (files.params.target) {
      case 'overview':
        return files.kind === 'fileLocked' ? texts.overviewLocked : texts.overview;
      case 'overviewHtml':
        return texts.overviewHtml;
      case 'txtFolder':
        return texts.txtFolder;
      case 'backup':
        return texts.backup;
      default:
        return texts.txt;
    }
  });
  // A text file problem is said once: by the export error when it names the text files.
  const txtFailed = $derived(
    files !== null && (files.params.target === 'txt' || files.params.target === 'txtFolder')
      ? 0
      : (summary?.export?.txtFailed ?? 0),
  );
  const pauses = $derived(
    (Object.entries(run.health) as [Portal, PortalHealth][]).filter(([, h]) => h.kind !== 'ok'),
  );
  const title = $derived(summary ? outcomeText(summary) : de.run.done);
  let actionError = $state<string | null>(null);

  function openTarget(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }

  async function copy(): Promise<void> {
    const lines = run.history.map(
      (line) => `${formatTime(new Date(line.at).toISOString())} ${line.text}`,
    );
    try {
      await navigator.clipboard.writeText(lines.join('\n'));
      toasts.show(de.toast.copied);
    } catch (error) {
      actionError = errorText(error);
    }
  }

  function toggle(): void {
    run.panel = open ? 'collapsed' : 'open';
  }

  /** A fitting action for a failed run. */
  const failureAction = $derived.by(() => {
    if (failure === null) return null;
    switch (failure.kind) {
      case 'mailAuth':
      case 'mailMissing':
      case 'mailNotGmail':
      case 'secretCorrupt':
      case 'secretStore':
        return { label: de.run.checkMailbox, onclick: () => navigation.go('settings') };
      case 'internal':
        return { label: de.common.openLog, onclick: () => openTarget({ kind: 'logDir' }) };
      default:
        return { label: de.common.retry, onclick: () => run.retry(summary) };
    }
  });
</script>

{#snippet head(text: string, extra: string | null)}
  <div class="head">
    <span class="title">{text}</span>
    {#if extra}<span class="extra" data-testid="countdown">{extra}</span>{/if}
    <span class="tools">
      {#if !run.fetching}
        <!-- System actions stay quiet: icons with tooltips, like in the day overview. -->
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="external-link"
          label={de.run.openOverview}
          testid="open-overview"
          onclick={() => openTarget({ kind: 'overview' })}
        />
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="folder-open"
          label={de.common.openFolder}
          testid="open-folder"
          onclick={() => openTarget({ kind: 'workspace' })}
        />
      {/if}
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon={open ? 'chevron-up' : 'chevron-down'}
        label={open ? de.run.collapse : de.run.expand}
        testid="run-toggle"
        onclick={toggle}
      />
      {#if !run.fetching}
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="x"
          label={de.common.hide}
          testid="run-close"
          onclick={() => run.hide()}
        />
      {/if}
    </span>
  </div>
{/snippet}

<section class="panel" data-testid="run-card" data-kind={run.fetching ? run.kind : summary?.kind}>
  {#if run.fetching}
    <div class="running" data-testid="run-running">
      <div class="lead">
        <Spinner size="sm" label={null} />
        {@render head(
          run.status
            ? de.run.statusOf(run.status.code, run.status.portal)
            : de.run.kind[run.kind ?? 'fetch'],
          run.waitLeft !== null ? de.run.resumesIn(run.waitLeft) : null,
        )}
      </div>
      <Meter value={run.fraction} size="sm" label={de.toolbar.progress} />
      {#if open}
        <ol class="steps">
          {#each run.steps as step (step)}
            {@const state = run.stepState(step)}
            {@const progress = run.progress[step]}
            <li class="step {state}" data-testid="step-{step}">
              <span class="mark">
                <Icon
                  name={state === 'done'
                    ? 'check'
                    : state === 'current'
                      ? 'loader-circle'
                      : 'circle'}
                  size="sm"
                />
              </span>
              <span class="name">{de.run.step[step]}</span>
              {#if progress && progress.total > 0}
                <span class="count">{de.run.of(progress.done, progress.total)}</span>
              {/if}
            </li>
          {/each}
        </ol>
        {#each pauses as [portal, health] (portal)}
          <Notice
            tone="warning"
            variant="inline"
            heading={de.portal[portal]}
            text={healthText(health).text ?? ''}
            testid="pause-{portal}"
          />
        {/each}
        {#if run.loginNeeded}
          <Notice tone="info" variant="inline" text={de.settings.signInWaiting} />
        {/if}
      {/if}
    </div>
  {:else if summary}
    <div class="finished" data-testid="run-finished">
      <div
        class="lead"
        class:failed={failure !== null}
        class:warned={failure === null && filesText !== null}
      >
        <Icon name={failure || filesText ? 'triangle-alert' : 'circle-check'} size="sm" />
        {@render head(title, formatMoment(summary.finishedAt))}
      </div>
      {#if open}
        {#if fetchRun && newJobs > 0}
          <p class="numbers">
            <span data-testid="last-new">{de.run.newCount(newJobs)}</span>
            {#if app.hasProfile && topJobs > 0}
              <span class="sep">·</span><span class="top" data-testid="last-top"
                >{de.run.topCount(topJobs)}</span
              >
            {/if}
          </p>
        {/if}
        {#if failure}
          <Notice
            tone="danger"
            variant="row"
            text={de.error.text(failure.kind, failure.params)}
            action={failureAction}
            testid="run-failed"
          />
        {:else if fetchRun && summary.outcome.kind === 'completed' && newJobs === 0}
          <Notice tone="info" variant="inline" text={de.run.nothingNew} testid="nothing-new" />
        {/if}
        {#if summary.kind === 'details' && sum('failed') > 0}
          <Notice
            tone="warning"
            variant="inline"
            text={de.run.details.failedAds(sum('failed'))}
            testid="details-failed"
          />
        {/if}
        {#if summary.kind === 'details' && sum('gone') > 0}
          <Notice
            tone="info"
            variant="inline"
            text={de.run.details.goneAds(sum('gone'))}
            testid="details-gone"
          />
        {/if}
        {#if skipped > 0}
          <Notice tone="info" variant="inline" text={de.run.skipped(skipped)} />
        {/if}
        {#if filesText}
          <Notice
            tone="warning"
            variant="row"
            text={filesText}
            action={failure || run.active
              ? null
              : { label: de.common.retry, onclick: () => run.retry(summary) }}
            testid="export-failed"
          />
        {/if}
        {#if txtFailed > 0}
          <Notice tone="warning" variant="inline" text={de.run.filesFailed(txtFailed)} />
        {/if}
        {#if run.history.length > 0}
          <Disclosure label={de.run.history} testid="run-history">
            <ol class="history" data-copy>
              {#each run.history as line, index (index)}
                <li>
                  <span class="time">{formatTime(new Date(line.at).toISOString())}</span>{line.text}
                </li>
              {/each}
            </ol>
            <Button variant="ghost" size="sm" icon="copy" label={de.common.copy} onclick={copy} />
          </Disclosure>
        {/if}
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
  .finished {
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
    color: var(--text-heading);
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

  .extra {
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
    white-space: nowrap;
  }

  .tools {
    display: flex;
    margin-left: auto;
  }

  .steps {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .step {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    column-gap: var(--space-8);
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .step.current,
  .step.done {
    color: var(--text);
  }

  .step.current .name {
    font-weight: var(--weight-medium);
  }

  /* 16 px markers: check when done, loader while current, an empty circle ahead. */
  .mark {
    display: flex;
    color: var(--text-subtle);
  }

  .done .mark {
    color: var(--success-strong);
  }

  .current .mark {
    color: var(--text);
  }

  .count {
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .numbers {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .top {
    color: var(--score-high-text);
    font-weight: var(--weight-medium);
  }

  .history {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-height: var(--list-min);
    margin-bottom: var(--space-8);
    overflow: auto;
    color: var(--text-muted);
    font: var(--type-xs);
  }

  .time {
    margin-right: var(--space-8);
    font-variant-numeric: var(--numeric);
  }
</style>
