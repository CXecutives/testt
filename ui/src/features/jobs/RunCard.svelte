<!--
  The run card above the list.
  idle/finished: when the last fetch was, how many new jobs and how many fit well, the
  overview and the folder, the history of this session's run.
  running: the steps Postfach, Details, Bewertung with the brand meter, one line per portal,
  the countdown of a pause and every limit or pause with its reason and end.
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
  import { formatRelative, formatTime } from '$lib/i18n/format';
  import { errorText, healthText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { OpenTarget, Portal, PortalHealth } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run, STEPS } from '$lib/state/run.svelte';
  import { tokenMs } from '$lib/tokens';

  const summary = $derived(run.summary ?? app.state?.lastRun ?? null);
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
  let copied = $state(false);
  let actionError = $state<string | null>(null);

  function open(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }

  async function copy(): Promise<void> {
    const text = run.history.map(
      (line) => `${formatTime(new Date(line.at).toISOString())} ${line.text}`,
    );
    try {
      await navigator.clipboard.writeText(text.join('\n'));
      copied = true;
      setTimeout(() => (copied = false), tokenMs('--dur-loop'));
    } catch (error) {
      actionError = errorText(error);
    }
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
        return { label: de.common.openLog, onclick: () => open({ kind: 'logDir' }) };
      default:
        return { label: de.common.retry, onclick: () => void run.start({ kind: 'fetch' }) };
    }
  });
</script>

<Card padding="md" testid="run-card">
  {#if run.active}
    <div class="running" data-testid="run-running">
      <p class="status">
        <Spinner size="sm" label={null} />
        <span>{run.status ? de.run.status[run.status.code] : de.run.kind[run.kind ?? 'fetch']}</span
        >
        {#if run.waitLeft !== null}
          <span class="countdown" data-testid="countdown">{de.run.resumesIn(run.waitLeft)}</span>
        {/if}
      </p>
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
          heading={de.portal[portal]}
          text={healthText(health).text ?? ''}
          testid="pause-{portal}"
        />
      {/each}
      {#if run.loginNeeded}
        <Notice tone="info" text={de.settings.signInWaiting} />
      {/if}
    </div>
  {:else}
    <div class="idle" data-testid="run-idle">
      <div class="facts">
        <span class="label">{de.run.lastFetch}</span>
        <span class="value" data-testid="last-fetch">
          {summary ? formatRelative(summary.finishedAt) : de.run.never}
        </span>
        {#if summary}
          <span class="numbers">
            <span data-testid="last-new">{de.run.newCount(newJobs)}</span>
            {#if app.hasProfile && topJobs > 0}
              <span class="sep">·</span><span class="top">{de.run.topCount(topJobs)}</span>
            {/if}
          </span>
        {/if}
      </div>
      {#if run.startError}
        <Notice tone="danger" variant="inline" text={run.startError} testid="start-error" />
      {:else if failure}
        <Notice
          tone="danger"
          text={de.error.text(failure.kind, failure.params)}
          action={failureAction}
          testid="run-failed"
        />
      {:else if summary?.outcome.kind === 'cancelled'}
        <Notice tone="info" variant="inline" text={de.run.cancelled} />
      {:else if run.summary && newJobs === 0}
        <Notice tone="info" variant="inline" text={de.run.nothingNew} testid="nothing-new" />
      {/if}
      {#if skipped > 0}
        <Notice tone="info" variant="inline" text={de.run.skipped(skipped)} />
      {/if}
      {#if summary && (summary.export?.txtFailed ?? 0) > 0}
        <Notice
          tone="warning"
          variant="inline"
          text={de.run.filesFailed(summary.export?.txtFailed ?? 0)}
        />
      {/if}
      {#if summary}
        <div class="actions">
          <Button
            variant="secondary"
            size="sm"
            icon="external-link"
            label={de.run.openOverview}
            testid="open-overview"
            onclick={() => open({ kind: 'overview' })}
          />
          <Button
            variant="ghost"
            size="sm"
            icon="folder-open"
            label={de.common.openFolder}
            onclick={() => open({ kind: 'workspace' })}
          />
        </div>
      {/if}
      {#if actionError}
        <Notice tone="danger" variant="inline" text={actionError} />
      {/if}
      {#if run.history.length > 0}
        <Disclosure label={de.run.history} testid="run-history">
          <ol class="history">
            {#each run.history as line, index (index)}
              <li>
                <span class="time">{formatTime(new Date(line.at).toISOString())}</span>{line.text}
              </li>
            {/each}
          </ol>
          <Button
            variant="ghost"
            size="sm"
            icon={copied ? 'check' : 'copy'}
            label={copied ? de.common.copied : de.common.copy}
            onclick={() => void copy()}
          />
        </Disclosure>
      {/if}
    </div>
  {/if}
</Card>

<style>
  .running,
  .idle {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }

  .status {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    color: var(--text-heading);
    font: var(--type-md);
    font-weight: var(--weight-semibold);
  }

  .countdown {
    margin-left: auto;
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
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

  .facts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .label {
    color: var(--text-muted);
    font: var(--type-xs);
  }

  .value {
    color: var(--text-heading);
    font: var(--type-lg);
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
