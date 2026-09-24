<!--
  The run panel on top of the list (flat on the sheet, a hairline below), shown only while a
  run is going or right after it (or when the run status in the sidebar is clicked); it
  collapses to its header line and closes.
  running: the header (the status, naming the portal it is about, and the countdown of a
  pause) and the progress bar right below it, the steps Postfach, Details, Bewertung (16 px
  check / loader / circle) and every limit or pause with its reason and end.
  finished: how many new and well-fitting jobs (nothing when there are none: the note says
  it), what went wrong with a fitting action, the history with copy. The overview file and
  the folder have their one place in the day overview. A finished rescore only says so.
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
  import { outcomeText, run, STEPS } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';

  const summary = $derived(run.summary ?? app.state?.lastRun ?? null);
  const open = $derived(run.panel === 'open');
  const newJobs = $derived(summary?.perPortal.reduce((sum, p) => sum + p.new, 0) ?? 0);
  const topJobs = $derived(
    (app.state?.topMatches ?? []).filter((job) => job.match?.band === 'high').length,
  );
  const skipped = $derived(summary?.perPortal.reduce((sum, p) => sum + p.skipped, 0) ?? 0);
  const failure = $derived(summary?.outcome.kind === 'failed' ? summary.outcome.error : null);
  const rescore = $derived(summary?.kind === 'rescore');
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
        return { label: de.common.retry, onclick: () => void run.start({ kind: 'fetch' }) };
    }
  });
</script>

{#snippet head(text: string, extra: string | null)}
  <div class="head">
    <span class="title">{text}</span>
    {#if extra}<span class="extra" data-testid="countdown">{extra}</span>{/if}
    <span class="tools">
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon={open ? 'chevron-up' : 'chevron-down'}
        label={open ? de.run.collapse : de.run.expand}
        testid="run-toggle"
        onclick={toggle}
      />
      {#if !run.active}
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="x"
          label={de.common.hide}
          testid="run-close"
          onclick={() => (run.panel = 'hidden')}
        />
      {/if}
    </span>
  </div>
{/snippet}

<section class="panel" data-testid="run-card">
  {#if run.active}
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
          {#each STEPS as step (step)}
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
      <div class="lead" class:failed={failure !== null}>
        <Icon name={failure ? 'triangle-alert' : 'circle-check'} size="sm" />
        {@render head(title, formatMoment(summary.finishedAt))}
      </div>
      {#if open}
        {#if !rescore && newJobs > 0}
          <p class="numbers">
            <span data-testid="last-new">{de.run.newCount(newJobs)}</span>
            {#if app.hasProfile && topJobs > 0}
              <span class="sep">·</span><span class="top">{de.run.topCount(topJobs)}</span>
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
        {:else if summary.outcome.kind === 'completed' && newJobs === 0 && !rescore}
          <Notice tone="info" variant="inline" text={de.run.nothingNew} testid="nothing-new" />
        {/if}
        {#if skipped > 0}
          <Notice tone="info" variant="inline" text={de.run.skipped(skipped)} />
        {/if}
        {#if (summary.export?.txtFailed ?? 0) > 0}
          <Notice
            tone="warning"
            variant="inline"
            text={de.run.filesFailed(summary.export?.txtFailed ?? 0)}
          />
        {/if}
        {#if run.history.length > 0}
          <Disclosure label={de.run.history} testid="run-history">
            <ol class="history" data-copy>
              {#each run.history as line, index (index)}
                <li>
                  <span class="time">{formatTime(new Date(line.at).toISOString())}</span>
                  {line.text}
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

  /* The space after the time is a real one, so a selection copies like "Kopieren". */
  .time {
    margin-right: var(--space-4);
    font-variant-numeric: var(--numeric);
  }
</style>
