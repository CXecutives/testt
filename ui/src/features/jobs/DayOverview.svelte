<!--
  The reader's empty state: what the sheet shows while no job is selected, unboxed like the
  reader. It never repeats the list and never looks empty: tiles that filter the list (their
  numbers count over every job, whatever the search), without a profile the tiles Neu and
  Ohne Details and a calm card that leads to choosing one, the open points (one per portal
  and problem, a failed fetch) only when there are any, and the new jobs per portal (each a
  filter of the list) with the overview and the folder as quiet icon buttons. The time of
  the last fetch is said once, in the sidebar.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Notice from '$components/Notice.svelte';
  import StatTile from '$components/StatTile.svelte';
  import { cssVars } from '$lib/actions/cssVars';
  import { de } from '$lib/i18n/de';
  import { errorText, healthText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { EmptyAlert, OpenTarget, Portal, PortalState } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { isExcluded, jobs, type JobFilter } from '$lib/state/jobs.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';

  const PORTALS: readonly Portal[] = ['linkedin', 'freelancermap', 'freelance'];

  const counts = $derived(jobs.overviewCounts ?? app.state?.counts ?? null);
  // "Neu" is unread and not excluded, exactly like the facet Neu (they add up to its count).
  const unread = $derived(
    jobs.overviewStatus === 'ready'
      ? PORTALS.map((portal) => ({
          portal,
          count: jobs.overview.filter((j) => j.portal === portal && j.unread && !isExcluded(j))
            .length,
        })).filter((line) => line.count > 0)
      : [],
  );

  interface Tile {
    id: JobFilter | 'new';
    label: string;
    value: number;
    icon: 'circle-check' | 'file-text' | 'ban' | 'star' | 'inbox';
    tone?: 'success';
  }
  const tiles = $derived.by((): Tile[] => {
    if (counts === null) return [];
    const out: Tile[] = app.hasProfile
      ? [
          {
            id: 'high',
            label: de.overview.high,
            value: counts.high,
            icon: 'circle-check',
            tone: 'success',
          },
          {
            id: 'noDetail',
            label: de.overview.noDetail,
            value: counts.noDetail,
            icon: 'file-text',
          },
          { id: 'excluded', label: de.overview.excluded, value: counts.excluded, icon: 'ban' },
        ]
      : [
          { id: 'new', label: de.overview.new, value: counts.new, icon: 'inbox' },
          {
            id: 'noDetail',
            label: de.overview.noDetail,
            value: counts.noDetail,
            icon: 'file-text',
          },
        ];
    if (jobs.pinned > 0) {
      out.push({ id: 'pinned', label: de.overview.pinned, value: jobs.pinned, icon: 'star' });
    }
    return out;
  });

  function toggle(tile: Tile): void {
    if (tile.id === 'new') jobs.setFacet('new');
    else jobs.setFilter(jobs.filter === tile.id ? null : tile.id);
  }

  // While a run goes, the run card shows pauses and limits; they are not repeated here.
  const troubled = $derived(
    run.active ? [] : (app.state?.portals ?? []).filter((p) => p.enabled && p.health.kind !== 'ok'),
  );
  const emptyAlerts = $derived(app.state?.lastRun?.emptyAlerts ?? []);

  interface Issue {
    id: string;
    portal: Portal;
    text: string;
    /** An alert mail to open in Gmail. */
    mail: string | null;
  }

  /**
   * Each problem of a portal once: alert mails without jobs (the portal's "layout suspect"
   * health and the empty alerts of the last fetch are one thing) with "In Gmail öffnen", and
   * a pause, a limit or a sign-in as its own line.
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
        text: de.overview.emptyAlerts(portal, mails),
        mail: alerts.find((a) => a.gmailId !== null)?.gmailId ?? null,
      });
    } else if (suspect !== null) {
      out.push({ id: `${portal}-pages`, portal, text: de.health.layoutPages, mail: null });
    }
    if (health !== null && health.kind !== 'ok' && suspect === null) {
      const said = healthText(health);
      out.push({ id: `${portal}-health`, portal, text: said.text ?? said.label, mail: null });
    }
    return out;
  }

  const portalIssues = $derived(
    PORTALS.flatMap((portal) =>
      issuesOf(
        portal,
        troubled.find((p) => p.portal === portal),
        emptyAlerts.filter((a) => a.portal === portal),
      ),
    ),
  );
  // A failed fetch from before this session; a run of this session speaks in the run card.
  const lastFailure = $derived.by(() => {
    const previous = app.state?.lastRun;
    if (run.active || run.panel !== 'hidden' || previous?.outcome.kind !== 'failed') return null;
    return previous.outcome.error;
  });
  const hasIssues = $derived(portalIssues.length > 0 || lastFailure !== null);
  const profileMissing = $derived(app.state !== null && !app.hasProfile);
  const fetchedOnce = $derived((run.summary ?? app.state?.lastRun ?? null) !== null);
  let actionError = $state<string | null>(null);
  let picking = $state(false);

  function open(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }

  /** No profile: choose one right here. A profile that no longer reads: the Profil view. */
  function chooseProfile(): void {
    if (app.state?.profile != null) {
      navigation.go('profile');
      return;
    }
    actionError = null;
    picking = true;
    invoke('pick_profile')
      .then(async (profile) => {
        if (profile === null) return;
        await app.load();
        void jobs.load(true);
        void jobs.loadOverview();
      })
      .catch((error: unknown) => (actionError = errorText(error)))
      .finally(() => (picking = false));
  }
</script>

<div class="overview" data-testid="day-overview" aria-label={de.overview.label}>
  {#if tiles.length > 0}
    <div class="tiles" class:many={tiles.length > 3} use:cssVars={{ tiles: tiles.length }}>
      {#each tiles as tile (tile.id)}
        <StatTile
          label={tile.label}
          value={tile.value}
          icon={tile.icon}
          tone={tile.tone ?? 'neutral'}
          active={tile.id !== 'new' && jobs.filter === tile.id}
          testid="tile-{tile.id === 'noDetail' ? 'no-detail' : tile.id}"
          onclick={() => toggle(tile)}
        />
      {/each}
    </div>
  {/if}

  {#if profileMissing}
    <Card variant="tinted" padding="md" testid="no-profile">
      <div class="profile">
        <div class="profile-copy">
          <h2 class="card-heading">
            {app.state?.profile == null
              ? de.overview.noProfile
              : app.state.profile.parseError
                ? de.profile.parseError
                : de.profile.qualityText.empty}
          </h2>
          <p class="card-text">{de.overview.noProfileText}</p>
        </div>
        <Button
          variant="secondary"
          icon="file-up"
          label={de.list.pickProfile}
          loading={picking}
          testid="choose-profile"
          onclick={chooseProfile}
        />
      </div>
    </Card>
  {/if}

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
        {#each portalIssues as issue (issue.id)}
          <Notice
            tone="warning"
            variant="row"
            heading={de.portal[issue.portal]}
            text={issue.text}
            action={issue.mail
              ? {
                  label: de.overview.openGmail,
                  onclick: () => open({ kind: 'alertMail', gmailId: issue.mail ?? '' }),
                }
              : null}
            testid="issue-{issue.id}"
          />
        {/each}
      </div>
    </section>
  {/if}

  {#if fetchedOnce && jobs.overviewStatus === 'ready'}
    <section class="block" data-testid="new-jobs">
      <div class="block-head">
        <h2 class="heading">{de.overview.newJobs}</h2>
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
      {:else}
        <p class="quiet">{de.overview.nothingNew}</p>
      {/if}
    </section>
  {/if}
  {#if actionError}
    <Notice tone="danger" variant="inline" text={actionError} />
  {/if}
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    container-type: inline-size;
  }

  /* One row of tiles, as many columns as tiles; four tiles take two rows where one row
     would cut their labels (the reader column is at most 720 px), and one column only when
     two would. */
  .tiles {
    display: grid;
    grid-template-columns: repeat(var(--tiles), minmax(0, 1fr));
    gap: var(--space-12);
  }

  @container (width < 720px) {
    .tiles.many {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @container (width < 460px) {
    .tiles,
    .tiles.many {
      grid-template-columns: 1fr;
    }
  }

  .profile {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-12) var(--space-16);
  }

  .profile-copy {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  .card-heading {
    color: var(--text-heading);
    font: var(--type-md);
    font-weight: var(--weight-medium);
  }

  .card-text {
    color: var(--text-muted);
    font: var(--type-sm);
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

  /* The ghost icons end on the edge of the column like the text above them. */
  .tools {
    display: flex;
    gap: var(--space-4);
    margin-right: calc(-1 * var(--space-6));
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
