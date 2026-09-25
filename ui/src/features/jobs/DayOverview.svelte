<!--
  The reader's empty state: what the sheet shows while no job is selected, unboxed like the
  reader. It answers "what now" in a short list, no counts (the list header counts): "Neu
  und passend" with the prompt of the best matches for any AI chat at the end of its heading
  (the three best scored new jobs as list rows with the list's tools, a click opens the
  job; only where the list beside does not show them on top already, else one quiet line;
  with no new match the prompt stands on its own line, as long as there is something to
  compare), the open points (one per enabled portal and problem, a failed fetch, each with
  its fitting action like the run card's; a warning only where she has to act, else a calm
  note in the words of Einstellungen) only when there are any, and at the end the overview
  file, the Excel file and the folder, their one place in the Jobs view (a file that is not
  there yet cannot be opened and says why). "Nothing new" is said by the list and the run
  card, not here. The time of the last fetch is said once, in the sidebar. The portals keep
  the one order of the app (the settings').
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Button from '$components/Button.svelte';
  import JobRow, { type RowTool } from '$components/JobRow.svelte';
  import Notice from '$components/Notice.svelte';
  import { tooltip } from '$lib/actions/tooltip';
  import { t } from '$lib/i18n/t';
  import { errorText, healthAdvice } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { EmptyAlert, JobView, OpenTarget, Portal, PortalState } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { jobs, keyOf, sameKey } from '$lib/state/jobs.svelte';
  import { failureAction, run } from '$lib/state/run.svelte';
  import { actionsOf, guarded, move, toggleStar } from './actions';
  import { copyTopPrompt } from './prompt';

  // The one order of the portals (the backend's, as in the settings); a portal switched off
  // has no open points.
  const portals = $derived(
    (app.state?.portals ?? []).filter((p) => p.enabled).map((p) => p.portal),
  );
  const BEST = 3;
  /** The best scored new jobs (one small query; again whenever the counts move). */
  let top = $state.raw<JobView[]>([]);
  let topRequest = 0;
  /** The best matches did not load: a quiet retry in their place. */
  let topError = $state<string | null>(null);

  function loadTop(): void {
    const request = ++topRequest;
    if (!app.hasProfile) {
      top = [];
      topError = null;
      return;
    }
    invoke('list_jobs', {
      query: {
        place: 'inbox',
        unread: true,
        favourites: false,
        sort: 'match',
        search: null,
        limit: BEST,
        offset: 0,
      },
    })
      .then((page) => {
        if (request !== topRequest) return;
        top = page.jobs;
        topError = null;
      })
      .catch((error: unknown) => {
        if (request === topRequest) topError = errorText(error);
      });
  }

  $effect(() => {
    void jobs.overviewCounts;
    void app.hasProfile;
    untrack(loadTop);
  });
  // As the list knows them now (read, pinned); only scored ones are a match.
  const best = $derived(
    top
      .map((job) => jobs.rows.find((row) => sameKey(row.key, job.key)) ?? job)
      .filter((job) => job.match?.status === 'scored'),
  );

  // While a run goes, the run card shows pauses and limits; they are not repeated here.
  const troubled = $derived(
    run.fetching
      ? []
      : (app.state?.portals ?? []).filter((p) => p.enabled && p.health.kind !== 'ok'),
  );
  const emptyAlerts = $derived(app.state?.lastRun?.emptyAlerts ?? []);

  interface Issue {
    id: string;
    portal: Portal;
    text: string;
    /** An alert mail to open in Gmail. */
    mail: string | null;
    /** She has to act (a warning); else it resolves itself (a calm note, as in Einstellungen). */
    act: boolean;
  }

  /**
   * Each problem of a portal once: alert mails without jobs (the portal's "layout suspect"
   * health and the empty alerts of the last fetch are one thing) with "Alert-Mail öffnen", and
   * a pause, a limit or a sign-in as its own line, in the words of the settings.
   */
  function issuesOf(portal: Portal, state: PortalState | undefined, alerts: EmptyAlert[]): Issue[] {
    const out: Issue[] = [];
    const health = state?.health ?? null;
    const suspect = health?.kind === 'layoutSuspect' ? health : null;
    const mails = Math.max(alerts.length, suspect?.emptyMails ?? 0);
    if (mails > 0) {
      out.push({
        id: `${portal}-mails`,
        portal,
        text: t.overview.emptyAlerts(mails),
        mail: alerts.find((a) => a.gmailId !== null)?.gmailId ?? null,
        act: true,
      });
    } else if (suspect !== null) {
      out.push({
        id: `${portal}-pages`,
        portal,
        text: healthAdvice(suspect) ?? '',
        mail: null,
        act: false,
      });
    }
    if (health !== null && health.kind !== 'ok' && suspect === null) {
      out.push({
        id: `${portal}-health`,
        portal,
        text: healthAdvice(health) ?? '',
        mail: null,
        act: state?.actionNeeded ?? false,
      });
    }
    return out;
  }

  const portalIssues = $derived(
    portals.flatMap((portal) =>
      issuesOf(
        portal,
        troubled.find((p) => p.portal === portal),
        emptyAlerts.filter((a) => a.portal === portal),
      ),
    ),
  );
  // The last fetch failed; while the run card is up it speaks, not this.
  const lastFailure = $derived.by(() => {
    const previous = app.state?.lastRun ?? null;
    if (run.fetching || run.panel !== 'hidden' || previous?.outcome.kind !== 'failed') {
      return null;
    }
    return previous.outcome.error;
  });
  const hasIssues = $derived(portalIssues.length > 0 || lastFailure !== null);
  const fetchedOnce = $derived((run.summary ?? app.state?.lastRun ?? null) !== null);
  /** The list beside shows the best new jobs on top already (Neu, by fit, no search). */
  const listShowsBest = $derived(
    jobs.facet === 'new' && jobs.sortChoice === 'match' && jobs.search.trim() === '',
  );
  /** Nothing else to say while the list beside holds jobs: a quiet "select one" (never beside
   *  an empty list, which says where jobs come from). */
  const pick = $derived(
    !topError &&
      best.length === 0 &&
      !hasIssues &&
      jobs.status === 'ready' &&
      jobs.visible.length > 0,
  );
  let actionError = $state<string | null>(null);

  function open(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }

  /** The last fetch failed: the fitting way on, as the run card has it. */
  const failureFix = $derived(
    lastFailure === null
      ? null
      : failureAction(app.state?.lastRun ?? null, lastFailure, () => open({ kind: 'logDir' })),
  );

  /** The comparison prompt takes the favourites first, then the best scored jobs of the
   *  inbox, read or not: it is there as long as there is something to compare. */
  const counts = $derived(jobs.overviewCounts);
  const canCompare = $derived(
    app.hasProfile && counts !== null && (counts.favourites > 0 || counts.inbox > counts.excluded),
  );

  /** A best row's tools, like the list's: the inbox's actions, then the star. */
  function toolsOf(job: JobView): RowTool[] {
    return actionsOf('inbox').map((action) => ({
      id: action.id,
      icon: action.icon,
      label: action.label,
      onclick: () => {
        if (action.id === 'archive' || action.id === 'trash') {
          actionError = null;
          void move([job], action.id).then((error) => (actionError = error));
        }
      },
    }));
  }

  function pin(job: JobView): void {
    if (!guarded()) toggleStar([job]);
  }

  // The files: the HTML overview is written with the Excel file by every export, and opening
  // it writes it first, except in the dry run and while a run holds the files.
  const settings = $derived(app.state?.settings ?? null);
  const dryRun = $derived(app.state?.dryRun ?? false);
  const noFiles = $derived(settings !== null && !settings.excelExists);
  const dryRunReason = $derived(t.error.text('dryRun', {}));
</script>

{#snippet comparePrompt()}
  <span class="with-hint" use:tooltip={t.overview.promptTopHint}>
    <Button
      variant="ghost"
      size="sm"
      icon="copy"
      label={t.overview.promptTop}
      testid="prompt-top"
      onclick={() => void copyTopPrompt().then((error) => (actionError = error))}
    />
  </span>
{/snippet}

<div class="overview" data-testid="day-overview" aria-label={t.overview.label}>
  {#if topError && jobs.status !== 'error'}
    <section class="block" data-testid="best-error">
      <Notice
        tone="warning"
        variant="row"
        text={topError}
        action={{ label: t.common.retry, icon: 'refresh-cw', onclick: loadTop }}
      />
    </section>
  {:else if best.length > 0}
    <section class="block" data-testid="best">
      <div class="heading-line">
        <h2 class="heading">{t.overview.best}</h2>
        {#if canCompare}<span class="heading-action">{@render comparePrompt()}</span>{/if}
      </div>
      {#if listShowsBest}
        <p class="quiet" data-testid="top-in-list">{t.overview.bestInList}</p>
      {:else}
        <div class="best">
          {#each best as job (keyOf(job.key))}
            <JobRow
              {job}
              testid="best-{job.key.portal}-{job.key.id}"
              onselect={(chosen) => void jobs.select(chosen, true)}
              onpin={pin}
              tools={toolsOf(job)}
            />
          {/each}
        </div>
      {/if}
    </section>
  {/if}

  <!-- Nothing new to show: the comparison of the best jobs stays within reach. -->
  {#if canCompare && best.length === 0 && !topError}
    <div class="compare" data-testid="compare">{@render comparePrompt()}</div>
  {/if}

  {#if hasIssues}
    <section class="block" data-testid="issues">
      <h2 class="heading">{t.overview.issues}</h2>
      <div class="rows">
        {#if lastFailure}
          <Notice
            tone="danger"
            variant="row"
            heading={t.overview.lastRun}
            text={t.error.text(lastFailure.kind, lastFailure.params)}
            action={failureFix}
            testid="run-failed"
          />
        {/if}
        {#each portalIssues as issue (issue.id)}
          <Notice
            tone={issue.act ? 'warning' : 'info'}
            variant="row"
            heading={t.portal[issue.portal]}
            text={issue.text}
            action={issue.mail
              ? {
                  label: t.reader.mail,
                  icon: 'mail',
                  onclick: () => open({ kind: 'alertMail', gmailId: issue.mail ?? '' }),
                }
              : null}
            testid="issue-{issue.id}"
          />
        {/each}
      </div>
    </section>
  {/if}

  {#if pick}
    <p class="pick" data-testid="overview-pick">{t.overview.pick}</p>
  {/if}

  {#if fetchedOnce}
    <section class="block" data-testid="files">
      <h2 class="heading">{t.overview.files}</h2>
      <div class="files" data-testid="overview-files">
        <Button
          variant="ghost"
          size="sm"
          icon="file-text"
          label={t.run.openOverview}
          disabled={noFiles && (dryRun || run.active)}
          disabledReason={dryRun ? dryRunReason : run.busyText}
          testid="overview-open"
          onclick={() => open({ kind: 'overview' })}
        />
        <Button
          variant="ghost"
          size="sm"
          icon="file-spreadsheet"
          label={t.overview.excel}
          disabled={noFiles}
          disabledReason={dryRun ? dryRunReason : t.settings.excelMissing}
          testid="overview-excel"
          onclick={() => open({ kind: 'excel' })}
        />
        <Button
          variant="ghost"
          size="sm"
          icon="folder-open"
          label={t.common.openFolder}
          testid="overview-folder"
          onclick={() => open({ kind: 'excelInFolder' })}
        />
      </div>
    </section>
  {/if}
  {#if actionError}
    <Notice tone="danger" variant="inline" text={actionError} />
  {/if}
</div>

<style>
  /* Sections in the rhythm of the reader's: 20 px above a hairline, 16 below a heading. */
  .overview {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    container-type: inline-size;
  }

  /* The rows of "Beste Passung" like the list's: their ring on the edge of the column; the
     last row's own line gives way to the hairline of the next block. */
  .best {
    --row-rule-inset: var(--pane-padding);

    display: flex;
    flex-direction: column;
    margin: 0 calc(-1 * var(--pane-padding));
    clip-path: inset(0 0 var(--border-width) 0);
  }

  /* Sections like the reader's: a hairline above, the heading, the content (the first one
     starts the overview without a line). */
  .overview > .block:first-child {
    padding-top: 0;
    border-top: 0;
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    padding-top: var(--space-20);
    border-top: var(--border-width) solid var(--border);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  /* A heading with its one action at the end (the prompt of the best matches); the button
     is centred on the heading's line and adds no height, so the heading starts where the
     reader's title does. */
  .heading-line {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-12);
    margin-right: calc(-1 * var(--ghost-inset));
  }

  .heading-action {
    display: flex;
    margin-block: calc((var(--leading-lg) - var(--control-sm)) / 2);
  }

  .with-hint {
    display: inline-flex;
  }

  /* The comparison on its own, above the files; its icon on the edge of the column. */
  .compare {
    display: flex;
    order: 2;
    margin-left: calc(-1 * var(--ghost-inset));
  }

  .quiet {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  /* Nothing else to say: a quiet line like a mail app's empty reader, the file actions below. */
  .pick {
    order: 1;
    margin-block: var(--space-48);
    color: var(--text-subtle);
    font: var(--type-md);
    text-align: center;
  }

  .overview > [data-testid='files'] {
    order: 2;
  }

  /* Quiet file actions below everything; their icons start on the edge of the column (the
     buttons' padding and border hang out). */
  .files {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
    margin-left: calc(-1 * var(--ghost-inset));
  }

  .rows {
    display: flex;
    flex-direction: column;
  }

  /* Rows apart by a hairline with room on both sides; the block's own gap and the next
     hairline frame the first and the last. */
  .rows > :global(*) {
    padding: var(--space-12) 0;
  }

  .rows > :global(:first-child) {
    padding-top: 0;
  }

  .rows > :global(:last-child) {
    padding-bottom: 0;
  }

  .rows > :global(* + *) {
    border-top: var(--border-width) solid var(--border);
  }
</style>
