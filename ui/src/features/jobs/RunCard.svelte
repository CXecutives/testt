<!--
  The run panel on top of the list (flat on the sheet, a hairline below), shown only while a
  run is going or right after it (or when the run status in the sidebar is clicked); it
  collapses to its header line (the chevron turns, the rest fades in when it opens and is
  gone at once when it closes) and closes.
  running: the header line (the spinner, the status naming the portal it is about, which
  cross-fades when it changes, and the countdown of a pause as a soft navy pill), the navy
  progress bar right below it, the steps Postfach, Details, Bewertung side by side (a navy
  dot for the current one, a check that draws itself when a step finishes while the card is
  on screen, the counters roll), then every limit or pause with its reason and end.
  finished: the outcome and its time (the header cross-fades from the running one), the
  pills "n neu" and "n passen gut" (nothing when there are none: the note says it), what
  went wrong with a fitting action, the history with copy. The overview file and the folder
  have their one place in the day overview. A finished rescore only says so.
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
  import { de } from '$lib/i18n/de';
  import { formatMoment, formatNumber, formatTime } from '$lib/i18n/format';
  import { errorText, healthText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { OpenTarget, Portal, PortalHealth, Step } from '$lib/ipc/types';
  import { fade, roll } from '$lib/motion/transitions';
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

  // A step that finishes while the card is on screen draws its check once; a card that
  // mounts with steps already done shows plain checks.
  const drawn = $state<Partial<Record<Step, true>>>({});
  let states: Partial<Record<Step, string>> = {};
  $effect.pre(() => {
    const now = STEPS.map((step) => [step, run.active ? run.stepState(step) : 'idle'] as const);
    untrack(() => {
      for (const [step, state] of now) {
        if (state === 'done' && states[step] === 'current') drawn[step] = true;
        if (state !== 'done') delete drawn[step];
        states[step] = state;
      }
    });
  });

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
    {#key text}<span class="title" in:fade>{text}</span>{/key}
    {#if extra}<span class="pill" data-testid="countdown">{extra}</span>{/if}
    <span class="tools">
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon="chevron-down"
        turned={open}
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
    <div class="running" data-testid="run-running" in:fade>
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
        <div class="more" in:fade>
          <ol class="steps">
            {#each STEPS as step (step)}
              {@const state = run.stepState(step)}
              {@const progress = run.progress[step]}
              <li class="step {state}" data-testid="step-{step}">
                <span class="step-head">
                  <span class="mark" class:drawn={drawn[step]}>
                    <Icon
                      name={state === 'done'
                        ? 'circle-check'
                        : state === 'current'
                          ? 'circle-dot'
                          : 'circle'}
                      size="sm"
                    />
                  </span>
                  <span class="name">{de.run.step[step]}</span>
                </span>
                <span class="count">
                  {#if progress && progress.total > 0}
                    {#key progress.done}<span class="value" in:roll={{ up: true }}
                        >{formatNumber(progress.done)}</span
                      >{/key}
                    {de.run.ofTotal(progress.total)}
                  {/if}
                </span>
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
        </div>
      {/if}
    </div>
  {:else if summary}
    <div class="finished" data-testid="run-finished" in:fade>
      <div class="lead" class:failed={failure !== null}>
        <Icon name={failure ? 'triangle-alert' : 'circle-check'} size="sm" />
        {@render head(title, null)}
      </div>
      {#if open}
        <div class="more" in:fade>
          <p class="facts">
            <span class="time">{formatMoment(summary.finishedAt)}</span>
            {#if !rescore && newJobs > 0}
              <span data-testid="last-new"
                ><Badge label={de.run.newPill(newJobs)} tone="navy" /></span
              >
              {#if app.hasProfile && topJobs > 0}
                <Badge label={de.run.topPill(topJobs)} tone="success" />
              {/if}
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
                    <span class="stamp">{formatTime(new Date(line.at).toISOString())}</span>
                    {line.text}
                  </li>
                {/each}
              </ol>
              <Button variant="ghost" size="sm" icon="copy" label={de.common.copy} onclick={copy} />
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
    background-color: var(--count-soft-bg);
    color: var(--count-soft-fg);
    font: var(--type-xs);
    font-weight: var(--weight-medium);
    font-variant-numeric: var(--numeric);
    white-space: nowrap;
  }

  .tools {
    display: flex;
    margin-left: auto;
  }

  /* The steps side by side: marker and name, below them the counter. */
  .steps {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
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

  /* 16 px markers: a check when done, the navy dot while current, an empty circle ahead. */
  .mark {
    display: flex;
    flex: none;
    color: var(--text-subtle);
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
  .stamp {
    margin-right: var(--space-4);
    font-variant-numeric: var(--numeric);
  }
</style>
