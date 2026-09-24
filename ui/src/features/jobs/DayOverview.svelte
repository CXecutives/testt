<!--
  What the reader shows while no job is selected: three tiles that filter the list, new
  jobs per portal, the best three, the pinned ones and the open points (portal health,
  alert mails without jobs, missing profile) plus the overview file.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import IconTile, { PORTAL_MONOGRAM } from '$components/IconTile.svelte';
  import JobRow from '$components/JobRow.svelte';
  import Notice from '$components/Notice.svelte';
  import StatTile from '$components/StatTile.svelte';
  import { de } from '$lib/i18n/de';
  import { formatNumber } from '$lib/i18n/format';
  import { errorText, healthText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { JobView, OpenTarget, Portal } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { jobs, keyOf } from '$lib/state/jobs.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';

  const PORTALS: readonly Portal[] = ['linkedin', 'freelancermap', 'freelance'];
  const BEST = 3;

  const all = $derived(jobs.overview);
  const unread = $derived(
    PORTALS.map((portal) => ({
      portal,
      count: all.filter((j) => j.portal === portal && j.unread && j.match?.status !== 'excluded')
        .length,
    })).filter((line) => line.count > 0),
  );
  const best = $derived(
    app.hasProfile
      ? all
          .filter((j) => j.match?.status === 'scored')
          .sort((a, b) => (b.match?.score ?? 0) - (a.match?.score ?? 0))
          .slice(0, BEST)
      : [],
  );
  const pinned = $derived(all.filter((j) => j.pinned));
  // While a run goes, the run card shows pauses and limits; they are not repeated here.
  const troubled = $derived(
    run.active ? [] : (app.state?.portals ?? []).filter((p) => p.enabled && p.health.kind !== 'ok'),
  );
  const emptyAlerts = $derived(app.state?.lastRun?.emptyAlerts ?? []);
  const profileMissing = $derived(app.state !== null && app.state.profile === null);
  const profileBroken = $derived(app.state?.profile?.parseError != null);
  const hasJobs = $derived(all.length > 0);
  // A failed fetch from before this session; a run of this session speaks in the run card.
  const lastFailure = $derived.by(() => {
    const last = app.state?.lastRun;
    if (run.active || run.panel !== 'hidden' || last?.outcome.kind !== 'failed') return null;
    return last.outcome.error;
  });
  let actionError = $state<string | null>(null);

  function open(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }

  function select(job: JobView): void {
    void jobs.select(job, true);
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
        testid="tile-high"
        onclick={() => jobs.setFilter('high')}
      />
    {/if}
    <StatTile
      label={de.overview.noDetail}
      value={jobs.counts.noDetail}
      icon="file-text"
      tone="neutral"
      testid="tile-no-detail"
      onclick={() => jobs.setFilter('noDetail')}
    />
    {#if app.hasProfile}
      <StatTile
        label={de.overview.excluded}
        value={jobs.counts.excluded}
        icon="ban"
        tone="danger"
        testid="tile-excluded"
        onclick={() => jobs.setFilter('excluded')}
      />
    {/if}
  </div>

  {#if troubled.length > 0 || emptyAlerts.length > 0 || profileMissing || profileBroken || lastFailure}
    <Card padding="md" testid="issues">
      <h2 class="heading">{de.overview.issues}</h2>
      <div class="stack">
        {#if lastFailure}
          <Notice
            tone="danger"
            heading={de.run.failed}
            text={de.error.text(lastFailure.kind, lastFailure.params)}
            action={{ label: de.common.retry, onclick: () => void run.start({ kind: 'fetch' }) }}
            testid="run-failed"
          />
        {/if}
        {#if profileMissing || profileBroken}
          <Notice
            tone="info"
            text={profileBroken ? de.profile.parseError : de.list.noProfile}
            action={{ label: de.list.pickProfile, onclick: () => navigation.go('profile') }}
          />
        {/if}
        {#each troubled as portal (portal.portal)}
          {@const health = healthText(portal.health)}
          <Notice
            tone="warning"
            heading={de.portal[portal.portal]}
            text={health.text ?? health.label}
            testid="issue-{portal.portal}"
          />
        {/each}
        {#each emptyAlerts as alert, index (index)}
          <Notice
            tone="warning"
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

  {#if unread.length > 0}
    <Card padding="md" testid="unread-portals">
      <h2 class="heading">{de.overview.unread}</h2>
      <ul class="portals">
        {#each unread as line (line.portal)}
          <li class="portal">
            <IconTile monogram={PORTAL_MONOGRAM[line.portal]} tone="coral" size="sm" />
            <span class="name">{de.portal[line.portal]}</span>
            <span class="count">{formatNumber(line.count)}</span>
          </li>
        {/each}
      </ul>
    </Card>
  {/if}

  {#if best.length > 0}
    <Card padding="none" testid="best">
      <h2 class="heading padded">{de.overview.best}</h2>
      {#each best as job (keyOf(job.key))}
        <JobRow {job} onselect={select} />
      {/each}
    </Card>
  {/if}

  {#if pinned.length > 0}
    <Card padding="none" testid="pinned">
      <h2 class="heading padded">{de.overview.pinned}</h2>
      {#each pinned as job (keyOf(job.key))}
        <JobRow {job} ring={app.hasProfile} onselect={select} />
      {/each}
    </Card>
  {/if}

  {#if hasJobs}
    <div class="footer">
      <p class="hint">{de.overview.choose}</p>
      <Button
        variant="secondary"
        icon="external-link"
        label={de.run.openOverview}
        onclick={() => open({ kind: 'overview' })}
      />
    </div>
  {/if}
  {#if actionError}
    <Notice tone="danger" variant="inline" text={actionError} />
  {/if}
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: var(--space-24);
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(var(--stat-min), 1fr));
    gap: var(--space-16);
  }

  .heading {
    margin-bottom: var(--space-12);
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .padded {
    margin: 0;
    padding: var(--space-16) var(--space-16) var(--space-8);
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .portals {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .portal {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    font: var(--type-md);
  }

  .count {
    margin-left: auto;
    color: var(--text-heading);
    font-weight: var(--weight-semibold);
    font-variant-numeric: var(--numeric);
  }

  .footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-12);
  }

  .hint {
    color: var(--text-muted);
    font: var(--type-md);
  }
</style>
