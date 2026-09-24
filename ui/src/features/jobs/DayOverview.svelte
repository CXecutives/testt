<!--
  The reader's empty state: what the sheet shows while no job is selected, unboxed like the
  reader. It answers "what now" in a short list, no counts (the list header counts): without
  a usable profile a calm card that leads to one, "Beste Passung" (the three best scored new
  jobs as list rows; a click opens the job), the open points (one per portal and problem, a
  failed fetch, each with its action) only when there are any, and at the end the overview
  file, the Excel file and the folder, their one place in the Jobs view. "Nothing new" is
  said by the list and the run card, not here. The time of the last fetch is said once, in
  the sidebar. The portals keep the one order of the app (the settings').
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import JobRow from '$components/JobRow.svelte';
  import Notice from '$components/Notice.svelte';
  import { de } from '$lib/i18n/de';
  import { errorText, healthSentence } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { EmptyAlert, JobView, OpenTarget, Portal, PortalState } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { jobs, keyOf, sameKey } from '$lib/state/jobs.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { editor } from '$lib/state/profile.svelte';
  import { run } from '$lib/state/run.svelte';

  // The one order of the portals (the backend's, as in the settings).
  const portals = $derived((app.state?.portals ?? []).map((p) => p.portal));
  const BEST = 3;
  /** The best scored new jobs (one small query; again whenever the counts move). */
  let top = $state.raw<JobView[]>([]);
  let topRequest = 0;
  $effect(() => {
    void jobs.overviewCounts;
    const scoring = app.hasProfile;
    untrack(() => {
      const request = ++topRequest;
      if (!scoring) {
        top = [];
        return;
      }
      invoke('list_jobs', {
        query: { facet: 'new', sort: 'match', search: null, limit: BEST, offset: 0 },
      })
        .then((page) => {
          if (request === topRequest) top = page.jobs;
        })
        .catch((error: unknown) => (actionError = errorText(error)));
    });
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
  }

  /**
   * Each problem of a portal once: alert mails without jobs (the portal's "layout suspect"
   * health and the empty alerts of the last fetch are one thing) with "Alert-Mail öffnen", and
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
        text: de.overview.emptyAlerts(mails),
        mail: alerts.find((a) => a.gmailId !== null)?.gmailId ?? null,
      });
    } else if (suspect !== null) {
      out.push({ id: `${portal}-pages`, portal, text: de.health.layoutPages, mail: null });
    }
    if (health !== null && health.kind !== 'ok' && suspect === null) {
      out.push({ id: `${portal}-health`, portal, text: healthSentence(health) ?? '', mail: null });
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
  const profileMissing = $derived(app.state !== null && !app.hasProfile);
  // A profile that is there but cannot be used is named, and the card leads to it.
  const profileCard = $derived.by(() => {
    const profile = app.state?.profile ?? null;
    if (profile === null) {
      return {
        heading: de.overview.noProfile,
        text: de.overview.noProfileText,
        label: de.list.createProfile,
        icon: 'file-text' as const,
      };
    }
    return {
      heading: profile.parseError ? de.overview.profileUnreadable : de.overview.profileEmpty,
      text: de.overview.profileBrokenText,
      label: de.list.openProfile,
      icon: 'user-round' as const,
    };
  });
  const fetchedOnce = $derived((run.summary ?? app.state?.lastRun ?? null) !== null);
  let actionError = $state<string | null>(null);

  function open(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }
</script>

<div class="overview" data-testid="day-overview" aria-label={de.overview.label}>
  {#if profileMissing}
    <Card variant="tinted" padding="md" testid="no-profile">
      <div class="profile">
        <div class="profile-copy">
          <h2 class="card-heading">{profileCard.heading}</h2>
          <p class="card-text">{profileCard.text}</p>
        </div>
        <Button
          variant="secondary"
          icon={profileCard.icon}
          label={profileCard.label}
          testid="choose-profile"
          onclick={() => {
            // No profile yet: straight into the empty form, one click.
            if (app.state?.profile == null) editor.create();
            navigation.go('profile');
          }}
        />
      </div>
    </Card>
  {/if}

  {#if best.length > 0}
    <section class="block" data-testid="best">
      <h2 class="heading">{de.overview.best}</h2>
      <div class="best">
        {#each best as job (keyOf(job.key))}
          <JobRow {job} onselect={(chosen) => void jobs.select(chosen, true)} />
        {/each}
      </div>
    </section>
  {/if}

  {#if hasIssues}
    <section class="block" data-testid="issues">
      <h2 class="heading">{de.overview.issues}</h2>
      <div class="rows">
        {#if lastFailure}
          <Notice
            tone="danger"
            variant="row"
            heading={de.overview.lastRun}
            text={de.error.text(lastFailure.kind, lastFailure.params)}
            action={{
              label: de.common.retry,
              onclick: () => run.retry(app.state?.lastRun ?? null),
            }}
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
                  label: de.reader.mail,
                  onclick: () => open({ kind: 'alertMail', gmailId: issue.mail ?? '' }),
                }
              : null}
            testid="issue-{issue.id}"
          />
        {/each}
      </div>
    </section>
  {/if}

  {#if fetchedOnce}
    <div class="files" data-testid="overview-files">
      <Button
        variant="ghost"
        size="sm"
        icon="external-link"
        label={de.run.openOverview}
        testid="overview-open"
        onclick={() => open({ kind: 'overview' })}
      />
      <Button
        variant="ghost"
        size="sm"
        icon="file-text"
        label={de.overview.excel}
        testid="overview-excel"
        onclick={() => open({ kind: 'excel' })}
      />
      <Button
        variant="ghost"
        size="sm"
        icon="folder-open"
        label={de.common.openFolder}
        testid="overview-folder"
        onclick={() => open({ kind: 'workspace' })}
      />
    </div>
  {/if}
  {#if actionError}
    <Notice tone="danger" variant="inline" text={actionError} />
  {/if}
</div>

<style>
  /* The first hairline lands on the line of the list header's bottom edge. */
  .overview {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    container-type: inline-size;
  }

  /* The rows of "Beste Passung" like the list's: their ring on the edge of the column; the
     last row's own line gives way to the hairline of the next block. */
  .best {
    display: flex;
    flex-direction: column;
    margin: 0 calc(-1 * var(--pane-padding));
    clip-path: inset(0 0 var(--border-width) 0);
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

  /* Sections like the reader's: a hairline above, the heading, the content (the first one
     starts the overview without a line). */
  .overview > .block:first-child {
    padding-top: 0;
    border-top: 0;
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    padding-top: var(--space-20);
    border-top: var(--border-width) solid var(--border);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  /* Quiet file actions below everything; their icons start on the edge of the column. */
  .files {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
    margin-left: calc(-1 * var(--space-12));
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
</style>
