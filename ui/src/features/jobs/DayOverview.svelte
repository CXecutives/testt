<!--
  What the reader side shows while no job is selected. It never repeats the list: three
  tiles that filter it (their numbers are the counts of those filters), the open points
  (portal health, alert mails without jobs, missing profile, a failed fetch) only when there
  are any, and the last fetch: when, what is new per portal, the overview and the folder.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Notice from '$components/Notice.svelte';
  import StatTile from '$components/StatTile.svelte';
  import { de } from '$lib/i18n/de';
  import { formatMoment } from '$lib/i18n/format';
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
        icon="star"
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
    <Card padding="none" testid="issues">
      <div class="card-head">
        <h2 class="heading">{de.overview.issues}</h2>
      </div>
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
    </Card>
  {/if}

  <!-- While the run card is open it tells the same; the overview does not repeat it. -->
  {#if last && !run.active && run.panel === 'hidden'}
    <Card padding="none" testid="last-run">
      <div class="card-head">
        <h2 class="heading">{de.overview.lastRun}</h2>
        <span class="time">{formatMoment(last.finishedAt)}</span>
      </div>
      <div class="last">
        {#if unread.length > 0}
          <ul class="portals" data-testid="new-per-portal">
            {#each unread as line (line.portal)}
              <li class="chip">{de.overview.newOn(de.portal[line.portal], line.count)}</li>
            {/each}
          </ul>
        {:else}
          <p class="quiet">{de.overview.nothingNew}</p>
        {/if}
        <div class="actions">
          <Button
            variant="secondary"
            size="sm"
            icon="external-link"
            label={de.run.openOverview}
            testid="overview-open"
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
      </div>
      {#if actionError}
        <div class="error"><Notice tone="danger" variant="inline" text={actionError} /></div>
      {/if}
    </Card>
  {/if}
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--stat-min), 1fr));
    gap: var(--space-16);
  }

  .card-head {
    display: flex;
    align-items: baseline;
    gap: var(--space-8);
    padding: var(--space-16) var(--space-20) 0;
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .time {
    color: var(--text-muted);
    font: var(--type-md);
    font-variant-numeric: var(--numeric);
  }

  .rows {
    display: flex;
    flex-direction: column;
    padding: var(--space-4) var(--space-20) var(--space-4);
  }

  .rows > :global(* + *) {
    border-top: var(--border-width) solid var(--border);
  }

  .last {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-12) var(--space-16);
    padding: var(--space-12) var(--space-20) var(--space-16);
  }

  .portals {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    height: var(--badge-height);
    padding: 0 var(--space-8);
    border-radius: var(--radius-full);
    background-color: var(--surface-muted);
    color: var(--text-muted);
    font: var(--type-xs);
    font-weight: var(--weight-medium);
    font-variant-numeric: var(--numeric);
    white-space: nowrap;
  }

  .quiet {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .actions {
    display: flex;
    gap: var(--space-8);
  }

  .error {
    padding: 0 var(--space-20) var(--space-16);
  }
</style>
