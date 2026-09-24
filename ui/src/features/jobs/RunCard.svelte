<!--
  The run card above the list, shown only while a run is going or right after it (or when
  the run status in the sidebar is clicked); it collapses to its header line and closes.
  running: the steps Postfach, Details, Bewertung with the brand meter, one line per portal,
  the countdown of a pause and every limit or pause with its reason and end.
  finished: how many new and well-fitting jobs, what went wrong with a fitting action, the
  overview and the folder, the history with copy.
-->
<script lang="ts">
  import Badge from '$components/Badge.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Disclosure from '$components/Disclosure.svelte';
  import Icon from '$components/Icon.svelte';
  import { PORTAL_MONOGRAM } from '$components/IconTile.svelte';
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
  import { run, STEPS } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';

  const summary = $derived(run.summary ?? app.state?.lastRun ?? null);
  const open = $derived(run.panel === 'open');
  const newJobs = $derived(summary?.perPortal.reduce((sum, p) => sum + p.new, 0) ?? 0);
  const topJobs = $derived(
    (app.state?.topMatches ?? []).filter((job) => job.match?.band === 'high').length,
  );
  const skipped = $derived(summary?.perPortal.reduce((sum, p) => sum + p.skipped, 0) ?? 0);
  const failure = $derived(summary?.outcome.kind === 'failed' ? summary.outcome.error : null);
  const portalLines = $derived(
    (Object.keys(run.portals) as Portal[]).map((portal) => ({
      portal,
      progress: run.portals[portal]!,
      health: run.health[portal] ?? null,
    })),
  );
  const pauses = $derived(
    (Object.entries(run.health) as [Portal, PortalHealth][]).filter(([, h]) => h.kind !== 'ok'),
  );
  const title = $derived(
    summary?.outcome.kind === 'failed'
      ? de.run.failed
      : summary?.outcome.kind === 'cancelled'
        ? de.run.cancelled
        : de.run.done,
  );
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

<Card padding="md" testid="run-card">
  {#if run.active}
    <div class="running" data-testid="run-running">
      <div class="lead">
        <Spinner size="sm" label={null} />
        {@render head(
          run.status ? de.run.status[run.status.code] : de.run.kind[run.kind ?? 'fetch'],
          run.waitLeft !== null ? de.run.resumesIn(run.waitLeft) : null,
        )}
      </div>
      {#if open}
        <ol class="steps">
          {#each STEPS as step (step)}
            {@const state = run.stepState(step)}
            {@const progress = run.progress[step]}
            <li class="step {state}" data-testid="step-{step}">
              <span class="mark">
                {#if state === 'done'}<Icon name="check" size="sm" />{:else}<span class="dot"
                  ></span>{/if}
              </span>
              <span class="name">{de.run.step[step]}</span>
              {#if progress && progress.total > 0}
                <span class="count">{de.run.of(progress.done, progress.total)}</span>
              {/if}
              {#if state === 'current'}
                <span class="meter">
                  <Meter
                    value={progress && progress.total > 0 ? progress.done / progress.total : null}
                    size="md"
                    label={de.run.step[step]}
                  />
                </span>
              {/if}
            </li>
          {/each}
        </ol>
        {#if portalLines.length > 0}
          <ul class="portals">
            {#each portalLines as line (line.portal)}
              <li class="portal" data-testid="run-portal-{line.portal}">
                <span class="mono" aria-hidden="true">{PORTAL_MONOGRAM[line.portal]}</span>
                <span class="name">{de.portal[line.portal]}</span>
                {#if line.health && line.health.kind !== 'ok'}
                  <Badge label={healthText(line.health).label} tone="warning" />
                {/if}
                <span class="count">{de.run.of(line.progress.done, line.progress.total)}</span>
              </li>
            {/each}
          </ul>
        {/if}
        {#each pauses as [portal, health] (portal)}
          <Notice
            tone="warning"
            variant="row"
            heading={de.portal[portal]}
            text={healthText(health).text ?? ''}
            testid="pause-{portal}"
          />
        {/each}
        {#if run.loginNeeded}
          <Notice tone="info" variant="row" text={de.settings.signInWaiting} />
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
        <p class="numbers">
          <span data-testid="last-new">{de.run.newCount(newJobs)}</span>
          {#if app.hasProfile && topJobs > 0}
            <span class="sep">·</span><span class="top">{de.run.topCount(topJobs)}</span>
          {/if}
        </p>
        {#if failure}
          <Notice
            tone="danger"
            variant="row"
            text={de.error.text(failure.kind, failure.params)}
            action={failureAction}
            testid="run-failed"
          />
        {:else if summary.outcome.kind === 'completed' && newJobs === 0}
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
        <div class="actions">
          <Button
            variant="secondary"
            size="sm"
            icon="external-link"
            label={de.run.openOverview}
            testid="open-overview"
            onclick={() => openTarget({ kind: 'overview' })}
          />
          <Button
            variant="ghost"
            size="sm"
            icon="folder-open"
            label={de.common.openFolder}
            onclick={() => openTarget({ kind: 'workspace' })}
          />
        </div>
        {#if run.history.length > 0}
          <Disclosure label={de.run.history} testid="run-history">
            <ol class="history">
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
</Card>

<style>
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
    font-weight: var(--weight-semibold);
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

  .steps,
  .portals {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .step {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    column-gap: var(--space-8);
    row-gap: var(--space-6);
    color: var(--text-subtle);
    font: var(--type-sm);
  }

  .step.current,
  .step.done {
    color: var(--text);
  }

  .step.current .name {
    font-weight: var(--weight-semibold);
  }

  .mark {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--icon-sm);
    height: var(--icon-sm);
    color: var(--success-strong);
  }

  .dot {
    width: var(--dot);
    height: var(--dot);
    border: var(--border-width) solid var(--border-strong);
    border-radius: var(--radius-full);
  }

  .current .dot {
    border-color: var(--accent);
    background-color: var(--accent);
  }

  .count {
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .meter {
    grid-column: 1 / -1;
  }

  .portal {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    font: var(--type-sm);
  }

  .portal .count {
    margin-left: auto;
  }

  .mono {
    padding: 0 var(--space-4);
    border-radius: var(--radius-xs);
    background-color: var(--surface-muted);
    color: var(--text-muted);
    font: var(--type-xs);
    font-weight: var(--weight-bold);
  }

  .numbers {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .top {
    color: var(--score-high-text);
    font-weight: var(--weight-medium);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-8);
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
