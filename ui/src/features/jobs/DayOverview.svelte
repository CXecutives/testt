<!--
  The reader's empty state: what the sheet shows while no job is selected, unboxed like the
  reader. It never repeats the list: three tiles that filter it (their numbers are the
  counts of those filters), the open points (portal health, alert mails without jobs,
  missing profile, a failed fetch) only when there are any, and the last fetch: what is new
  per portal (each a filter of the list) with the overview and the folder as quiet icon
  buttons. The time of the last fetch is said once, in the sidebar.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Notice from '$components/Notice.svelte';
  import StatTile from '$components/StatTile.svelte';
  import { de } from '$lib/i18n/de';
  import { errorText, healthText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { OpenTarget, Portal } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';

  const PORTALS: readonly Portal[] = ['linkedin', 'freelancermap', 'freelance'];

  // "Neu" is unread and not excluded, exactly like the facet Neu (they add up to its count).
  const unread = $derived(
    PORTALS.map((portal) => ({
      portal,
      count: jobs.overview.filter(
        (j) => j.portal === portal && j.unread && j.match?.status !== 'excluded',
      ).length,
    })).filter((line) => line.count > 0),
  );
  // While a run goes, the run card shows pauses and limits; they are not repeated here.
  const troubled = $derived(
    run.active ? [] : (app.state?.portals ?? []).filter((p) => p.enabled && p.health.kind !== 'ok'),
  );
  const last = $derived(run.summary ?? app.state?.lastRun ?? null);
  const emptyAlerts = $derived(app.state?.lastRun?.emptyAlerts ?? []);
  // A missing profile is said once, above the list where the rings are missing.
  const profileBroken = $derived(app.state?.profile?.parseError != null);
  // A failed fetch from before this session; a run of this session speaks in the run card.
  const lastFailure = $derived.by(() => {
    const previous = app.state?.lastRun;
    if (run.active || run.panel !== 'hidden' || previous?.outcome.kind !== 'failed') return null;
    return previous.outcome.error;
  });
  const hasIssues = $derived(
    troubled.length > 0 || emptyAlerts.length > 0 || profileBroken || lastFailure !== null,
  );
  let actionError = $state<string | null>(null);

  function open(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }
</script>

<div class="overview" data-testid="day-overview" aria-label={de.overview.label}>
  <div class="tiles">
    {#if app.hasProfile}
      <StatTile
        label={de.overview.high}
        value={jobs.counts.high}
        icon="circle-check"
        tone="success"
        active={jobs.filter === 'high'}
        testid="tile-high"
        onclick={() => jobs.setFilter(jobs.filter === 'high' ? null : 'high')}
      />
    {/if}
    <StatTile
      label={de.overview.noDetail}
      value={jobs.counts.noDetail}
      icon="file-text"
      active={jobs.filter === 'noDetail'}
      testid="tile-no-detail"
      onclick={() => jobs.setFilter(jobs.filter === 'noDetail' ? null : 'noDetail')}
    />
    {#if app.hasProfile}
      <StatTile
        label={de.overview.excluded}
        value={jobs.counts.excluded}
        icon="ban"
        active={jobs.filter === 'excluded'}
        testid="tile-excluded"
        onclick={() => jobs.setFilter(jobs.filter === 'excluded' ? null : 'excluded')}
      />
    {/if}
  </div>

  {#if hasIssues}
    <section class="block" data-testid="issues">
      <h2 class="heading">{de.overview.issues}</h2>
      <div class="rows">
        {#if lastFailure}
          <Notice
            tone="danger"
            variant="row"
            heading={de.run.failed}
            text={de.error.text(lastFailure.kind, lastFailure.params)}
            action={{ label: de.common.retry, onclick: () => void run.start({ kind: 'fetch' }) }}
            testid="run-failed"
          />
        {/if}
        {#if profileBroken}
          <Notice
            tone="info"
            variant="row"
            text={de.profile.parseError}
            action={{ label: de.list.pickProfile, onclick: () => navigation.go('profile') }}
          />
        {/if}
        {#each troubled as portal (portal.portal)}
          {@const health = healthText(portal.health)}
          <Notice
            tone="warning"
            variant="row"
            heading={de.portal[portal.portal]}
            text={health.text ?? health.label}
            testid="issue-{portal.portal}"
          />
        {/each}
        {#each emptyAlerts as alert, index (index)}
          <Notice
            tone="warning"
            variant="row"
            text={de.overview.emptyAlert(alert.portal)}
            action={alert.gmailId
              ? {
                  label: de.overview.openGmail,
                  onclick: () => open({ kind: 'alertMail', gmailId: alert.gmailId ?? '' }),
                }
              : null}
            testid="issue-alert"
          />
        {/each}
      </div>
    </section>
  {/if}

  <!-- While the run card is open it tells the same; the overview does not repeat it. -->
  {#if last && !run.active && run.panel === 'hidden'}
    <section class="block" data-testid="last-run">
      <div class="block-head">
        <h2 class="heading">{de.overview.lastRun}</h2>
        <span class="tools">
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            icon="external-link"
            label={de.run.openOverview}
            testid="overview-open"
            onclick={() => open({ kind: 'overview' })}
          />
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            icon="folder-open"
            label={de.common.openFolder}
            testid="overview-folder"
            onclick={() => open({ kind: 'workspace' })}
          />
        </span>
      </div>
      {#if unread.length > 0}
        <ul class="portals" data-testid="new-per-portal">
          {#each unread as line (line.portal)}
            <li>
              <Button
                variant="secondary"
                size="sm"
                label={de.overview.newOn(de.portal[line.portal], line.count)}
                pressed={jobs.filter === line.portal}
                testid="new-{line.portal}"
                onclick={() => jobs.setFilter(jobs.filter === line.portal ? null : line.portal)}
              />
            </li>
          {/each}
        </ul>
      {:else if jobs.overviewReady}
        <p class="quiet">{de.overview.nothingNew}</p>
      {/if}
      {#if actionError}
        <Notice tone="danger" variant="inline" text={actionError} />
      {/if}
    </section>
  {/if}
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    container-type: inline-size;
  }

  /* Always three columns (a missing tile leaves its place empty), one column only when
     three would be too narrow for their labels. */
  .tiles {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-12);
  }

  @container (width < 460px) {
    .tiles {
      grid-template-columns: 1fr;
    }
  }

  /* Sections like the reader's: a hairline above, the heading, the content. */
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    padding-top: var(--space-20);
    border-top: var(--border-width) solid var(--border);
  }

  .block-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-8);
    min-height: var(--control-sm);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .tools {
    display: flex;
    gap: var(--space-4);
  }

  .rows {
    display: flex;
    flex-direction: column;
  }

  .rows > :global(*) {
    padding: var(--space-12) 0;
  }

  .rows > :global(* + *) {
    border-top: var(--border-width) solid var(--border);
  }

  .portals {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-8);
  }

  .quiet {
    color: var(--text-muted);
    font: var(--type-md);
  }
</style>
